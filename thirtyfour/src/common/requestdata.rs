use std::fmt::Display;
use std::sync::Arc;

use crate::IntoArcStr;
use http::Method;

/// `RequestData` is a wrapper around the data required to make an HTTP request.
#[derive(Debug, Clone)]
pub struct RequestData {
    /// The request method.
    pub method: Method,
    /// The request URI.
    pub uri: Arc<str>,
    /// The request body.
    pub body: Option<serde_json::Value>,
}

impl RequestData {
    /// Create a new `RequestData` struct.
    pub fn new<S: IntoArcStr>(method: Method, uri: S) -> Self {
        RequestData {
            method,
            uri: uri.into(),
            body: None,
        }
    }

    /// Add a request body.
    #[must_use] 
    pub fn add_body(mut self, body: serde_json::Value) -> Self {
        self.body = Some(body);
        self
    }
}

impl Display for RequestData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(body) = &self.body {
            write!(
                f,
                "{} {} {}",
                self.method,
                self.uri,
                serde_json::to_string(body).unwrap_or_default()
            )
        } else {
            write!(f, "{} {}", self.method, self.uri)
        }
    }
}
