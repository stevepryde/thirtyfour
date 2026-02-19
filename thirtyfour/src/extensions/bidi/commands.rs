//! BiDi commands and event types.

use http::Method;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;

use crate::{common::command::FormatRequestData, error::WebDriverResult, RequestData};

/// BiDi commands for WebDriver session.
#[derive(Debug)]
pub enum BiDiCommand {
    /// Subscribe to events.
    Subscribe {
        /// Events to subscribe to.
        events: Vec<String>,
    },
    /// Unsubscribe from events.
    Unsubscribe {
        /// Events to unsubscribe from.
        events: Vec<String>,
    },
    /// Execute a BiDi command.
    ExecuteCommand {
        /// Command method name.
        method: String,
        /// Command parameters.
        params: serde_json::Value,
    },
}

impl FormatRequestData for BiDiCommand {
    fn format_request(&self, session_id: &crate::SessionId) -> RequestData {
        match &self {
            BiDiCommand::Subscribe {
                events,
            } => RequestData::new(
                Method::POST,
                format!("/session/{}/bidi/session/subscribe", session_id),
            )
            .add_body(json!({ "events": events })),
            BiDiCommand::Unsubscribe {
                events,
            } => RequestData::new(
                Method::POST,
                format!("/session/{}/bidi/session/unsubscribe", session_id),
            )
            .add_body(json!({ "events": events })),
            BiDiCommand::ExecuteCommand {
                method,
                params,
            } => RequestData::new(
                Method::POST,
                format!("/session/{}/bidi/{}", session_id, method.replace('.', "/")),
            )
            .add_body(params.clone()),
        }
    }
}

/// WebSocket command to send to `BiDi`.
#[derive(Debug, Clone, Serialize)]
pub struct WebSocketCommand {
    /// Command ID.
    pub id: u64,
    /// Command method.
    pub method: String,
    /// Command parameters.
    pub params: serde_json::Value,
}

impl WebSocketCommand {
    /// Create a new WebSocket command.
    pub fn new(id: u64, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            id,
            method: method.into(),
            params,
        }
    }

    /// Create a subscribe command.
    pub fn subscribe(id: u64, events: &[&str]) -> Self {
        Self::new(id, "session.subscribe", json!({ "events": events }))
    }

    /// Create an unsubscribe command.
    pub fn unsubscribe(id: u64, events: &[&str]) -> Self {
        Self::new(id, "session.unsubscribe", json!({ "events": events }))
    }

    /// Serialize the command to JSON.
    pub fn to_json(&self) -> WebDriverResult<String> {
        serde_json::to_string(self).map_err(|e| {
            crate::error::WebDriverError::Json(format!("Failed to serialize command: {e}"))
        })
    }
}

/// WebSocket response from `BiDi`.
#[derive(Debug, Clone, Deserialize)]
pub struct WebSocketResponse {
    /// Command ID that this response corresponds to.
    pub id: u64,
    /// Result of the command, if successful.
    pub result: Option<serde_json::Value>,
    /// Error, if the command failed.
    pub error: Option<BidiError>,
}

/// BiDi error response.
#[derive(Debug, Clone, Deserialize)]
pub struct BidiError {
    /// Error code.
    pub code: i64,
    /// Error message.
    pub message: String,
}

/// WebSocket event received from `BiDi`.
#[derive(Debug, Clone, Deserialize)]
pub struct WebSocketEvent {
    /// Event method name.
    pub method: String,
    /// Event parameters.
    pub params: serde_json::Value,
}

impl WebSocketEvent {
    /// Parse an event from JSON text.
    pub fn parse(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }

    /// Get the event method name.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Get the event parameters.
    pub fn params(&self) -> &serde_json::Value {
        &self.params
    }

    /// Parse the event parameters as a specific type.
    pub fn params_as<T: DeserializeOwned>(&self) -> WebDriverResult<T> {
        serde_json::from_value(self.params.clone())
            .map_err(|e| crate::error::WebDriverError::Json(format!("Failed to parse params: {e}")))
    }
}

/// `BiDi` event types and parameter structures.
pub mod events {
    use serde::Deserialize;

    /// Parameters for the `network.beforeRequestSent` event.
    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct BeforeRequestSentParams {
        /// Browsing context ID.
        pub context: Option<String>,
        /// Whether the request is blocked.
        pub is_blocked: bool,
        /// Navigation ID.
        pub navigation: Option<String>,
        /// Number of redirects.
        pub redirect_count: u64,
        /// Request data.
        pub request: RequestData,
        /// Timestamp.
        pub timestamp: f64,
        /// Intercept IDs.
        pub intercepts: Option<Vec<String>>,
    }

    /// Parameters for the `network.responseStarted` event.
    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ResponseStartedParams {
        /// Browsing context ID.
        pub context: Option<String>,
        /// Navigation ID.
        pub navigation: Option<String>,
        /// Number of redirects.
        pub redirect_count: u64,
        /// Request data.
        pub request: RequestData,
        /// Response data.
        pub response: ResponseData,
        /// Timestamp.
        pub timestamp: f64,
    }

    /// Parameters for the `network.responseCompleted` event.
    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ResponseCompletedParams {
        /// Browsing context ID.
        pub context: Option<String>,
        /// Navigation ID.
        pub navigation: Option<String>,
        /// Number of redirects.
        pub redirect_count: u64,
        /// Request data.
        pub request: RequestData,
        /// Response data.
        pub response: ResponseData,
        /// Timestamp.
        pub timestamp: f64,
    }

    /// HTTP request data.
    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RequestData {
        /// Request URL.
        pub url: String,
        /// HTTP method.
        pub method: String,
        /// Request headers.
        pub headers: Vec<Header>,
        /// Request cookies.
        pub cookies: Vec<Cookie>,
        /// Headers size in bytes.
        pub headers_size: Option<u64>,
        /// Body size in bytes.
        pub body_size: Option<u64>,
        /// Request timings.
        pub timings: Option<Timings>,
    }

    /// HTTP response data.
    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ResponseData {
        /// Response URL.
        pub url: String,
        /// Protocol version.
        pub protocol: Option<String>,
        /// HTTP status code.
        pub status: u16,
        /// HTTP status text.
        pub status_text: Option<String>,
        /// Whether response came from cache.
        pub from_cache: Option<bool>,
        /// Response headers.
        pub headers: Vec<Header>,
        /// Response cookies.
        pub cookies: Vec<Cookie>,
        /// Headers size in bytes.
        pub headers_size: Option<u64>,
        /// Body size in bytes.
        pub body_size: Option<u64>,
        /// Response content info.
        pub content: Option<ContentData>,
    }

    /// HTTP header.
    #[derive(Debug, Clone, Deserialize)]
    pub struct Header {
        /// Header name.
        pub name: String,
        /// Header value.
        pub value: HeaderValue,
    }

    /// HTTP header value.
    #[derive(Debug, Clone, Deserialize)]
    pub struct HeaderValue {
        /// Text value.
        #[serde(default)]
        pub text: Option<String>,
        /// Binary value (base64).
        #[serde(default)]
        pub binary: Option<String>,
    }

    /// HTTP cookie.
    #[derive(Debug, Clone, Deserialize)]
    pub struct Cookie {
        /// Cookie name.
        pub name: String,
        /// Cookie value.
        pub value: CookieValue,
        /// Cookie domain.
        pub domain: String,
        /// Cookie path.
        pub path: String,
        /// Cookie expiry time.
        #[serde(default)]
        pub expires: Option<f64>,
        /// Cookie size.
        #[serde(default)]
        pub size: Option<u64>,
        /// Whether cookie is HTTP-only.
        #[serde(default)]
        pub http_only: Option<bool>,
        /// Whether cookie is secure.
        #[serde(default)]
        pub secure: Option<bool>,
        /// Same-site policy.
        #[serde(default)]
        pub same_site: Option<String>,
    }

    /// Cookie value.
    #[derive(Debug, Clone, Deserialize)]
    pub struct CookieValue {
        /// Text value.
        #[serde(default)]
        pub text: Option<String>,
        /// Binary value (base64).
        #[serde(default)]
        pub binary: Option<String>,
    }

    /// Request/response timings.
    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Timings {
        /// Request start time.
        pub request_time: f64,
        /// Redirect start time.
        pub redirect_start: Option<f64>,
        /// Redirect end time.
        pub redirect_end: Option<f64>,
        /// Fetch start time.
        pub fetch_start: Option<f64>,
        /// DNS lookup start time.
        pub dns_start: Option<f64>,
        /// DNS lookup end time.
        pub dns_end: Option<f64>,
        /// Connection start time.
        pub connect_start: Option<f64>,
        /// Connection end time.
        pub connect_end: Option<f64>,
        /// TLS handshake start time.
        pub tls_start: Option<f64>,
        /// TLS handshake end time.
        pub tls_end: Option<f64>,
        /// Request start time.
        pub request_start: Option<f64>,
        /// Response start time.
        pub response_start: Option<f64>,
        /// Response end time.
        pub response_end: Option<f64>,
    }

    /// Response content data.
    #[derive(Debug, Clone, Deserialize)]
    pub struct ContentData {
        /// Content type.
        #[serde(rename = "type")]
        pub content_type: Option<String>,
        /// Content size.
        pub size: Option<u64>,
    }

    /// Event name for `network.beforeRequestSent`.
    pub const BEFORE_REQUEST_SENT: &str = "network.beforeRequestSent";
    /// Event name for `network.responseStarted`.
    pub const RESPONSE_STARTED: &str = "network.responseStarted";
    /// Event name for `network.responseCompleted`.
    pub const RESPONSE_COMPLETED: &str = "network.responseCompleted";
    /// Event name for `network.fetchError`.
    pub const FETCH_ERROR: &str = "network.fetchError";
    /// Event name for `log.entryAdded`.
    pub const LOG_ENTRY_ADDED: &str = "log.entryAdded";
    /// Event name for `cdp.consoleEntryAdded`.
    pub const CONSOLE_ENTRY_ADDED: &str = "cdp.consoleEntryAdded";
}

/// `BiDi` domain helpers.
pub mod domains {
    /// Network domain events.
    pub mod network {
        /// Domain name.
        pub const DOMAIN: &str = "network";

        /// All network events.
        pub const EVENTS: &[&str] = &[
            "network.beforeRequestSent",
            "network.responseStarted",
            "network.responseCompleted",
            "network.fetchError",
        ];

        /// Get all network events as a vector.
        pub fn events() -> Vec<&'static str> {
            EVENTS.to_vec()
        }
    }

    /// Log domain events.
    pub mod log {
        /// Domain name.
        pub const DOMAIN: &str = "log";

        /// All log events.
        pub const EVENTS: &[&str] = &["log.entryAdded"];

        /// Get all log events as a vector.
        pub fn events() -> Vec<&'static str> {
            EVENTS.to_vec()
        }
    }

    /// CDP domain events.
    pub mod cdp {
        /// Domain name.
        pub const DOMAIN: &str = "cdp";

        /// All CDP events.
        pub const EVENTS: &[&str] = &["cdp.consoleEntryAdded"];

        /// Get all CDP events as a vector.
        pub fn events() -> Vec<&'static str> {
            EVENTS.to_vec()
        }
    }
}
