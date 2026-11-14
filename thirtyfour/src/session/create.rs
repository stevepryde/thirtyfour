use serde::Deserialize;
use url::Url;

use super::http::HttpClient;
use crate::error::WebDriverErrorInner;
use crate::{
    Capabilities, SessionId, TimeoutConfiguration,
    common::{
        command::{Command, FormatRequestData},
        config::WebDriverConfig,
    },
    prelude::WebDriverResult,
    session::http::run_webdriver_cmd,
};

/// Result of [`start_session`]: the session id assigned by the server, plus
/// the capabilities map the server returned (used for CDP WebSocket URL
/// discovery, BiDi, etc.).
#[derive(Debug)]
pub struct StartedSession {
    /// Session id assigned by the WebDriver server.
    pub session_id: SessionId,
    /// Capabilities returned by `New Session`. May be empty if the server
    /// returned an unrecognised shape.
    pub capabilities: Capabilities,
}

/// Start a new WebDriver session, returning the session id and the
/// capabilities JSON that was received back from the server.
///
/// Managed and external-driver constructors share this path and its single
/// retry budget (at most two New Session requests). Besides the legacy
/// Selenium UnknownError/500 workaround, Chromium's exact missing DevTools port
/// error is retried once. Other session errors and transport failures are not
/// retried by the new policy. The final error is returned unchanged.
pub async fn start_session(
    http_client: &dyn HttpClient,
    server_url: &Url,
    config: &WebDriverConfig,
    capabilities: Capabilities,
) -> WebDriverResult<StartedSession> {
    let is_chromium =
        capabilities.get("browserName").and_then(serde_json::Value::as_str).is_some_and(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "chrome" | "chromium" | "microsoftedge" | "msedge" | "edge"
            )
        });
    let request_data = Command::NewSession(capabilities.into()).format_request(&SessionId::null());

    let v = match run_webdriver_cmd(http_client, &request_data, server_url, config).await {
        Ok(x) => Ok(x),
        Err(e) => {
            // Share one retry budget with the legacy Selenium HTTP-500 workaround.
            // Never retry an uncertain transport failure: a session may already exist.
            if retry_new_session(&e, is_chromium) {
                tracing::warn!(error = %e, "retrying failed New Session once");
                run_webdriver_cmd(http_client, &request_data, server_url, config).await
            } else {
                Err(e)
            }
        }
    }?;

    #[derive(Debug, Deserialize)]
    struct ConnectionData {
        #[serde(default, rename(deserialize = "sessionId"))]
        session_id: String,
        #[serde(default)]
        capabilities: serde_json::Value,
    }

    #[derive(Debug, Deserialize)]
    struct ConnectionResp {
        #[serde(default, rename(deserialize = "sessionId"))]
        session_id: String,
        value: ConnectionData,
    }

    let resp: ConnectionResp = serde_json::from_value(v.body)?;
    let data = resp.value;
    let session_id = SessionId::from(if resp.session_id.is_empty() {
        data.session_id
    } else {
        resp.session_id
    });
    let capabilities = match data.capabilities {
        serde_json::Value::Object(map) => Capabilities::from(map),
        _ => Capabilities::new(),
    };

    // Set default timeouts.
    let request_data =
        Command::SetTimeouts(TimeoutConfiguration::default()).format_request(&session_id);
    run_webdriver_cmd(http_client, &request_data, server_url, config).await?;

    Ok(StartedSession {
        session_id,
        capabilities,
    })
}

/// ChromeDriver or EdgeDriver has already accepted New Session here; this is
/// not the manager's `/status` readiness check. Retry only its definitive missing-port failure.
/// The driver owns failed-launch cleanup before replying. No session ID exists
/// to DELETE, and we must not kill a shared/external driver or delete its files.
fn retry_new_session(error: &crate::error::WebDriverError, is_chromium: bool) -> bool {
    match &**error {
        // Preserve the existing Selenium compatibility retry.
        WebDriverErrorInner::UnknownError(info) => info.status == 500,
        WebDriverErrorInner::SessionNotCreated(info) if is_chromium && info.status == 500 => {
            // Exact matching deliberately excludes nested errors such as
            // "cannot kill ...: DevToolsActivePort file doesn't exist", which
            // indicate that the driver's failed-launch cleanup did not finish.
            matches!(
                info.value.message.as_str(),
                "DevToolsActivePort file doesn't exist"
                    | "session not created: DevToolsActivePort file doesn't exist"
            )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::WebDriverError;
    use crate::session::http::Body;
    use bytes::Bytes;
    use http::{Request, Response};
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    struct ScriptedClient {
        responses: Mutex<VecDeque<WebDriverResult<Response<Bytes>>>>,
        paths: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl HttpClient for ScriptedClient {
        async fn send(&self, request: Request<Body<'_>>) -> WebDriverResult<Response<Bytes>> {
            self.paths.lock().unwrap().push(request.uri().path().to_string());
            self.responses.lock().unwrap().pop_front().expect("unexpected extra request")
        }

        async fn new(&self) -> Arc<dyn HttpClient> {
            panic!("start_session must not replace the client")
        }
    }

    fn response(status: u16, value: serde_json::Value) -> WebDriverResult<Response<Bytes>> {
        Ok(Response::builder().status(status).body(Bytes::from(value.to_string())).unwrap())
    }

    fn failure(kind: &str, message: &str) -> WebDriverResult<Response<Bytes>> {
        response(
            500,
            json!({"value": {"error": kind, "message": message, "stacktrace": "original driver stack"}}),
        )
    }

    async fn run(
        browser: &str,
        responses: Vec<WebDriverResult<Response<Bytes>>>,
    ) -> (WebDriverResult<StartedSession>, Vec<String>) {
        let client = ScriptedClient {
            responses: Mutex::new(responses.into()),
            paths: Mutex::new(Vec::new()),
        };
        let mut caps = Capabilities::new();
        caps.set("browserName", browser).unwrap();
        let result = start_session(
            &client,
            &Url::parse("http://localhost:4444").unwrap(),
            &WebDriverConfig::default(),
            caps,
        )
        .await;
        (result, client.paths.into_inner().unwrap())
    }

    const MISSING: &str = "session not created: DevToolsActivePort file doesn't exist";

    #[tokio::test]
    async fn chromium_missing_port_retries_once_then_initializes_only_successful_session() {
        for browser in ["chrome", "chromium", "MicrosoftEdge", "msedge", "edge", "CHROME"] {
            for message in [MISSING, "DevToolsActivePort file doesn't exist"] {
                let (result, paths) = run(
                    browser,
                    vec![
                        failure("session not created", message),
                        response(
                            200,
                            json!({"value": {"sessionId": "new-session", "capabilities": {}}}),
                        ),
                        response(200, json!({"value": null})),
                    ],
                )
                .await;
                assert_eq!(result.unwrap().session_id.to_string(), "new-session");
                assert_eq!(paths, ["/session", "/session", "/session/new-session/timeouts"]);
            }
        }
    }

    #[rstest::rstest]
    #[case("chrome")]
    #[case("chromium")]
    #[case("MicrosoftEdge")]
    #[tokio::test]
    async fn repeated_failure_is_bounded_and_preserves_final_driver_error(#[case] browser: &str) {
        let (result, paths) = run(
            browser,
            vec![failure("session not created", MISSING), failure("session not created", MISSING)],
        )
        .await;
        let error = result.unwrap_err();
        let WebDriverErrorInner::SessionNotCreated(info) = &*error else {
            panic!("wrong error: {error:?}")
        };
        assert_eq!(info.value.message, MISSING);
        assert_eq!(info.value.stacktrace.as_deref(), Some("original driver stack"));
        assert_eq!(paths, ["/session", "/session"]);
    }

    #[tokio::test]
    async fn unrelated_session_errors_and_other_browsers_are_not_retried() {
        for (browser, kind, message) in [
            ("firefox", "session not created", MISSING),
            ("safari", "session not created", MISSING),
            ("MicrosoftEdge", "invalid argument", MISSING),
            ("MicrosoftEdge", "session not created", "unsupported browser version"),
            (
                "MicrosoftEdge",
                "session not created",
                "cannot kill browser: DevToolsActivePort file doesn't exist",
            ),
            ("MicrosoftEdge", "session not created", "user data directory is already in use"),
            ("chrome", "session not created", "unsupported browser version"),
            (
                "chrome",
                "session not created",
                "cannot kill Chrome: DevToolsActivePort file doesn't exist",
            ),
            ("chrome", "session not created", "user data directory is already in use"),
        ] {
            let (result, paths) = run(browser, vec![failure(kind, message)]).await;
            assert!(result.is_err());
            assert_eq!(paths, ["/session"]);
        }
    }

    #[tokio::test]
    async fn uncertain_transport_failure_is_not_retried() {
        for browser in ["chrome", "chromium", "MicrosoftEdge"] {
            for error in [
                WebDriverError::RequestFailed("connection lost".into()),
                WebDriverError::HttpError("request timed out".into()),
                WebDriverError::Timeout("timed out".into()),
            ] {
                let (result, paths) = run(browser, vec![Err(error)]).await;
                assert!(result.is_err());
                assert_eq!(paths, ["/session"]);
            }
        }
    }

    #[rstest::rstest]
    #[case("chrome")]
    #[case("chromium")]
    #[case("MicrosoftEdge")]
    #[tokio::test]
    async fn success_and_non_500_failures_do_not_retry(#[case] browser: &str) {
        let (result, paths) = run(
            browser,
            vec![
                response(200, json!({"value": {"sessionId": "first", "capabilities": {}}})),
                response(200, json!({"value": null})),
            ],
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(paths, ["/session", "/session/first/timeouts"]);
        let (result, paths) = run(
            browser,
            vec![response(
                400,
                json!({"value": {"error": "session not created", "message": MISSING}}),
            )],
        )
        .await;
        assert!(result.is_err());
        assert_eq!(paths, ["/session"]);
    }

    #[rstest::rstest]
    #[case("chrome")]
    #[case("chromium")]
    #[case("MicrosoftEdge")]
    #[tokio::test]
    async fn retry_returns_unrelated_final_error_without_masking_or_retrying_it(
        #[case] browser: &str,
    ) {
        let (result, paths) = run(
            browser,
            vec![
                failure("session not created", MISSING),
                failure("invalid argument", "invalid capability"),
            ],
        )
        .await;
        let error = result.unwrap_err();
        let WebDriverErrorInner::InvalidArgument(info) = &*error else {
            panic!("wrong error: {error:?}")
        };
        assert_eq!(info.value.message, "invalid capability");
        assert_eq!(paths, ["/session", "/session"]);
    }

    #[rstest::rstest]
    #[case("chrome")]
    #[case("chromium")]
    #[case("MicrosoftEdge")]
    #[tokio::test]
    async fn legacy_unknown_error_and_chromium_failure_share_one_retry_budget(
        #[case] browser: &str,
    ) {
        let (result, paths) = run(
            browser,
            vec![
                failure("unknown error", "Chrome failed to start"),
                failure("session not created", MISSING),
            ],
        )
        .await;
        assert!(matches!(&*result.unwrap_err(), WebDriverErrorInner::SessionNotCreated(_)));
        assert_eq!(paths, ["/session", "/session"]);
    }
}
