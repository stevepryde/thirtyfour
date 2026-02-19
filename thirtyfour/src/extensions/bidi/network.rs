//! Network interception commands for `BiDi`.

use std::sync::Arc;

use super::commands::events::{
    BeforeRequestSentParams, ResponseCompletedParams, ResponseStartedParams,
};
use crate::error::WebDriverResult;
use crate::session::handle::SessionHandle;

use http::Method;
use serde::Serialize;
use serde_json::json;

use crate::{common::command::FormatRequestData, RequestData};

/// Network commands for `BiDi`.
#[derive(Debug)]
pub enum NetworkCommand {
    /// Add a network intercept for specified phases.
    AddIntercept {
        /// Phases at which to intercept requests.
        phases: Vec<InterceptPhase>,
    },
    /// Continue a blocked request with optional modifications.
    ContinueRequest {
        /// Request ID to continue.
        request: String,
        /// Optional new URL for the request.
        url: Option<String>,
        /// Optional new HTTP method.
        method: Option<String>,
        /// Optional new headers.
        headers: Option<Vec<Header>>,
        /// Optional new body.
        body: Option<Body>,
    },
    /// Continue a blocked response with optional modifications.
    ContinueResponse {
        /// Request ID to continue.
        request: String,
        /// Optional new status code.
        status_code: Option<u16>,
        /// Optional reason phrase.
        reason_phrase: Option<String>,
        /// Optional new headers.
        headers: Option<Vec<Header>>,
        /// Optional new body.
        body: Option<Body>,
    },
    /// Fail a blocked request.
    FailRequest {
        /// Request ID to fail.
        request: String,
    },
    /// Provide a mock response for a blocked request.
    ProvideResponse {
        /// Request ID to provide response for.
        request: String,
        /// Response status code.
        status_code: u16,
        /// Optional reason phrase.
        reason_phrase: Option<String>,
        /// Optional response headers.
        headers: Option<Vec<Header>>,
        /// Optional response body.
        body: Option<Body>,
    },
}

/// Phase at which to intercept network requests.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InterceptPhase {
    /// Intercept before request is sent.
    BeforeRequestSent,
    /// Intercept when response starts.
    ResponseStarted,
    /// Intercept when authentication is required.
    AuthRequired,
}

/// HTTP header for `BiDi` requests.
#[derive(Debug, Clone, Serialize)]
pub struct Header {
    /// Header name.
    pub name: String,
    /// Header value.
    #[serde(rename = "value")]
    pub value: HeaderValue,
}

/// Header value wrapper.
#[derive(Debug, Clone, Serialize)]
pub struct HeaderValue {
    /// Text value.
    pub text: String,
}

impl Header {
    /// Create a new header.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: HeaderValue {
                text: value.into(),
            },
        }
    }
}

/// Body content for `BiDi` requests/responses.
#[derive(Debug, Clone, Serialize)]
pub struct Body {
    /// Body type.
    #[serde(rename = "type")]
    pub body_type: String,
    /// Text content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Base64-encoded content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
}

impl Body {
    /// Create a text body.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            body_type: "string".to_string(),
            text: Some(text.into()),
            base64: None,
        }
    }

    /// Create a base64-encoded body.
    pub fn base64(data: impl Into<String>) -> Self {
        Self {
            body_type: "base64".to_string(),
            text: None,
            base64: Some(data.into()),
        }
    }
}

impl FormatRequestData for NetworkCommand {
    fn format_request(&self, session_id: &crate::SessionId) -> RequestData {
        match &self {
            NetworkCommand::AddIntercept {
                phases,
            } => RequestData::new(
                Method::POST,
                format!("/session/{}/bidi/network/addIntercept", session_id),
            )
            .add_body(json!({ "phases": phases })),
            NetworkCommand::ContinueRequest {
                request,
                url,
                method,
                headers,
                body,
            } => {
                let mut params = json!({ "request": request });
                if let Some(u) = url {
                    params["url"] = json!(u);
                }
                if let Some(m) = method {
                    params["method"] = json!(m);
                }
                if let Some(h) = headers {
                    params["headers"] = serde_json::to_value(h).unwrap();
                }
                if let Some(b) = body {
                    params["body"] = serde_json::to_value(b).unwrap();
                }
                RequestData::new(
                    Method::POST,
                    format!("/session/{}/bidi/network/continueRequest", session_id),
                )
                .add_body(params)
            }
            NetworkCommand::ContinueResponse {
                request,
                status_code,
                reason_phrase,
                headers,
                body,
            } => {
                let mut params = json!({ "request": request });
                if let Some(s) = status_code {
                    params["statusCode"] = json!(s);
                }
                if let Some(r) = reason_phrase {
                    params["reasonPhrase"] = json!(r);
                }
                if let Some(h) = headers {
                    params["headers"] = serde_json::to_value(h).unwrap();
                }
                if let Some(b) = body {
                    params["body"] = serde_json::to_value(b).unwrap();
                }
                RequestData::new(
                    Method::POST,
                    format!("/session/{}/bidi/network/continueResponse", session_id),
                )
                .add_body(params)
            }
            NetworkCommand::FailRequest {
                request,
            } => RequestData::new(
                Method::POST,
                format!("/session/{}/bidi/network/failRequest", session_id),
            )
            .add_body(json!({ "request": request })),
            NetworkCommand::ProvideResponse {
                request,
                status_code,
                reason_phrase,
                headers,
                body,
            } => {
                let mut params = json!({ "request": request, "statusCode": status_code });
                if let Some(r) = reason_phrase {
                    params["reasonPhrase"] = json!(r);
                }
                if let Some(h) = headers {
                    params["headers"] = serde_json::to_value(h).unwrap();
                }
                if let Some(b) = body {
                    params["body"] = serde_json::to_value(b).unwrap();
                }
                RequestData::new(
                    Method::POST,
                    format!("/session/{}/bidi/network/provideResponse", session_id),
                )
                .add_body(params)
            }
        }
    }
}

/// Network interception helper.
#[derive(Debug, Clone)]
pub struct Network {
    handle: Arc<SessionHandle>,
}

impl Network {
    /// Create a new Network helper.
    pub fn new(handle: Arc<SessionHandle>) -> Self {
        Self {
            handle,
        }
    }

    /// Add a network intercept for the specified phases.
    pub async fn add_intercept(&self, phases: Vec<InterceptPhase>) -> WebDriverResult<String> {
        let response = self
            .handle
            .cmd(NetworkCommand::AddIntercept {
                phases,
            })
            .await?;
        response.value()
    }

    /// Continue a blocked request without modifications.
    pub async fn continue_request(&self, request_id: &str) -> WebDriverResult<()> {
        self.handle
            .cmd(NetworkCommand::ContinueRequest {
                request: request_id.to_string(),
                url: None,
                method: None,
                headers: None,
                body: None,
            })
            .await?;
        Ok(())
    }

    /// Continue a blocked request with a modified URL.
    pub async fn continue_request_with_url(
        &self,
        request_id: &str,
        url: &str,
    ) -> WebDriverResult<()> {
        self.handle
            .cmd(NetworkCommand::ContinueRequest {
                request: request_id.to_string(),
                url: Some(url.to_string()),
                method: None,
                headers: None,
                body: None,
            })
            .await?;
        Ok(())
    }

    /// Continue a blocked request with a modified HTTP method.
    pub async fn continue_request_with_method(
        &self,
        request_id: &str,
        method: &str,
    ) -> WebDriverResult<()> {
        self.handle
            .cmd(NetworkCommand::ContinueRequest {
                request: request_id.to_string(),
                url: None,
                method: Some(method.to_string()),
                headers: None,
                body: None,
            })
            .await?;
        Ok(())
    }

    /// Continue a blocked request with modified headers.
    pub async fn continue_request_with_headers(
        &self,
        request_id: &str,
        headers: Vec<Header>,
    ) -> WebDriverResult<()> {
        self.handle
            .cmd(NetworkCommand::ContinueRequest {
                request: request_id.to_string(),
                url: None,
                method: None,
                headers: Some(headers),
                body: None,
            })
            .await?;
        Ok(())
    }

    /// Continue a blocked request with a modified body.
    pub async fn continue_request_with_body(
        &self,
        request_id: &str,
        body: Body,
    ) -> WebDriverResult<()> {
        self.handle
            .cmd(NetworkCommand::ContinueRequest {
                request: request_id.to_string(),
                url: None,
                method: None,
                headers: None,
                body: Some(body),
            })
            .await?;
        Ok(())
    }

    /// Continue a blocked request with multiple modifications.
    pub async fn continue_request_modified(
        &self,
        request_id: &str,
        url: Option<&str>,
        method: Option<&str>,
        headers: Option<Vec<Header>>,
        body: Option<Body>,
    ) -> WebDriverResult<()> {
        self.handle
            .cmd(NetworkCommand::ContinueRequest {
                request: request_id.to_string(),
                url: url.map(String::from),
                method: method.map(String::from),
                headers,
                body,
            })
            .await?;
        Ok(())
    }

    /// Continue a blocked response with optional modifications.
    pub async fn continue_response(
        &self,
        request_id: &str,
        status_code: Option<u16>,
        reason_phrase: Option<&str>,
        headers: Option<Vec<Header>>,
        body: Option<Body>,
    ) -> WebDriverResult<()> {
        self.handle
            .cmd(NetworkCommand::ContinueResponse {
                request: request_id.to_string(),
                status_code,
                reason_phrase: reason_phrase.map(String::from),
                headers,
                body,
            })
            .await?;
        Ok(())
    }

    /// Fail a blocked request.
    pub async fn fail_request(&self, request_id: &str) -> WebDriverResult<()> {
        self.handle
            .cmd(NetworkCommand::FailRequest {
                request: request_id.to_string(),
            })
            .await?;
        Ok(())
    }

    /// Provide a mock response for a blocked request.
    pub async fn provide_response(
        &self,
        request_id: &str,
        status_code: u16,
        reason_phrase: Option<&str>,
        headers: Option<Vec<Header>>,
        body: Option<Body>,
    ) -> WebDriverResult<()> {
        self.handle
            .cmd(NetworkCommand::ProvideResponse {
                request: request_id.to_string(),
                status_code,
                reason_phrase: reason_phrase.map(String::from),
                headers,
                body,
            })
            .await?;
        Ok(())
    }
}

/// An intercepted request.
#[derive(Debug, Clone)]
pub struct InterceptedRequest {
    network: Network,
    /// Request ID.
    pub request_id: String,
    /// Event parameters.
    pub params: BeforeRequestSentParams,
}

impl InterceptedRequest {
    /// Create a new intercepted request.
    pub fn new(network: Network, request_id: String, params: BeforeRequestSentParams) -> Self {
        Self {
            network,
            request_id,
            params,
        }
    }

    /// Get the request URL.
    pub fn url(&self) -> &str {
        &self.params.request.url
    }

    /// Get the HTTP method.
    pub fn method(&self) -> &str {
        &self.params.request.method
    }

    /// Check if the request is blocked.
    pub fn is_blocked(&self) -> bool {
        self.params.is_blocked
    }

    /// Continue the request without modifications.
    pub async fn continue_request(&self) -> WebDriverResult<()> {
        self.network.continue_request(&self.request_id).await
    }

    /// Continue the request with a modified URL.
    pub async fn continue_with_url(&self, url: &str) -> WebDriverResult<()> {
        self.network.continue_request_with_url(&self.request_id, url).await
    }

    /// Continue the request with a modified method.
    pub async fn continue_with_method(&self, method: &str) -> WebDriverResult<()> {
        self.network.continue_request_with_method(&self.request_id, method).await
    }

    /// Continue the request with modified headers.
    pub async fn continue_with_headers(&self, headers: Vec<Header>) -> WebDriverResult<()> {
        self.network.continue_request_with_headers(&self.request_id, headers).await
    }

    /// Continue the request with a modified body.
    pub async fn continue_with_body(&self, body: Body) -> WebDriverResult<()> {
        self.network.continue_request_with_body(&self.request_id, body).await
    }

    /// Continue the request with multiple modifications.
    pub async fn continue_modified(
        &self,
        url: Option<&str>,
        method: Option<&str>,
        headers: Option<Vec<Header>>,
        body: Option<Body>,
    ) -> WebDriverResult<()> {
        self.network.continue_request_modified(&self.request_id, url, method, headers, body).await
    }

    /// Fail the request.
    pub async fn fail(&self) -> WebDriverResult<()> {
        self.network.fail_request(&self.request_id).await
    }
}

/// An intercepted response.
#[derive(Debug, Clone)]
pub struct InterceptedResponse {
    network: Network,
    /// Request ID.
    pub request_id: String,
    /// Event parameters.
    pub params: ResponseStartedParams,
}

impl InterceptedResponse {
    /// Create a new intercepted response.
    pub fn new(network: Network, request_id: String, params: ResponseStartedParams) -> Self {
        Self {
            network,
            request_id,
            params,
        }
    }

    /// Get the response URL.
    pub fn url(&self) -> &str {
        &self.params.response.url
    }

    /// Get the HTTP status code.
    pub fn status(&self) -> u16 {
        self.params.response.status
    }

    /// Continue the response with optional modifications.
    pub async fn continue_response(
        &self,
        status_code: Option<u16>,
        reason_phrase: Option<&str>,
        headers: Option<Vec<Header>>,
        body: Option<Body>,
    ) -> WebDriverResult<()> {
        self.network
            .continue_response(&self.request_id, status_code, reason_phrase, headers, body)
            .await
    }

    /// Fail the response.
    pub async fn fail(&self) -> WebDriverResult<()> {
        self.network.fail_request(&self.request_id).await
    }
}

/// A completed response.
#[derive(Debug, Clone)]
pub struct CompletedResponse {
    /// Request ID.
    pub request_id: String,
    /// Event parameters.
    pub params: ResponseCompletedParams,
}

impl CompletedResponse {
    /// Create a new completed response.
    pub fn new(request_id: String, params: ResponseCompletedParams) -> Self {
        Self {
            request_id,
            params,
        }
    }

    /// Get the response URL.
    pub fn url(&self) -> &str {
        &self.params.response.url
    }

    /// Get the HTTP status code.
    pub fn status(&self) -> u16 {
        self.params.response.status
    }
}
