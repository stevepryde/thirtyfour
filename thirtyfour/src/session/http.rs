use std::sync::Arc;

use base64::Engine;
use bytes::Bytes;
use http::{
    header::{ACCEPT, AUTHORIZATION, CONNECTION, CONTENT_TYPE, USER_AGENT},
    HeaderValue, Request, Response,
};
use serde_json::Value;
use url::Url;

use crate::{
    common::config::WebDriverConfig,
    prelude::{WebDriverError, WebDriverResult},
    ElementId, ElementRef, RequestData, WebElement,
};

use super::handle::SessionHandle;

/// Enum representing the body of an HTTP request.
#[derive(Debug, Clone)]
pub enum Body<'a> {
    /// Empty body.
    Empty,
    /// JSON body.
    Json(&'a Value),
}

impl<'a, T: Into<Option<&'a Value>>> From<T> for Body<'a> {
    fn from(value: T) -> Self {
        match value.into() {
            None => Body::Empty,
            Some(value) => Body::Json(value),
        }
    }
}

/// Trait used to implement a HTTP client.
#[async_trait::async_trait]
pub trait HttpClient: Send + Sync + 'static {
    /// Send an HTTP request and return the response.
    async fn send(&self, request: Request<Body<'_>>) -> WebDriverResult<Response<Bytes>>;

    /// Make a new HttpClient, that **has no connection to the previous I/O drivers of self's runtime**
    /// this is used when dropping the webdriver but the old runtime has already shut down
    /// or couldn't prove its availability
    /// this isn't a simple clone,
    /// this new client needs to be able to run in a new runtime even if the old runtime has been destroyed
    //
    // needed for object safety
    #[allow(clippy::new_ret_no_self)]
    #[allow(clippy::wrong_self_convention)]
    async fn new(&self) -> Arc<dyn HttpClient>;
}

#[cfg(feature = "reqwest")]
#[async_trait::async_trait]
impl HttpClient for reqwest::Client {
    async fn send(&self, request: Request<Body<'_>>) -> WebDriverResult<Response<Bytes>> {
        let (parts, body) = request.into_parts();

        let mut req = self.request(parts.method, parts.uri.to_string());
        for (key, value) in parts.headers.into_iter() {
            let key = match key {
                Some(x) => x,
                None => continue,
            };
            req = req.header(key, value);
        }
        match body {
            Body::Empty => req = req.body(reqwest::Body::default()),
            Body::Json(json) => {
                req = req.json(json);
            }
        }

        let resp = req.send().await?;
        let status = resp.status();
        let mut builder = Response::builder();

        builder = builder.status(status);
        for (key, value) in resp.headers().iter() {
            builder = builder.header(key.clone(), value.clone());
        }

        let body = resp.bytes().await?;
        let body_str = String::from_utf8_lossy(&body).into_owned();
        let resp = builder
            .body(body)
            .map_err(|_| WebDriverError::UnknownResponse(status.as_u16(), body_str))?;
        Ok(resp)
    }

    async fn new(&self) -> Arc<dyn HttpClient> {
        Arc::new(self.clone())
    }
}

#[cfg(all(feature = "reqwest", test))]
mod tests {
    #[test]
    fn test_reqwest_clone_ok() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = rt.block_on(async move {
            let client = reqwest::Client::new();
            let resp = client.get("https://google.com/").send().await.unwrap();
            assert_eq!(resp.status(), 200);
            let _ = resp.text().await.unwrap();
            client
        });

        drop(rt);

        tokio::runtime::Runtime::new().unwrap().block_on(async move {
            let resp = client.get("https://google.com/").send().await.unwrap();
            assert_eq!(resp.status(), 200);
            let _ = resp.text().await.unwrap();
        });
    }
}

#[cfg(feature = "reqwest")]
pub(crate) fn create_reqwest_client(timeout: std::time::Duration) -> reqwest::Client {
    reqwest::Client::builder().timeout(timeout).build().expect("Failed to create reqwest client")
}

// Null client so that we can compile without the `reqwest` feature.
#[cfg(not(feature = "reqwest"))]
pub(crate) mod null_client {
    use super::*;

    pub struct NullHttpClient;

    #[async_trait::async_trait]
    impl HttpClient for NullHttpClient {
        async fn send(&self, _: Request<Body<'_>>) -> WebDriverResult<Response<Bytes>> {
            panic!("Either enable the `reqwest` feature or implement your own `HttpClient`")
        }

        async fn new(&self) -> Arc<dyn HttpClient> {
            Arc::new(NullHttpClient)
        }
    }

    pub(crate) fn create_null_client() -> NullHttpClient {
        NullHttpClient
    }
}

#[tracing::instrument(skip_all)]
pub(crate) async fn run_webdriver_cmd(
    client: &dyn HttpClient,
    request_data: &RequestData,
    server_url: &Url,
    config: &WebDriverConfig,
) -> WebDriverResult<CmdResponse> {
    tracing::debug!("webdriver request: {request_data}");
    let uri = server_url
        .join(&request_data.uri)
        .map_err(|e| WebDriverError::ParseError(format!("invalid url: {e}")))?;
    let mut builder = http::Request::builder()
        .method(request_data.method.clone())
        .uri(uri.as_str())
        .header(ACCEPT, HeaderValue::from_static("application/json"))
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json;charset=UTF-8"))
        .header(USER_AGENT, config.user_agent.clone());

    // Authentication.
    let url_username = server_url.username();
    let url_password = server_url.password();
    if !url_username.is_empty() || url_password.is_some() {
        let base64_string = base64::prelude::BASE64_STANDARD.encode(format!(
            "{}:{}",
            url_username,
            url_password.unwrap_or_default()
        ));
        builder = builder.header(AUTHORIZATION, format!("Basic {}", base64_string));
    }

    // Optional headers.
    if config.keep_alive {
        builder = builder.header(CONNECTION, "keep-alive");
    }

    let body = match &request_data.body {
        Some(body) => Body::from(body),
        None => Body::Empty,
    };

    let request = builder
        .body(body)
        .map_err(|e| WebDriverError::RequestFailed(format!("invalid request body: {e}")))?;
    let response = client.send(request).await?;
    let status = response.status().as_u16();
    let lossy_response = String::from_utf8_lossy(response.body());
    tracing::debug!("webdriver response: {status} {lossy_response}");
    match status {
        200..=399 => match serde_json::from_slice(response.body()) {
            Ok(v) => Ok(CmdResponse {
                body: v,
                status,
            }),
            Err(_) => Err(WebDriverError::parse(status, lossy_response.into_owned())),
        },
        _ => Err(WebDriverError::parse(status, lossy_response.into_owned())),
    }
}

/// Struct representing a WebDriver command response.
#[derive(Debug, Clone)]
pub struct CmdResponse {
    /// The body of the response.
    pub body: Value,
    /// The HTTP status code of the response.
    pub status: u16,
}

impl CmdResponse {
    /// Get the `value` field of the response as a JSON value.
    pub fn value_json(self) -> WebDriverResult<Value> {
        match self.body {
            Value::Object(mut x) => x
                .remove("value")
                .ok_or_else(|| WebDriverError::Json("Unexpected response body".to_string())),
            _ => Err(WebDriverError::Json("Unexpected response body".to_string())),
        }
    }

    /// Deserialize the value of the response.
    pub fn value<T: serde::de::DeserializeOwned>(self) -> WebDriverResult<T> {
        serde_json::from_value(self.value_json()?)
            .map_err(|e| WebDriverError::Json(format!("Failed to decode response body: {:?}", e)))
    }

    /// Deserialize the element from the response.
    pub fn element(self, handle: Arc<SessionHandle>) -> WebDriverResult<WebElement> {
        let elem_id: ElementRef = serde_json::from_value(self.value_json()?)?;
        Ok(WebElement::new(ElementId::from(elem_id.id()), handle))
    }

    /// Deserialize a list of elements from the response.
    pub fn elements(self, handle: Arc<SessionHandle>) -> WebDriverResult<Vec<WebElement>> {
        let values: Vec<ElementRef> = serde_json::from_value(self.value_json()?)?;
        Ok(values
            .into_iter()
            .map(|x| WebElement::new(ElementId::from(x.id()), handle.clone()))
            .collect())
    }
}
