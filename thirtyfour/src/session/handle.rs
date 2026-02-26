use serde_json::Value;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::sync::OnceCell;
use url::{ParseError, Url};

use crate::action_chain::ActionChain;
use crate::common::command::{Command, FormatRequestData};
use crate::common::config::WebDriverConfig;
use crate::common::cookie::Cookie;
use crate::common::print::PrintParameters;
use crate::error::WebDriverResult;
use crate::prelude::WebDriverError;
use crate::session::scriptret::ScriptRet;
use crate::support::base64_decode;
use crate::web_driver::AlreadyQuit;
use crate::{support, By, OptionRect, Rect, SessionId, SwitchTo, WebDriverStatus, WebElement};
use crate::{IntoArcStr, IntoUrl};
use crate::{TimeoutConfiguration, WindowHandle};

use super::http::{run_webdriver_cmd, CmdResponse, HttpClient};

/// The SessionHandle contains a shared reference to the HTTP client
/// to allow sending commands to the underlying WebDriver.
pub struct SessionHandle {
    /// The HTTP client for performing webdriver requests.
    pub client: Arc<dyn HttpClient>,
    /// The webdriver server URL.
    server_url: Arc<Url>,
    /// The session id for this webdriver session.
    session_id: SessionId,
    /// The config used by this instance.
    config: WebDriverConfig,
    /// quit session flag
    quit: Arc<OnceCell<()>>,
    /// The WebSocket URL for BiDi connections.
    pub websocket_url: Option<String>,
}

impl Debug for SessionHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionHandle")
            .field("session_id", &self.session_id)
            .field("config", &self.config)
            .field("websocket_url", &self.websocket_url)
            .finish()
    }
}

impl SessionHandle {
    /// Create new SessionHandle.
    pub fn new(
        client: Arc<dyn HttpClient>,
        server_url: impl IntoUrl,
        session_id: SessionId,
    ) -> WebDriverResult<Self> {
        Self::new_with_config(client, server_url, session_id, None, WebDriverConfig::default())
    }

    /// Create new `SessionHandle` with the specified `WebDriverConfig`.
    pub(crate) fn new_with_config(
        client: Arc<dyn HttpClient>,
        server_url: impl IntoUrl,
        session_id: SessionId,
        websocket_url: Option<String>,
        config: WebDriverConfig,
    ) -> WebDriverResult<Self> {
        Ok(Self {
            client,
            server_url: Arc::new(server_url.into_url()?),
            session_id,
            config,
            quit: Arc::new(OnceCell::new()),
            websocket_url,
        })
    }

    /// Clone this session handle but attach the specified `WebDriverConfig`.
    ///
    /// See `WebDriver::clone_with_config()`.
    pub(crate) fn clone_with_config(self: &SessionHandle, config: WebDriverConfig) -> Self {
        Self {
            client: Arc::clone(&self.client),
            server_url: Arc::clone(&self.server_url),
            session_id: self.session_id.clone(),
            quit: Arc::clone(&self.quit),
            config,
            websocket_url: self.websocket_url.clone(),
        }
    }

    /// The session id for this webdriver session.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// The configuration used by this instance.
    ///
    /// NOTE: It's sometimes useful to have separate instances pointing at the same
    ///       underlying browser session but using different configurations.
    ///       See [`WebDriver::clone_with_config()`] for more details.
    ///
    /// [`WebDriver::clone_with_config()`]: crate::WebDriver::clone_with_config()
    pub fn config(&self) -> &WebDriverConfig {
        &self.config
    }

    /// Derive the WebSocket URL for BiDi connections from the server URL.
    ///
    /// Converts http:// to ws:// and https:// to wss://.
    pub fn derive_bidi_ws_url(&self) -> String {
        let url_str = self.server_url.as_ref().as_str();
        if url_str.starts_with("https://") {
            format!("wss{}", &url_str[4..])
        } else if url_str.starts_with("http://") {
            format!("ws{}", &url_str[3..])
        } else {
            url_str.to_string()
        }
    }

    /// Send the specified command to the webdriver server.
    pub async fn cmd(&self, command: impl FormatRequestData) -> WebDriverResult<CmdResponse> {
        let request_data = command.format_request(&self.session_id);
        run_webdriver_cmd(&*self.client, &request_data, &self.server_url, &self.config).await
    }

    /// Get the WebDriver status.
    ///
    /// # Example
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// let caps = DesiredCapabilities::chrome();
    /// let mut driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let status = driver.status().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn status(&self) -> WebDriverResult<WebDriverStatus> {
        self.cmd(Command::Status).await?.value()
    }

    /// Close the current window or tab. This will close the session if no other windows exist.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// // Open a new tab.
    /// driver.new_tab().await?;
    ///
    /// // Get window handles and switch to the new tab.
    /// let handles = driver.windows().await?;
    /// driver.switch_to_window(handles[1].clone()).await?;
    ///
    /// // We are now controlling the new tab.
    /// driver.goto("https://www.rust-lang.org").await?;
    ///
    /// // Close the tab. This will return to the original tab.
    /// driver.close_window().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn close_window(&self) -> WebDriverResult<()> {
        self.cmd(Command::CloseWindow).await?;
        Ok(())
    }

    /// Close the current window or tab. This will close the session if no other windows exist.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to close_window()")]
    pub async fn close(&self) -> WebDriverResult<()> {
        self.close_window().await
    }

    /// Navigate to the specified URL.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.goto("https://www.rust-lang.org").await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn goto(&self, url: impl IntoArcStr) -> WebDriverResult<()> {
        let url = url.into();

        let parse_url = |url: Arc<str>| Url::parse(&url).map(|_| url);
        let url = parse_url(url.clone())
            .or_else(|e| match e {
                ParseError::RelativeUrlWithoutBase => {
                    parse_url(("https://".to_string() + &*url).into())
                }
                e => Err(e),
            })
            .map_err(WebDriverError::InvalidUrl)?;
        self.cmd(Command::NavigateTo(url)).await?;
        Ok(())
    }

    /// Navigate to the specified URL. Alias of goto().
    pub async fn get(&self, url: impl IntoArcStr) -> WebDriverResult<()> {
        self.goto(url).await
    }

    /// Get the current URL.
    pub async fn current_url(&self) -> WebDriverResult<Url> {
        let r = self.cmd(Command::GetCurrentUrl).await?;
        let s: String = r.value()?;
        Url::parse(&s).map_err(|e| WebDriverError::ParseError(format!("invalid url: {s}: {e}")))
    }

    /// Get the page source as a String.
    pub async fn source(&self) -> WebDriverResult<String> {
        self.cmd(Command::GetPageSource).await?.value()
    }

    /// Get the page source as a String.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to source()")]
    pub async fn page_source(&self) -> WebDriverResult<String> {
        self.source().await
    }

    /// Get the page title as a String.
    pub async fn title(&self) -> WebDriverResult<String> {
        self.cmd(Command::GetTitle).await?.value()
    }

    /// Search for an element on the current page using the specified selector.
    ///
    /// **NOTE**: For more powerful element queries including polling and filters, see the
    ///           [`WebDriver::query`] method instead.
    ///
    /// [`WebDriver::query`]: crate::extensions::query::ElementQueryable::query
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem_button = driver.find(By::Id("my-element-id")).await?;
    /// let elem_text = driver.find(By::Name("my-text-input")).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn find(self: &Arc<Self>, by: By) -> WebDriverResult<WebElement> {
        let r = self.cmd(Command::FindElement(by.into())).await?;
        r.element(self.clone())
    }

    /// Search for an element on the current page using the specified selector.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to find()")]
    pub async fn find_element(self: &Arc<Self>, by: By) -> WebDriverResult<WebElement> {
        self.find(by).await
    }

    /// Search for all elements on the current page that match the specified selector.
    ///
    /// **NOTE**: For more powerful element queries including polling and filters, see the
    ///           [`WebDriver::query`] method instead.
    ///
    /// [`WebDriver::query`]: crate::extensions::query::ElementQueryable::query
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elems = driver.find_all(By::ClassName("section")).await?;
    /// for elem in elems {
    ///     assert!(elem.attr("class").await?.expect("Missing class on element").contains("section"));
    /// }
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn find_all(self: &Arc<Self>, by: By) -> WebDriverResult<Vec<WebElement>> {
        let r = self.cmd(Command::FindElements(by.into())).await?;
        r.elements(self.clone())
    }

    /// Search for all elements on the current page that match the specified selector.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to find_all()")]
    pub async fn find_elements(self: &Arc<Self>, by: By) -> WebDriverResult<Vec<WebElement>> {
        self.find_all(by).await
    }

    /// Execute the specified Javascript synchronously and return the result.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let ret = driver.execute(r#"
    ///     let elem = document.getElementById("button1");
    ///     elem.click();
    ///     return elem;
    ///     "#, Vec::new()
    /// ).await?;
    /// let elem_out: WebElement = ret.element()?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    /// To supply an element as an input argument to a script, use
    /// [`WebElement::to_json`] as follows:
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// let ret = driver.execute(r#"
    ///     arguments[0].innerHTML = arguments[1];
    ///     return arguments[0];
    ///     "#, vec![elem.to_json()?, serde_json::to_value("TESTING")?]
    /// ).await?;
    /// let elem_out = ret.element()?;
    /// assert_eq!(elem_out.element_id(), elem.element_id());
    /// assert_eq!(elem_out.text().await?, "TESTING");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn execute(
        self: &Arc<Self>,
        script: impl IntoArcStr,
        args: impl Into<Arc<[Value]>>,
    ) -> WebDriverResult<ScriptRet> {
        let r = self.cmd(Command::ExecuteScript(script.into(), args.into())).await?;
        Ok(ScriptRet::new(self.clone(), r.value()?))
    }

    /// Execute the specified Javascript synchronously and return the result.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to execute()")]
    pub async fn execute_script(
        self: &Arc<Self>,
        script: impl IntoArcStr,
        args: Vec<Value>,
    ) -> WebDriverResult<ScriptRet> {
        self.execute(script, args).await
    }

    /// Execute the specified JavaScript asynchronously and return the result.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let ret = driver.execute_async(r#"
    ///     // Selenium automatically provides an extra argument which is a
    ///     // function that receives the return value(s).
    ///     let done = arguments[0];
    ///     window.setTimeout(() => {
    ///         let elem = document.getElementById("button1");
    ///         elem.click();
    ///         done(elem);
    ///     }, 1000);
    ///     "#, Vec::new()
    /// ).await?;
    /// let elem_out: WebElement = ret.element()?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    /// To supply an element as an input argument to a script, use
    /// [`WebElement::to_json`] as follows:
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem = driver.find(By::Id("button1")).await?;
    /// let args = vec![elem.to_json()?, serde_json::to_value("TESTING")?];
    /// let ret = driver.execute_async(r#"
    ///     // Selenium automatically provides an extra argument which is a
    ///     // function that receives the return value(s).
    ///     let done = arguments[2];
    ///     window.setTimeout(() => {
    ///         arguments[0].innerHTML = arguments[1];
    ///         done(arguments[0]);
    ///     }, 1000);
    ///     "#, args
    /// ).await?;
    /// let elem_out = ret.element()?;
    /// assert_eq!(elem_out.element_id(), elem.element_id());
    /// assert_eq!(elem_out.text().await?, "TESTING");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn execute_async(
        self: &Arc<Self>,
        script: impl IntoArcStr,
        args: impl Into<Arc<[Value]>>,
    ) -> WebDriverResult<ScriptRet> {
        let r = self.cmd(Command::ExecuteAsyncScript(script.into(), args.into())).await?;
        Ok(ScriptRet::new(self.clone(), r.value()?))
    }

    /// Execute the specified JavaScript asynchronously and return the result.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to execute_async()")]
    pub async fn execute_script_async(
        self: &Arc<Self>,
        script: impl IntoArcStr,
        args: impl Into<Arc<[Value]>>,
    ) -> WebDriverResult<ScriptRet> {
        self.execute_async(script, args.into()).await
    }

    /// Get the current window handle.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// // Get the current window handle.
    /// let handle = driver.window().await?;
    ///
    /// // Open a new tab.
    /// driver.new_tab().await?;
    ///
    /// // Get window handles and switch to the new tab.
    /// let handles = driver.windows().await?;
    /// driver.switch_to_window(handles[1].clone()).await?;
    ///
    /// // We are now controlling the new tab.
    /// driver.goto("https://www.rust-lang.org/").await?;
    /// assert_ne!(driver.window().await?, handle);
    ///
    /// // Switch back to original tab.
    /// driver.switch_to_window(handle.clone()).await?;
    /// assert_eq!(driver.window().await?, handle);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn window(&self) -> WebDriverResult<WindowHandle> {
        let r = self.cmd(Command::GetWindowHandle).await?;
        Ok(WindowHandle::from(r.value::<String>()?))
    }

    /// Get the current window handle.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to window()")]
    pub async fn current_window_handle(&self) -> WebDriverResult<WindowHandle> {
        self.window().await
    }

    /// Get all window handles for the current session.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// assert_eq!(driver.windows().await?.len(), 1);
    /// // Open a new tab.
    /// driver.new_tab().await?;
    ///
    /// // Get window handles and switch to the new tab.
    /// let handles = driver.windows().await?;
    /// assert_eq!(handles.len(), 2);
    /// driver.switch_to_window(handles[1].clone()).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn windows(&self) -> WebDriverResult<Vec<WindowHandle>> {
        let r = self.cmd(Command::GetWindowHandles).await?;
        let handles: Vec<String> = r.value()?;
        Ok(handles.into_iter().map(WindowHandle::from).collect())
    }

    /// Get all window handles for the current session.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to windows()")]
    pub async fn window_handles(&self) -> WebDriverResult<Vec<WindowHandle>> {
        self.windows().await
    }

    /// Maximize the current window.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.maximize_window().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn maximize_window(&self) -> WebDriverResult<()> {
        self.cmd(Command::MaximizeWindow).await?;
        Ok(())
    }

    /// Minimize the current window.
    ///
    /// # Example:
    /// ```no_run
    /// # // Minimize is not currently working on Chrome, but does work
    /// # // on Firefox/geckodriver.
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.minimize_window().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn minimize_window(&self) -> WebDriverResult<()> {
        self.cmd(Command::MinimizeWindow).await?;
        Ok(())
    }

    /// Make the current window fullscreen.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.fullscreen_window().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn fullscreen_window(&self) -> WebDriverResult<()> {
        self.cmd(Command::FullscreenWindow).await?;
        Ok(())
    }

    /// Get the current window rectangle, in pixels.
    ///
    /// The returned Rect struct has members `x`, `y`, `width`, `height`,
    /// all i32.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::Rect;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.set_window_rect(0, 0, 600, 400).await?;
    /// let rect = driver.get_window_rect().await?;
    /// assert_eq!(rect, Rect::new(0, 0, 600, 400));
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn get_window_rect(&self) -> WebDriverResult<Rect> {
        self.cmd(Command::GetWindowRect).await?.value()
    }

    /// Set the current window rectangle, in pixels.
    ///
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.set_window_rect(0, 0, 500, 400).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn set_window_rect(
        &self,
        x: i64,
        y: i64,
        width: u32,
        height: u32,
    ) -> WebDriverResult<()> {
        let rect = OptionRect {
            x: Some(x),
            y: Some(y),
            width: Some(width as i64),
            height: Some(height as i64),
        };
        self.cmd(Command::SetWindowRect(rect)).await?;
        Ok(())
    }

    /// Go back. This is equivalent to clicking the browser's back button.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.back().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn back(&self) -> WebDriverResult<()> {
        self.cmd(Command::Back).await?;
        Ok(())
    }

    /// Go forward. This is equivalent to clicking the browser's forward button.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.forward().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn forward(&self) -> WebDriverResult<()> {
        self.cmd(Command::Forward).await?;
        Ok(())
    }

    /// Refresh the current page.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.refresh().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn refresh(&self) -> WebDriverResult<()> {
        self.cmd(Command::Refresh).await?;
        Ok(())
    }

    /// Get all timeouts for the current session.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::TimeoutConfiguration;
    /// use std::time::Duration;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let timeouts = driver.get_timeouts().await?;
    /// println!("Page load timeout = {:?}", timeouts.page_load());
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn get_timeouts(&self) -> WebDriverResult<TimeoutConfiguration> {
        self.cmd(Command::GetTimeouts).await?.value()
    }

    /// Set all timeouts for the current session.
    ///
    /// **NOTE:** Setting the implicit wait timeout to a non-zero value will interfere with the use
    /// of [`WebDriver::query`] and [`WebElement::wait_until`].
    /// It is therefore recommended to use these methods (which provide polling
    /// and explicit waits) instead rather than increasing the implicit wait timeout.
    ///
    /// [`WebDriver::query`]: crate::extensions::query::ElementQueryable::query
    /// [`WebElement::wait_until`]: crate::extensions::query::ElementWaitable::wait_until
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::TimeoutConfiguration;
    /// use std::time::Duration;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// // Setting timeouts to None means those timeout values will not be updated.
    /// let timeouts = TimeoutConfiguration::new(None, Some(Duration::new(11, 0)), None);
    /// driver.update_timeouts(timeouts).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn update_timeouts(&self, timeouts: TimeoutConfiguration) -> WebDriverResult<()> {
        self.cmd(Command::SetTimeouts(timeouts)).await?;
        Ok(())
    }

    /// Set all timeouts for the current session.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to update_timeouts()")]
    pub async fn set_timeouts(&self, timeouts: TimeoutConfiguration) -> WebDriverResult<()> {
        self.update_timeouts(timeouts).await
    }

    /// Set the implicit wait timeout.
    ///
    /// This is how long the WebDriver will wait when querying elements.
    /// By default, this is set to 0 seconds.
    ///
    /// **NOTE:** Setting the implicit wait timeout to a non-zero value will interfere with the use
    /// of [`WebDriver::query`] and [`WebElement::wait_until`].
    /// It is therefore recommended to use these methods (which provide polling
    /// and explicit waits) instead rather than increasing the implicit wait timeout.
    ///
    /// [`WebDriver::query`]: crate::extensions::query::ElementQueryable::query
    /// [`WebElement::wait_until`]: crate::extensions::query::ElementWaitable::wait_until
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::TimeoutConfiguration;
    /// use std::time::Duration;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let delay = Duration::new(11, 0);
    /// driver.set_implicit_wait_timeout(delay).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn set_implicit_wait_timeout(&self, time_to_wait: Duration) -> WebDriverResult<()> {
        let timeouts = TimeoutConfiguration::new(None, None, Some(time_to_wait));
        self.update_timeouts(timeouts).await
    }

    /// Set the script timeout.
    ///
    /// This is how long the WebDriver will wait for a Javascript script to execute.
    /// By default, this is set to 60 seconds.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::TimeoutConfiguration;
    /// use std::time::Duration;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let delay = Duration::new(11, 0);
    /// driver.set_script_timeout(delay).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn set_script_timeout(&self, time_to_wait: Duration) -> WebDriverResult<()> {
        let timeouts = TimeoutConfiguration::new(Some(time_to_wait), None, None);
        self.update_timeouts(timeouts).await
    }

    /// Set the page load timeout.
    ///
    /// This is how long the WebDriver will wait for the page to finish loading.
    /// By default, this is set to 60 seconds.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::TimeoutConfiguration;
    /// use std::time::Duration;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let delay = Duration::new(11, 0);
    /// driver.set_page_load_timeout(delay).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn set_page_load_timeout(&self, time_to_wait: Duration) -> WebDriverResult<()> {
        let timeouts = TimeoutConfiguration::new(None, Some(time_to_wait), None);
        self.update_timeouts(timeouts).await
    }

    /// Create a new action chain for this session.
    ///
    /// Action chains can be used to simulate more complex user input actions
    /// involving key combinations, mouse movements, mouse click, right-click,
    /// and more.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let elem_text = driver.find(By::Name("input1")).await?;
    /// let elem_button = driver.find(By::Id("button-set")).await?;
    ///
    /// driver.action_chain()
    ///     .send_keys_to_element(&elem_text, "thirtyfour")
    ///     .move_to_element_center(&elem_button)
    ///     .click()
    ///     .perform()
    ///     .await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub fn action_chain(self: &Arc<SessionHandle>) -> ActionChain {
        ActionChain::new(self.clone())
    }

    /// Create a new action chain for this session.
    /// Set custom delays for key and pointer actions
    ///
    /// The [`Duration`] is the time before an action is executed in the chain.
    ///
    /// `key_delay` defaults to 0ms, `pointer_delay` defaults to 250ms
    pub fn action_chain_with_delay(
        self: &Arc<SessionHandle>,
        key_delay: Option<Duration>,
        pointer_delay: Option<Duration>,
    ) -> ActionChain {
        ActionChain::new_with_delay(self.clone(), key_delay, pointer_delay)
    }

    /// Get all cookies.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let cookies = driver.get_all_cookies().await?;
    /// for cookie in &cookies {
    ///     println!("Got cookie: {}", cookie.value);
    /// }
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn get_all_cookies(&self) -> WebDriverResult<Vec<Cookie>> {
        self.cmd(Command::GetAllCookies).await?.value()
    }

    /// Get all cookies.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to get_all_cookies()")]
    pub async fn get_cookies(&self) -> WebDriverResult<Vec<Cookie>> {
        self.get_all_cookies().await
    }

    /// Get the specified cookie.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let cookie = driver.get_named_cookie("key").await?;
    /// println!("Got cookie: {}", cookie.value);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn get_named_cookie(&self, name: impl IntoArcStr) -> WebDriverResult<Cookie> {
        self.cmd(Command::GetNamedCookie(name.into())).await?.value()
    }

    /// Get the specified cookie.
    #[deprecated(since = "0.30.0", note = "This method has been renamed to get_named_cookie()")]
    pub async fn get_cookie(&self, name: impl IntoArcStr) -> WebDriverResult<Cookie> {
        self.get_named_cookie(name).await
    }

    /// Delete the specified cookie.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.delete_cookie("key").await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn delete_cookie(&self, name: impl IntoArcStr) -> WebDriverResult<()> {
        self.cmd(Command::DeleteCookie(name.into())).await?;
        Ok(())
    }

    /// Delete all cookies.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.delete_all_cookies().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn delete_all_cookies(&self) -> WebDriverResult<()> {
        self.cmd(Command::DeleteAllCookies).await?;
        Ok(())
    }

    /// Add the specified cookie.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.goto("https://wikipedia.org").await?;
    /// let mut cookie = Cookie::new("key", "value");
    /// cookie.set_domain("wikipedia.org");
    /// cookie.set_path("/");
    /// cookie.set_same_site(SameSite::Lax);
    /// driver.add_cookie(cookie.clone()).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn add_cookie(&self, cookie: Cookie) -> WebDriverResult<()> {
        self.cmd(Command::AddCookie(cookie)).await?;
        Ok(())
    }

    /// Print the current window and return it as a PDF.
    pub async fn print_page(&self, parameters: PrintParameters) -> WebDriverResult<Vec<u8>> {
        base64_decode(&self.print_page_base64(parameters).await?)
    }

    /// Print the current window and return it as a PDF, base64 encoded.
    pub async fn print_page_base64(&self, parameters: PrintParameters) -> WebDriverResult<String> {
        self.cmd(Command::PrintPage(parameters)).await?.value()
    }

    /// Take a screenshot of the current window and return it as PNG, base64 encoded.
    pub async fn screenshot_as_png_base64(&self) -> WebDriverResult<String> {
        self.cmd(Command::TakeScreenshot).await?.value()
    }

    /// Take a screenshot of the current window and return it as PNG bytes.
    pub async fn screenshot_as_png(&self) -> WebDriverResult<Vec<u8>> {
        base64_decode(&self.screenshot_as_png_base64().await?)
    }

    /// Take a screenshot of the current window and write it to the specified filename.
    pub async fn screenshot(&self, path: &Path) -> WebDriverResult<()> {
        let png = self.screenshot_as_png().await?;
        support::write_file(path, png).await?;
        Ok(())
    }

    /// Return a SwitchTo struct for switching to another window or frame.
    #[deprecated(
        since = "0.30.0",
        note = "SwitchTo has been deprecated. Use WebDriver::switch_to_*() methods instead"
    )]
    pub fn switch_to(self: &Arc<SessionHandle>) -> SwitchTo {
        SwitchTo::new(self.clone())
    }

    /// Set the current window name.
    ///
    /// Useful for switching between windows/tabs using [`WebDriver::switch_to_named_window`]
    ///
    /// [`WebDriver::switch_to_named_window`]: SessionHandle::switch_to_named_window
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// // Get the current window handle.
    /// let handle = driver.window().await?;
    /// driver.set_window_name("main").await?;
    ///
    /// // Open a new tab.
    /// let new_handle = driver.new_tab().await?;
    ///
    /// // Get window handles and switch to the new tab.
    /// driver.switch_to_window(new_handle).await?;
    ///
    /// // We are now controlling the new tab.
    /// driver.goto("https://www.rust-lang.org").await?;
    /// assert_ne!(driver.window().await?, handle);
    ///
    /// // Switch back to original tab using window name.
    /// driver.switch_to_named_window("main").await?;
    /// assert_eq!(driver.window().await?, handle);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn set_window_name(
        self: &Arc<SessionHandle>,
        window_name: impl Display,
    ) -> WebDriverResult<()> {
        let script = format!(r#"window.name = "{}""#, window_name);
        self.execute(script, Vec::new()).await?;
        Ok(())
    }

    /// Execute the specified function in a new browser tab, closing the tab when complete.
    ///
    /// The return value will be that of the supplied function, unless an error occurs while
    /// opening or closing the tab.
    ///
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let window_title = driver.in_new_tab(|| async {
    ///     driver.goto("https://www.google.com").await?;
    ///     driver.title().await
    /// }).await?;
    /// assert_eq!(window_title, "Google");
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn in_new_tab<F, Fut, T>(&self, f: F) -> WebDriverResult<T>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = WebDriverResult<T>> + Send,
        T: Send,
    {
        let handle = self.window().await?;

        // Open new tab.
        let tab_handle = self.new_tab().await?;
        self.switch_to_window(tab_handle).await?;

        let result = f().await;

        // Close tab.
        self.close_window().await?;
        self.switch_to_window(handle).await?;

        result
    }

    pub(crate) async fn quit(&self) -> WebDriverResult<()> {
        self.quit
            .get_or_try_init(|| async { self.cmd(Command::DeleteSession).await.map(drop) })
            .await?;
        Ok(())
    }

    pub(crate) fn leak(&self) -> Result<(), AlreadyQuit> {
        self.quit.set(()).map_err(|_| AlreadyQuit(()))
    }
}

// "SyncDrop" only runs if not manually quit
impl Drop for SessionHandle {
    #[track_caller]
    fn drop(&mut self) {
        if self.quit.initialized() {
            return;
        }

        #[cfg(feature = "debug_sync_quit")]
        eprintln!(
            "WebDriver didn't wasn't quit properly at\n{}",
            std::backtrace::Backtrace::capture()
        );

        struct SessionDropGuard(SessionHandle);

        impl Deref for SessionDropGuard {
            type Target = SessionHandle;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for SessionDropGuard {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl Drop for SessionDropGuard {
            fn drop(&mut self) {
                if !self.0.quit.initialized() {
                    static ALWAYS_INIT: LazyLock<Arc<OnceCell<()>>> =
                        LazyLock::new(|| Arc::new(OnceCell::new_with(Some(()))));
                    self.0.quit = Arc::clone(&ALWAYS_INIT);
                    debug_assert!(self.0.quit.initialized())
                }
            }
        }

        let mut this = SessionDropGuard(Self {
            client: Arc::clone(&self.client),
            server_url: Arc::clone(&self.server_url),
            quit: Arc::clone(&self.quit),
            session_id: self.session_id.clone(),
            config: self.config.clone(),
            websocket_url: self.websocket_url.clone(),
        });

        support::spawn_blocked_future(|spawned| async move {
            if spawned {
                // Old I/O drivers may be destroyed at this point
                this.client = this.client.new().await;
            }
            let _ = this.quit().await;
        });
    }
}
