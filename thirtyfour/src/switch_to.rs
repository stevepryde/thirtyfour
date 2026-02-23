use crate::common::command::Command;
use crate::error::WebDriverErrorInfo;
use crate::session::handle::SessionHandle;
use crate::WindowHandle;
use crate::{
    error::{WebDriverError, WebDriverResult},
    Alert, WebElement,
};
use std::sync::Arc;

/// Struct for switching between frames/windows/alerts.
#[derive(Debug)]
pub struct SwitchTo {
    handle: Arc<SessionHandle>,
}

impl SwitchTo {
    /// Create a new `SwitchTo` struct. This is typically created internally
    /// via a call to `WebDriver::switch_to()`.
    pub fn new(handle: Arc<SessionHandle>) -> Self {
        Self {
            handle,
        }
    }

    /// Get the active element for this session.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been moved to WebDriver::active_element()"
    )]
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn active_element(self) -> WebDriverResult<WebElement> {
        self.handle.active_element().await
    }

    /// Switch to the specified alert.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been deprecated. See the `Alert` module for new method names"
    )]
    #[must_use]
    pub fn alert(self) -> Alert {
        Alert::new(self.handle)
    }

    /// Switch to the default frame.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been moved to WebDriver::enter_default_frame()"
    )]
    pub async fn default_content(self) -> WebDriverResult<()> {
        self.handle.enter_default_frame().await
    }

    /// Switch to the frame specified at the index.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    #[deprecated(since = "0.30.0", note = "This method has been moved to WebDriver::enter_frame()")]
    pub async fn frame_number(self, frame_number: u16) -> WebDriverResult<()> {
        self.handle.enter_frame(frame_number).await
    }

    /// Switch to the frame contained within the element.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been moved to WebElement::enter_frame()"
    )]
    pub async fn frame_element(self, frame_element: &WebElement) -> WebDriverResult<()> {
        frame_element.clone().enter_frame().await
    }

    /// Switch to the parent of the frame the client is currently contained within.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been moved to WebDriver::enter_parent_frame()"
    )]
    pub async fn parent_frame(self) -> WebDriverResult<()> {
        self.handle.enter_parent_frame().await?;
        Ok(())
    }

    /// Create a new window.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    #[deprecated(since = "0.30.0", note = "This method has been moved to WebDriver::new_window()")]
    pub async fn new_window(self) -> WebDriverResult<WindowHandle> {
        self.handle.new_window().await
    }

    /// Create a new tab.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    #[deprecated(since = "0.30.0", note = "This method has been moved to WebDriver::new_tab()")]
    pub async fn new_tab(self) -> WebDriverResult<WindowHandle> {
        self.handle.new_tab().await
    }

    /// Switch to the specified window.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been moved to WebDriver::switch_to_window()"
    )]
    pub async fn window(self, handle: WindowHandle) -> WebDriverResult<()> {
        self.handle.switch_to_window(handle).await
    }

    /// Switch to the specified named window.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or if the window is not found.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been moved to WebDriver::switch_to_named_window()"
    )]
    pub async fn window_name(self, name: &str) -> WebDriverResult<()> {
        let original_handle = self.handle.window().await?;
        let handles = self.handle.windows().await?;
        for handle in handles {
            self.handle.switch_to_window(handle).await?;
            let ret = self.handle.execute(r"return window.name;", Vec::new()).await?;
            let current_name: String = ret.convert()?;
            if current_name == name {
                return Ok(());
            }
        }

        self.handle.switch_to_window(original_handle).await?;
        Err(WebDriverError::NoSuchWindow(WebDriverErrorInfo::new(format!(
            "unable to find named window: {name}"
        ))))
    }
}

impl SessionHandle {
    /// Return the element with focus, or the `<body>` element if nothing has focus.
    ///
    /// # Errors
    /// Returns an error if there is no active element or if the `WebDriver` returns an error.
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
    /// // If no element has focus, active_element() will return the body tag.
    /// let active_elem = driver.active_element().await?;
    /// assert_eq!(active_elem.tag_name().await?, "body");
    ///
    /// // Now let's manually focus an element and try active_element() again.
    /// let elem = driver.find(By::Id("my-element-id")).await?;
    /// elem.focus().await?;
    ///
    /// // And fetch the active element again.
    /// let active_elem = driver.active_element().await?;
    /// assert_eq!(active_elem.element_id(), elem.element_id());
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn active_element(self: &Arc<SessionHandle>) -> WebDriverResult<WebElement> {
        let r = self.cmd(Command::GetActiveElement).await?;
        r.element(self)
    }

    /// Switch to the default frame.
    ///
    /// # Errors
    /// Returns an error if the `WebDriver` returns an error.
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
    /// // Enter the first iframe.
    /// driver.enter_frame(0).await?;
    /// // We are now inside the iframe.
    /// driver.find(By::Id("button1")).await?;
    /// driver.enter_default_frame().await?;
    /// // We are now back in the original window.
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn enter_default_frame(&self) -> WebDriverResult<()> {
        self.cmd(Command::SwitchToFrameDefault).await?;
        Ok(())
    }

    /// Switch to an iframe by index. The first iframe on the page has index 0.
    ///
    /// # Errors
    /// Returns an error if the frame doesn't exist or if the `WebDriver` returns an error.
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
    /// // Enter the first iframe.
    /// driver.enter_frame(0).await?;
    /// // We can now search for elements within the iframe.
    /// let elem = driver.find(By::Id("button1")).await?;
    /// elem.click().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn enter_frame(&self, frame_number: u16) -> WebDriverResult<()> {
        self.cmd(Command::SwitchToFrameNumber(frame_number)).await?;
        Ok(())
    }

    /// Switch to the parent frame.
    ///
    /// # Errors
    /// Returns an error if there is no parent frame or if the `WebDriver` returns an error.
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
    /// // Find the iframe element and enter the iframe.
    /// let elem_iframe = driver.find(By::Id("iframeid1")).await?;
    /// elem_iframe.enter_frame().await?;
    /// // We can now search for elements within the iframe.
    /// let elem = driver.find(By::Id("button1")).await?;
    /// elem.click().await?;
    /// // Now switch back to the parent frame.
    /// driver.enter_parent_frame().await?;
    /// // We are now back in the parent document.
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn enter_parent_frame(&self) -> WebDriverResult<()> {
        self.cmd(Command::SwitchToParentFrame).await?;
        Ok(())
    }

    /// Switch to the specified window.
    ///
    /// # Errors
    /// Returns an error if the window doesn't exist or if the `WebDriver` returns an error.
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
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn switch_to_window(&self, handle: WindowHandle) -> WebDriverResult<()> {
        self.cmd(Command::SwitchToWindow(handle)).await?;
        Ok(())
    }

    /// Switch to the window with the specified name. This uses the `window.name` property.
    /// You can set a window name via `WebDriver::set_window_name("someName").await?`.
    ///
    /// # Errors
    /// Returns an error if no window with the given name exists or if the `WebDriver` returns an error.
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
    /// // Set main window name so we can switch back easily.
    /// driver.set_window_name("mywindow").await?;
    ///
    /// // Open a new tab.
    /// driver.new_tab().await?;
    ///
    /// // Get window handles and switch to the new tab.
    /// let handles = driver.windows().await?;
    /// driver.switch_to_window(handles[1].clone()).await?;
    ///
    /// // We are now controlling the new tab.
    /// assert_eq!(driver.title().await?, "");
    /// driver.switch_to_named_window("mywindow").await?;
    ///
    /// // We are now back in the original tab.
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn switch_to_named_window(
        self: &Arc<SessionHandle>,
        name: &str,
    ) -> WebDriverResult<()> {
        let original_handle = self.window().await?;
        let handles = self.windows().await?;
        for handle in handles {
            self.switch_to_window(handle).await?;
            let ret = self.execute(r"return window.name;", Vec::new()).await?;
            let current_name: String = ret.convert()?;
            if current_name == name {
                return Ok(());
            }
        }

        self.switch_to_window(original_handle).await?;
        Err(WebDriverError::NoSuchWindow(WebDriverErrorInfo::new(format!(
            "unable to find named window: {name}"
        ))))
    }

    /// Switch to a new window.
    ///
    /// # Errors
    /// Returns an error if the `WebDriver` returns an error.
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
    /// // Open a new window.
    /// let handle = driver.new_window().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn new_window(&self) -> WebDriverResult<WindowHandle> {
        self.cmd(Command::NewWindow).await?.value()
    }

    /// Switch to a new tab.
    ///
    /// # Errors
    /// Returns an error if the `WebDriver` returns an error.
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
    /// // Open a new tab in the current window.
    /// let handle = driver.new_tab().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn new_tab(&self) -> WebDriverResult<WindowHandle> {
        self.cmd(Command::NewTab).await?.value()
    }
}
