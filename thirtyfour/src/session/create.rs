use serde::Deserialize;
use url::Url;

use super::http::HttpClient;
use crate::error::WebDriverErrorInner;
use crate::{
    common::{
        command::{Command, FormatRequestData},
        config::WebDriverConfig,
    },
    prelude::WebDriverResult,
    session::http::run_webdriver_cmd,
    Capabilities, SessionId, TimeoutConfiguration,
};

#[derive(Debug, Deserialize)]
struct ConnectionData {
    #[serde(default, rename(deserialize = "sessionId"))]
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct ConnectionResp {
    #[serde(default, rename(deserialize = "sessionId"))]
    session_id: String,
    value: ConnectionData,
}

/// Start a new `WebDriver` session, returning the session id and the
/// capabilities JSON that was received back from the server.
///
/// # Errors
/// Returns an error if communication with the driver fails or if the session cannot be created.
pub async fn start_session<C: HttpClient>(
    http_client: &C,
    server_url: &Url,
    config: &WebDriverConfig,
    capabilities: Capabilities,
) -> WebDriverResult<SessionId> {
    let request_data = Command::NewSession(serde_json::Value::Object(capabilities))
        .format_request(&SessionId::null());

    let v = match run_webdriver_cmd(http_client, &request_data, server_url, config).await {
        Ok(x) => Ok(x),
        Err(e) => {
            // Selenium sometimes gives a bogus 500 error "Chrome failed to start".
            // Retry if we get a 500. If it happens twice in a row, then the second error
            // will be returned.
            if let WebDriverErrorInner::UnknownError(x) = &*e {
                if x.status == 500 {
                    run_webdriver_cmd(http_client, &request_data, server_url, config).await
                } else {
                    Err(e)
                }
            } else {
                Err(e)
            }
        }
    }?;

    let resp: ConnectionResp = serde_json::from_value(v.body)?;
    let data = resp.value;
    let session_id = SessionId::from(if resp.session_id.is_empty() {
        data.session_id
    } else {
        resp.session_id
    });

    // Set default timeouts.
    let request_data =
        Command::SetTimeouts(TimeoutConfiguration::default()).format_request(&session_id);
    run_webdriver_cmd(http_client, &request_data, server_url, config).await?;

    Ok(session_id)
}
