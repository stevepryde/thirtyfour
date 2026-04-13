use std::ops::Deref;
use std::sync::Arc;

use crate::common::config::WebDriverConfig;
use crate::error::WebDriverResult;
use crate::prelude::WebDriverError;
use crate::session::create::start_session;
use crate::session::handle::SessionHandle;
#[cfg(feature = "reqwest")]
use crate::session::http::create_reqwest_client;
use crate::session::http::HttpClient;
use crate::Capabilities;

/// The `WebDriver` struct encapsulates an async Selenium WebDriver browser
/// session.
///
/// # Example:
/// ```no_run
/// use thirtyfour::prelude::*;
/// # use thirtyfour::support::block_on;
///
/// # fn main() -> color_eyre::Result<()> {
/// #     block_on(async {
/// let server_url = "http://localhost:4444";
/// let caps = DesiredCapabilities::firefox();
/// start_webdriver_process(server_url, &caps, true)?;
/// let driver = WebDriver::new(server_url, caps).await?;
/// driver.goto("https://www.rust-lang.org/").await?;
/// // Always remember to close the session.
/// driver.quit().await?;
/// #         Ok(())
/// #     })
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct WebDriver {
    /// The underlying session handle.
    pub handle: Arc<SessionHandle>,
}

#[derive(Debug, thiserror::Error)]
#[error("Webdriver has already quit, can't leak an already quit driver")]
pub struct AlreadyQuit(pub(crate) ());

impl WebDriver {
    /// Create a new WebDriver as follows:
    ///
    /// # Example
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// let caps = DesiredCapabilities::firefox();
    /// let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    ///
    /// ## Using Selenium Server
    /// - For selenium 3.x, you need to also add "/wd/hub/session" to the end of the url
    ///   (e.g. "http://localhost:4444/wd/hub/session")
    /// - For selenium 4.x and later, no path should be needed on the url.
    ///
    /// ## Troubleshooting
    ///
    /// - If the webdriver appears to freeze or give no response, please check that the
    ///   capabilities' object is of the correct type for that webdriver.
    pub async fn new<S, C>(server_url: S, capabilities: C) -> WebDriverResult<Self>
    where
        S: Into<String>,
        C: Into<Capabilities>,
    {
        Self::new_with_config(server_url, capabilities, WebDriverConfig::default()).await
    }

    /// Create a new `WebDriver` with the specified `WebDriverConfig`.
    ///
    /// Use `WebDriverConfig::builder().build()` to construct the config.
    pub async fn new_with_config<S, C>(
        server_url: S,
        capabilities: C,
        config: WebDriverConfig,
    ) -> WebDriverResult<Self>
    where
        S: Into<String>,
        C: Into<Capabilities>,
    {
        // TODO: create builder
        #[cfg(feature = "reqwest")]
        let client = create_reqwest_client(config.reqwest_timeout, config.basic_auth.clone());
        #[cfg(not(feature = "reqwest"))]
        let client = crate::session::http::null_client::create_null_client();
        Self::new_with_config_and_client(server_url, capabilities, config, client).await
    }

    /// Create a new `WebDriver` with the specified `WebDriverConfig`.
    ///
    /// Use `WebDriverConfig::builder().build()` to construct the config.
    pub async fn new_with_config_and_client<S, C>(
        server_url: S,
        capabilities: C,
        config: WebDriverConfig,
        client: impl HttpClient,
    ) -> WebDriverResult<Self>
    where
        S: Into<String>,
        C: Into<Capabilities>,
    {
        let capabilities = capabilities.into();
        let server_url = server_url
            .into()
            .parse()
            .map_err(|e| WebDriverError::ParseError(format!("invalid url: {e}")))?;

        let client = Arc::new(client);
        let (session_id, ws_url) =
            start_session(client.as_ref(), &server_url, &config, capabilities).await?;

        let handle =
            SessionHandle::new_with_config(client, server_url, session_id, ws_url, config)?;
        Ok(Self {
            handle: Arc::new(handle),
        })
    }

    /// Clone this `WebDriver` keeping the session handle, but supplying a new `WebDriverConfig`.
    ///
    /// This still uses the same underlying client, and still controls the same browser
    /// session, but uses a different `WebDriverConfig` for this instance.
    ///
    /// This is useful in cases where you want to specify a custom poller configuration (or
    /// some other configuration option) for only one instance of `WebDriver`.
    pub fn clone_with_config(&self, config: WebDriverConfig) -> Self {
        Self {
            handle: Arc::new(self.handle.clone_with_config(config)),
        }
    }

    /// End the webdriver session and close the browser.
    ///
    /// **NOTE:** Although `WebDriver` does close when all instances go out of scope.
    ///           When this happens it blocks the current executor,
    ///           therefore, if you know when a webdriver is no longer used/required
    ///           call this method and await it to more or less "asynchronously drop" it
    ///           this also allows you to catch errors during quitting,
    ///           and possibly panic or report back to the user
    pub async fn quit(self) -> WebDriverResult<()> {
        self.handle.quit().await
    }

    /// Leak the webdriver session and prevent it from being closed,
    /// use this if you don't want your driver to automatically close
    pub fn leak(self) -> Result<(), AlreadyQuit> {
        self.handle.leak()
    }

    #[cfg(feature = "bidi")]
    /// Connect to the WebDriver BiDi channel.
    ///
    /// Requires the browser to have been started with BiDi capabilities enabled.
    /// For Chrome, set `"webSocketUrl": true` in capabilities.
    /// For Firefox, BiDi is enabled by default in supported versions.
    ///
    /// The connection method depends on [`WebDriverConfig::bidi_connection_type`]:
    /// - [`BidiConnectionType::UseHubProvided`] (default): Uses the WebSocket URL
    ///   returned by the browser during session creation.
    /// - [`BidiConnectionType::DeriveFromServerUrl`]: Derives the BiDi WebSocket URL
    ///   from the server URL. This is useful when the browser doesn't provide a
    ///   `webSocketUrl` but supports BiDi on a well-known path.
    ///
    /// **Important:** After connecting, you must spawn the dispatch loop:
    /// ```ignore
    /// let mut bidi = driver.bidi_connect().await?;
    /// tokio::spawn(bidi.dispatch_future().expect("dispatch already started"));
    /// ```
    ///
    /// # TLS Support
    ///
    /// This convenience method automatically installs a crypto provider for `wss://`
    /// (TLS) connections. If you need custom TLS configuration, use
    /// [`Self::bidi_connect_with_builder`] instead.
    ///
    /// # Errors
    ///
    /// Returns `WebDriverError::BiDi` if:
    /// - No WebSocket URL is available and [`BidiConnectionType::UseHubProvided`] is set
    /// - The browser doesn't support BiDi from server URL when
    ///   [`BidiConnectionType::DeriveFromServerUrl`] is set
    /// - The WebSocket connection fails
    pub async fn bidi_connect(
        &self,
    ) -> crate::error::WebDriverResult<crate::extensions::bidi::BiDiSession> {
        let mut builder = crate::extensions::bidi::BiDiSessionBuilder::new();

        // Configure auth from WebDriver config
        if let Some(ref auth) = self.handle.config().basic_auth {
            builder = builder.basic_auth(&auth.username, &auth.password);
        }

        // Use connect_with_driver which respects bidi_connection_type and handles URL resolution
        builder.connect_with_driver(self).await
    }

    #[cfg(feature = "bidi")]
    /// Connect to BiDi using a builder for custom configuration.
    ///
    /// Use this method instead of [`Self::bidi_connect`] when you need:
    /// - **TLS/SSL support** for `wss://` connections (call `install_crypto_provider()`)
    /// - **HTTP Basic Authentication** (call `basic_auth(username, password)`)
    /// - **Custom timeouts** (call `command_timeout()`)
    /// - **Custom event channel capacity** (call `event_channel_capacity()`)
    ///
    /// **Important:** After connecting, you must spawn the dispatch loop:
    /// ```ignore
    /// let mut bidi = BiDiSessionBuilder::new()
    ///     .install_crypto_provider()
    ///     .connect_with_driver(driver)
    ///     .await?;
    /// tokio::spawn(bidi.dispatch_future().expect("dispatch already started"));
    /// ```
    ///
    /// # Example with TLS
    ///
    /// ```no_run
    /// # use std::time::Duration;
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::BiDiSessionBuilder;
    /// # async fn example(driver: &WebDriver) -> WebDriverResult<()> {
    /// let mut bidi = BiDiSessionBuilder::new()
    ///     .install_crypto_provider()
    ///     .connect_with_driver(driver)
    ///     .await?;
    /// tokio::spawn(bidi.dispatch_future().expect("dispatch already started"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Example with Basic Auth
    ///
    /// ```no_run
    /// # use std::time::Duration;
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::BiDiSessionBuilder;
    /// # async fn example(driver: &WebDriver) -> WebDriverResult<()> {
    /// let mut bidi = BiDiSessionBuilder::new()
    ///     .install_crypto_provider()
    ///     .basic_auth("user", "pass")
    ///     .connect_with_driver(driver)
    ///     .await?;
    /// tokio::spawn(bidi.dispatch_future().expect("dispatch already started"));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bidi_connect_with_builder(
        &self,
        builder: crate::extensions::bidi::BiDiSessionBuilder,
    ) -> crate::error::WebDriverResult<crate::extensions::bidi::BiDiSession> {
        // Get the WebSocket URL, respecting custom_url_base, use_server_url flag, and config
        let ws_url = if let Some(ref base) = builder.custom_url_base {
            // Use custom base + session path (takes absolute precedence)
            let sid = self.handle.session_id();
            format!("{}/session/{}/se/bidi", base.trim_end_matches('/'), sid)
        } else if builder.use_server_url {
            self.handle.derive_bidi_ws_url()
        } else {
            match self.handle.config().bidi_connection_type {
                crate::common::config::BidiConnectionType::DeriveFromServerUrl => {
                    self.handle.derive_bidi_ws_url()
                }
                crate::common::config::BidiConnectionType::UseHubProvided => {
                    self.handle
                        .websocket_url
                        .as_deref()
                        .ok_or_else(|| {
                            crate::prelude::WebDriverError::BiDi(
                                "No webSocketUrl in session capabilities and unable to derive from server URL. \
                             Enable BiDi in your browser capabilities \
                             (e.g., for Chrome: set 'webSocketUrl: true'), \
                             or configure BidiConnectionType::DeriveFromServerUrl in WebDriverConfig."
                                    .to_string(),
                            )
                        })?.to_string()
                }
            }
        };

        builder.connect(&ws_url).await
    }

    #[cfg(feature = "bidi")]
    /// Connect to the WebDriver BiDi channel using a derived server URL.
    ///
    /// This method derives the BiDi WebSocket URL from the server URL by converting:
    /// - `http://server:port` → `ws://server:port`
    /// - `https://server:port` → `wss://server:port`
    ///
    /// Use this method when:
    /// - The browser/Selenium grid doesn't return a `webSocketUrl` in capabilities
    /// - You want to explicitly connect via the server URL rather than hub-provided one
    ///
    /// **Important:** After connecting, you must spawn the dispatch loop:
    /// ```ignore
    /// let mut bidi = driver.bidi_connect_with_server_url().await?;
    /// tokio::spawn(bidi.dispatch_future().expect("dispatch already started"));
    /// ```
    ///
    /// # TLS Support
    ///
    /// This method automatically installs a crypto provider for `wss://` connections.
    /// If your infrastructure requires additional TLS configuration, use
    /// [`Self::bidi_connect_with_builder`] instead.
    ///
    /// # Basic Auth
    ///
    /// This method applies the HTTP Basic Authentication from the WebDriver config
    /// (if configured). Use [`Self::bidi_connect_with_builder`] to override with
    /// custom credentials.
    pub async fn bidi_connect_with_server_url(
        &self,
    ) -> crate::error::WebDriverResult<crate::extensions::bidi::BiDiSession> {
        let ws_url = self.handle.derive_bidi_ws_url();

        let mut builder = crate::extensions::bidi::BiDiSessionBuilder::new();
        if ws_url.starts_with("wss://") {
            builder = builder.install_crypto_provider();
        }

        if let Some(ref auth) = self.handle.config().basic_auth {
            builder = builder.basic_auth(&auth.username, &auth.password);
        }

        builder.connect(&ws_url).await
    }
}

/// The Deref implementation allows the WebDriver to "fall back" to SessionHandle and
/// exposes all the methods there without requiring us to use an async_trait.
/// See documentation at the top of this module for more details on the design.
impl Deref for WebDriver {
    type Target = Arc<SessionHandle>;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}
