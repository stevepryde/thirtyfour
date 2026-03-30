use crate::error::WebDriverError;
use crate::{
    extensions::query::{ElementPollerWithTimeout, IntoElementPoller},
    prelude::WebDriverResult,
};
use const_format::formatcp;
use http::HeaderValue;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for BiDi connection URL derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BidiConnectionType {
    /// Use the hub-provided WebSocket URL directly from the newSession response.
    #[default]
    UseHubProvided,
    /// Derive the BiDi connection URL from the server URL using a well-known port.
    DeriveFromServerUrl,
}

/// HTTP Basic Auth credentials for Selenium grid authentication.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct BasicAuth {
    /// The username for authentication.
    pub username: String,
    /// The password for authentication.
    pub password: String,
}

impl BasicAuth {
    /// Create a new `BasicAuth` with the specified credentials.
    #[must_use]
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

impl std::fmt::Debug for BasicAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BasicAuth")
            .field("username", &self.username)
            .field("password", &"***REDACTED***")
            .finish()
    }
}

/// Configuration options used by a `WebDriver` instance and the related `SessionHandle`.
///
/// The configuration of a `WebDriver` will be shared by all elements found via that instance.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct WebDriverConfig {
    /// If true, send "Connection: keep-alive" header with all requests.
    pub keep_alive: bool,
    /// The default poller to use when performing element queries or waits.
    pub poller: Arc<dyn IntoElementPoller + Send + Sync>,
    /// The user agent to use when sending commands to the webdriver server.
    pub user_agent: HeaderValue,
    /// The timeout duration for reqwest client requests.
    pub reqwest_timeout: Duration,
    /// Configuration for BiDi connection URL derivation.
    pub bidi_connection_type: BidiConnectionType,
    /// HTTP Basic Auth credentials for Selenium grid authentication.
    pub basic_auth: Option<BasicAuth>,
}

impl Default for WebDriverConfig {
    fn default() -> Self {
        Self::builder().build().expect("default values failed")
    }
}

impl WebDriverConfig {
    /// Create new `WebDriverConfigBuilder`.
    #[must_use]
    pub fn builder() -> WebDriverConfigBuilder {
        WebDriverConfigBuilder::new()
    }

    /// The default user agent.
    pub const DEFAULT_USER_AGENT: HeaderValue = {
        //noinspection RsReplaceMatchExpr
        const RUST_VER: &str = match option_env!("RUSTC_VERSION") {
            Some(ver) => ver,
            None => "unknown",
        };

        //noinspection RsCompileErrorMacro
        const HEADER: &str = formatcp!(
            "thirtyfour/{} (rust/{}; {})",
            crate::VERSION,
            RUST_VER,
            std::env::consts::OS
        );

        HeaderValue::from_static(HEADER)
    };

    /// Get the default user agent.
    #[deprecated(
        since = "0.34.1",
        note = "This associated function is now a constant `WebDriverConfig::DEFAULT_USER_AGENT`"
    )]
    #[must_use]
    pub fn default_user_agent() -> HeaderValue {
        Self::DEFAULT_USER_AGENT
    }
}

/// Builder for `WebDriverConfig`.
#[derive(Debug)]
pub struct WebDriverConfigBuilder {
    keep_alive: bool,
    poller: Option<Arc<dyn IntoElementPoller + Send + Sync>>,
    user_agent: Option<WebDriverResult<HeaderValue>>,
    reqwest_timeout: Duration,
    bidi_connection_type: BidiConnectionType,
    basic_auth: Option<BasicAuth>,
}

impl Default for WebDriverConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl WebDriverConfigBuilder {
    /// Create a new `WebDriverConfigBuilder`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            keep_alive: true,
            poller: None,
            user_agent: None,
            reqwest_timeout: Duration::from_secs(120),
            bidi_connection_type: BidiConnectionType::default(),
            basic_auth: None,
        }
    }

    /// Set `keep_alive` option.
    #[must_use]
    pub fn keep_alive(mut self, keep_alive: bool) -> Self {
        self.keep_alive = keep_alive;
        self
    }

    /// Set the specified element poller.
    #[must_use]
    pub fn poller(mut self, poller: Arc<dyn IntoElementPoller + Send + Sync>) -> Self {
        self.poller = Some(poller);
        self
    }

    /// Set the user agent.
    #[must_use]
    pub fn user_agent<V>(mut self, user_agent: V) -> Self
    where
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<WebDriverError>,
    {
        self.user_agent = Some(user_agent.try_into().map_err(Into::into));
        self
    }

    /// Set the reqwest timeout.
    #[must_use]
    pub fn reqwest_timeout(mut self, timeout: Duration) -> Self {
        self.reqwest_timeout = timeout;
        self
    }

    /// Set the BiDi connection type for URL derivation.
    #[must_use]
    pub fn bidi_connection_type(mut self, bidi_connection_type: BidiConnectionType) -> Self {
        self.bidi_connection_type = bidi_connection_type;
        self
    }

    /// Set the HTTP Basic Auth credentials for Selenium grid authentication.
    ///
    /// Accepts a tuple of (username, password) which is converted internally to `BasicAuth`.
    #[must_use]
    pub fn basic_auth<U, P>(mut self, username: U, password: P) -> Self
    where
        U: Into<String>,
        P: Into<String>,
    {
        self.basic_auth = Some(BasicAuth::new(username, password));
        self
    }

    /// Set the HTTP Basic Auth credentials from an optional tuple.
    ///
    /// Use this when you want to conditionally set auth (passing None to disable).
    #[must_use]
    pub fn basic_auth_option<U, P>(mut self, credentials: Option<(U, P)>) -> Self
    where
        U: Into<String>,
        P: Into<String>,
    {
        self.basic_auth = credentials.map(|(u, p)| BasicAuth::new(u, p));
        self
    }

    /// Build `WebDriverConfig` using builder options.
    ///
    /// # Errors
    ///
    /// Returns a `WebDriverError` if user agent conversion fails.
    pub fn build(self) -> WebDriverResult<WebDriverConfig> {
        Ok(WebDriverConfig {
            keep_alive: self.keep_alive,
            poller: self.poller.unwrap_or_else(|| Arc::new(ElementPollerWithTimeout::default())),
            user_agent: self.user_agent.transpose()?.unwrap_or(WebDriverConfig::DEFAULT_USER_AGENT),
            reqwest_timeout: self.reqwest_timeout,
            bidi_connection_type: self.bidi_connection_type,
            basic_auth: self.basic_auth,
        })
    }
}
