use async_trait::async_trait;
use log::error;
use serde::Serialize;
use serde_json::Value;
use std::marker::PhantomData;

use crate::common::config::WebDriverConfig;
use crate::http::connection_async::{HttpClientCreateParams, WebDriverHttpClientAsync};

use crate::runtime::imports::{HttpClientAsync, Mutex};
use crate::webdrivercommands::{start_session, WebDriverCommands};
use crate::{
    common::command::Command, error::WebDriverResult, session::WebDriverSession,
    DesiredCapabilities, SessionId,
};
use futures::executor::block_on;
use std::sync::Arc;
use std::time::Duration;

/// The WebDriver struct represents a browser session.
///
/// For full documentation of all WebDriver methods,
/// see the [WebDriverCommands](trait.WebDriverCommands.html) trait.
pub type WebDriver = GenericWebDriver<HttpClientAsync>;

/// **NOTE:** For WebDriver method documentation,
/// see the [WebDriverCommands](trait.WebDriverCommands.html) trait.
///
/// The `thirtyfour` crate uses a generic struct that implements the
/// `WebDriverCommands` trait. The generic struct is then implemented for
/// a specific HTTP client. This enables `thirtyfour` to support different
/// HTTP clients in order to target different async runtimes. If you do not
/// require a specific async runtime or if you are using tokio then the
/// default will work fine.
///
/// The `GenericWebDriver` struct encapsulates an async Selenium WebDriver browser
/// session. For the sync driver, see
/// [sync::GenericWebDriver](sync/struct.GenericWebDriver.html).
///
/// # Example:
/// ```rust
/// use thirtyfour::prelude::*;
/// use thirtyfour::support::block_on;
///
/// fn main() -> WebDriverResult<()> {
///     block_on(async {
///         let caps = DesiredCapabilities::chrome();
///         let driver = WebDriver::new("http://localhost:4444/wd/hub", &caps).await?;
///         driver.get("http://webappdemo").await?;
///         Ok(())
///     })
/// }
/// ```
#[derive(Debug)]
pub struct GenericWebDriver<T: WebDriverHttpClientAsync + 'static> {
    pub session: WebDriverSession,
    http_create_params: HttpClientCreateParams,
    capabilities: Value,
    quit_on_drop: bool,
    phantom: PhantomData<T>,
}

impl<T: 'static> GenericWebDriver<T>
where
    T: WebDriverHttpClientAsync,
{
    /// The GenericWebDriver struct is not intended to be created directly.
    ///
    /// Instead you would use the WebDriver struct, which wires up the
    /// GenericWebDriver with a HTTP client for making requests to the
    /// WebDriver server.
    ///
    /// Create a new WebDriver as follows:
    ///
    /// # Example
    /// ```rust
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// let caps = DesiredCapabilities::chrome();
    /// let driver = WebDriver::new("http://localhost:4444/wd/hub", &caps).await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn new<C>(server_url: &str, capabilities: C) -> WebDriverResult<Self>
    where
        C: Serialize,
    {
        Self::new_with_timeout(server_url, capabilities, None).await
    }

    /// Creates a new GenericWebDriver just like the `new` function. Allows a
    /// configurable timeout for all HTTP requests including the session creation.
    ///
    /// Create a new WebDriver as follows:
    ///
    /// # Example
    /// ```rust
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// # use std::time::Duration;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// let caps = DesiredCapabilities::chrome();
    /// let driver = WebDriver::new_with_timeout("http://localhost:4444/wd/hub", &caps, Some(Duration::from_secs(120))).await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn new_with_timeout<C>(
        server_url: &str,
        capabilities: C,
        timeout: Option<Duration>,
    ) -> WebDriverResult<Self>
    where
        C: Serialize,
    {
        let params = HttpClientCreateParams {
            server_url: server_url.to_string(),
            timeout,
        };
        let conn = T::create(params.clone())?;

        let (session_id, session_capabilities) = start_session(&conn, capabilities).await?;

        let driver = GenericWebDriver {
            session: WebDriverSession::new(session_id, Arc::new(Mutex::new(conn))),
            http_create_params: params,
            capabilities: session_capabilities,
            quit_on_drop: true,
            phantom: PhantomData,
        };

        Ok(driver)
    }

    /// Return a clone of the capabilities as returned by Selenium.
    pub fn capabilities(&self) -> DesiredCapabilities {
        DesiredCapabilities::new(self.capabilities.clone())
    }

    /// End the webdriver session.
    pub async fn quit(mut self) -> WebDriverResult<()> {
        self.cmd(Command::DeleteSession).await?;
        self.quit_on_drop = false;
        Ok(())
    }

    pub fn session_id(&self) -> &SessionId {
        self.session.session_id()
    }

    pub fn config(&self) -> &WebDriverConfig {
        self.session.config()
    }

    pub fn config_mut(&mut self) -> &mut WebDriverConfig {
        self.session.config_mut()
    }

    /// Set the request timeout for the HTTP client.
    ///
    /// # Example
    /// ```rust
    /// # use thirtyfour::prelude::*;
    /// # use std::time::Duration;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// let caps = DesiredCapabilities::chrome();
    /// let mut driver = WebDriver::new("http://localhost:4444/wd/hub", &caps).await?;
    /// driver.set_request_timeout(Duration::from_secs(180)).await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn set_request_timeout(&mut self, timeout: Duration) -> WebDriverResult<()> {
        self.session.set_request_timeout(timeout).await
    }
}

#[async_trait]
impl<T> WebDriverCommands for GenericWebDriver<T>
where
    T: WebDriverHttpClientAsync,
{
    fn session(&self) -> &WebDriverSession {
        &self.session
    }
}

impl<T: 'static> Drop for GenericWebDriver<T>
where
    T: WebDriverHttpClientAsync,
{
    /// Close the current session when the WebDriver struct goes out of scope.
    fn drop(&mut self) {
        if self.quit_on_drop && !self.session.session_id().is_empty() {
            if let Err(e) = block_on(self.cmd(Command::DeleteSession)) {
                error!("Failed to close session: {:?}", e);
            }
        }
    }
}

#[cfg(feature = "persist")]
pub mod persist {
    use std::marker::PhantomData;

    use serde::{Deserialize, Serialize};

    use crate::{
        common::types::SessionId,
        error::WebDriverResult,
        http::connection_async::WebDriverHttpClientAsync,
        session::{spawn_session_task, WebDriverSession},
        webdriver::{GenericWebDriver, WebDriver},
    };

    /// The persisted session that can be serialized and deserialized.
    ///
    /// # TODO
    ///
    ///  - What is `WebDriverConfig` used for? Do we need to persist it as well?
    ///
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct PersistedSession {
        session_id: SessionId,
        capabilities: serde_json::Value,
    }

    impl PersistedSession {
        pub async fn connect<T>(
            self,
            remote_server_addr: &str,
        ) -> WebDriverResult<GenericWebDriver<T>>
        where
            T: WebDriverHttpClientAsync + 'static,
        {
            let conn = T::create(remote_server_addr)?;
            // TODO: Check if session exists?
            let tx = spawn_session_task(Box::new(conn));

            let driver = GenericWebDriver {
                session: WebDriverSession::new(self.session_id, tx),
                capabilities: self.capabilities,
                quit_on_drop: true,
                phantom: PhantomData,
            };

            Ok(driver)
        }
    }

    impl WebDriver {
        pub fn persist(mut self) -> PersistedSession {
            // Disable `quit_on_drop` so that the session is not closed when this `self` is dropped.
            self.quit_on_drop = false;

            PersistedSession {
                session_id: self.session.session_id().clone(),
                capabilities: self.capabilities.clone(),
            }
        }
    }
}
