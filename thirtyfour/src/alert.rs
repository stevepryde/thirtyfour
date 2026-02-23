use crate::common::command::Command;
use crate::error::WebDriverResult;
use crate::session::handle::SessionHandle;
use crate::TypingData;
use std::sync::Arc;

/// Struct for managing alerts.
#[derive(Debug)]
pub struct Alert {
    handle: Arc<SessionHandle>,
}

impl Alert {
    /// Create a new Alert struct. This is typically created internally
    /// via a call to `WebDriver::switch_to().alert()`.
    pub fn new(handle: Arc<SessionHandle>) -> Self {
        Self {
            handle,
        }
    }

    /// Get the text of the active alert if there is one.
    ///
    /// # Errors
    /// Returns an error if there is no active alert or if the `WebDriver` returns an error.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been moved to WebDriver::get_alert_text()"
    )]
    pub async fn text(&self) -> WebDriverResult<String> {
        self.handle.get_alert_text().await
    }

    /// Dismiss the active alert if there is one.
    ///
    /// # Errors
    /// Returns an error if there is no active alert or if the `WebDriver` returns an error.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been moved to WebDriver::dismiss_alert()"
    )]
    pub async fn dismiss(&self) -> WebDriverResult<()> {
        self.handle.dismiss_alert().await
    }

    /// Accept the active alert if there is one.
    ///
    /// # Errors
    /// Returns an error if there is no active alert or if the `WebDriver` returns an error.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been moved to WebDriver::accept_alert()"
    )]
    pub async fn accept(&self) -> WebDriverResult<()> {
        self.handle.accept_alert().await
    }

    /// Send the specified text to the active alert if there is one.
    ///
    /// # Errors
    /// Returns an error if there is no active alert or if the `WebDriver` returns an error.
    #[deprecated(
        since = "0.30.0",
        note = "This method has been moved to WebDriver::send_alert_text()"
    )]
    pub async fn send_keys(&self, keys: impl Into<TypingData>) -> WebDriverResult<()> {
        self.handle.send_alert_text(keys.into()).await
    }
}

impl SessionHandle {
    /// Get the active alert text.
    ///
    /// # Errors
    /// Returns an error if there is no active alert or if the `WebDriver` returns an error.
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
    /// let text = driver.get_alert_text().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn get_alert_text(&self) -> WebDriverResult<String> {
        self.cmd(Command::GetAlertText).await?.value::<String>()
    }

    /// Dismiss the active alert.
    ///
    /// # Errors
    /// Returns an error if there is no active alert or if the `WebDriver` returns an error.
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
    /// driver.dismiss_alert().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn dismiss_alert(&self) -> WebDriverResult<()> {
        self.cmd(Command::DismissAlert).await?;
        Ok(())
    }

    /// Accept the active alert.
    ///
    /// # Errors
    /// Returns an error if there is no active alert or if the `WebDriver` returns an error.
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
    /// driver.accept_alert().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn accept_alert(&self) -> WebDriverResult<()> {
        self.cmd(Command::AcceptAlert).await?;
        Ok(())
    }

    /// Send the specified keys to the active alert.
    ///
    /// # Errors
    /// Returns an error if there is no active alert or if the `WebDriver` returns an error.
    ///
    /// # Example:
    /// You can specify anything that implements `Into<TypingData>`. This
    /// includes &str and String.
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.send_alert_text("selenium").await?;
    /// driver.accept_alert().await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    ///
    /// You can also send special key combinations like this:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// driver.send_alert_text(Key::Control + "a").await?;
    /// driver.send_alert_text("thirtyfour").await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub async fn send_alert_text(&self, keys: impl Into<TypingData>) -> WebDriverResult<()> {
        self.cmd(Command::SendAlertText(keys.into())).await?;
        Ok(())
    }
}
