use super::ChromeCommand;
use super::NetworkConditions;
use crate::error::WebDriverResult;
use crate::session::handle::SessionHandle;
use serde_json::{json, Value};
use std::sync::Arc;

/// The `ChromeDevTools` struct allows you to interact with Chromium-based browsers via
/// the Chrome Devtools Protocol (CDP).
///
/// You can find documentation for the available commands here:
/// [https://chromedevtools.github.io/devtools-protocol/](https://chromedevtools.github.io/devtools-protocol/])
///
/// # Example
/// ```no_run
/// # use thirtyfour::prelude::*;
/// # use thirtyfour::support::block_on;
/// # use thirtyfour::extensions::cdp::ChromeDevTools;
/// #
/// # fn main() -> WebDriverResult<()> {
/// #     block_on(async {
/// let caps = DesiredCapabilities::chrome();
/// let driver = WebDriver::new("http://localhost:4444", caps).await?;
///
/// // Create a ChromeDevTools struct like this.
/// let dev_tools = ChromeDevTools::new(driver.handle.clone());
/// dev_tools.execute_cdp("Network.clearBrowserCache").await?;
/// #         driver.quit().await?;
/// #         Ok(())
/// #     })
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ChromeDevTools {
    /// The underlying session handle.
    pub handle: Arc<SessionHandle>,
}

impl ChromeDevTools {
    /// Create a new `ChromeDevTools` struct.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::extensions::cdp::ChromeDevTools;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let dev_tools = ChromeDevTools::new(driver.handle.clone());
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    pub fn new(handle: Arc<SessionHandle>) -> Self {
        Self {
            handle,
        }
    }

    /// Launch the Chrome app with the specified id.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn launch_app(&self, app_id: &str) -> WebDriverResult<()> {
        self.handle.cmd(ChromeCommand::LaunchApp(app_id.to_string())).await?;
        Ok(())
    }

    /// Get the current network conditions. You must set the conditions first.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::extensions::cdp::{ChromeDevTools, NetworkConditions};
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// // Create ChromeDevTools struct.
    /// let dev_tools = ChromeDevTools::new(driver.handle.clone());
    ///
    /// // First we need to set the network conditions.
    /// let mut conditions = NetworkConditions::new();
    /// conditions.download_throughput = 20;
    /// dev_tools.set_network_conditions(&conditions).await?;
    ///
    /// // Now we can get the network conditions.
    /// let conditions_out = dev_tools.get_network_conditions().await?;
    /// assert_eq!(conditions_out.download_throughput, conditions.download_throughput);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn get_network_conditions(&self) -> WebDriverResult<NetworkConditions> {
        let v = self.handle.cmd(ChromeCommand::GetNetworkConditions).await?;
        v.value()
    }

    /// Set the network conditions.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::extensions::cdp::{ChromeDevTools, NetworkConditions};
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// // Create ChromeDevTools struct.
    /// let dev_tools = ChromeDevTools::new(driver.handle.clone());
    ///
    /// // Now we can set the network conditions. You do not need to set all parameters.
    /// let mut conditions = NetworkConditions::new();
    /// conditions.download_throughput = 20;
    /// conditions.upload_throughput = 10;
    /// conditions.offline = false;
    /// conditions.latency = 200;
    ///
    /// dev_tools.set_network_conditions(&conditions).await?;
    /// #         let conditions_out = dev_tools.get_network_conditions().await?;
    /// #         assert_eq!(conditions_out.download_throughput, conditions.download_throughput);
    /// #         assert_eq!(conditions_out.upload_throughput, conditions.upload_throughput);
    /// #         assert_eq!(conditions_out.latency, conditions.latency);
    /// #         assert_eq!(conditions_out.offline, conditions.offline);
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn set_network_conditions(
        &self,
        conditions: &NetworkConditions,
    ) -> WebDriverResult<()> {
        self.handle.cmd(ChromeCommand::SetNetworkConditions(conditions.clone())).await?;
        Ok(())
    }

    /// Execute the specified command without parameters.
    /// For commands that require parameters, use `execute_cdp_with_params()` instead.
    ///
    /// You can find documentation for the available commands here:
    /// [https://chromedevtools.github.io/devtools-protocol/](https://chromedevtools.github.io/devtools-protocol/])
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::extensions::cdp::ChromeDevTools;
    ///
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let dev_tools = ChromeDevTools::new(driver.handle.clone());
    /// dev_tools.execute_cdp("Network.clearBrowserCache").await?;
    ///
    /// // execute_cdp() can also return values as well.
    /// let version_info = dev_tools.execute_cdp("Browser.getVersion").await?;
    /// let user_agent = version_info["userAgent"].as_str().unwrap();
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails or if command execution fails.
    pub async fn execute_cdp(&self, cmd: &str) -> WebDriverResult<Value> {
        self.execute_cdp_with_params(cmd, json!({})).await
    }

    /// Execute the specified command with the specified parameter(s).
    ///
    /// You can find documentation for the available commands here:
    /// [https://chromedevtools.github.io/devtools-protocol/](https://chromedevtools.github.io/devtools-protocol/])
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::extensions::cdp::ChromeDevTools;
    /// use serde_json::json;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let dev_tools = ChromeDevTools::new(driver.handle.clone());
    /// dev_tools.execute_cdp_with_params("Network.setCacheDisabled", json!({"cacheDisabled": true})).await?;
    /// #         driver.quit().await?;
    /// #         Ok(())
    /// #     })
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn execute_cdp_with_params(
        &self,
        cmd: &str,
        cmd_args: Value,
    ) -> WebDriverResult<Value> {
        let v =
            self.handle.cmd(ChromeCommand::ExecuteCdpCommand(cmd.to_string(), cmd_args)).await?;
        v.value()
    }

    /// Get the list of sinks available for cast.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn get_sinks(&self) -> WebDriverResult<Value> {
        let v = self.handle.cmd(ChromeCommand::GetSinks).await?;
        v.value()
    }

    /// Get the issue message for any issue in a cast session.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn get_issue_message(&self) -> WebDriverResult<Value> {
        let v = self.handle.cmd(ChromeCommand::GetIssueMessage).await?;
        v.value()
    }

    /// Set the specified sink as the cast session receiver target.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn set_sink_to_use(&self, sink_name: &str) -> WebDriverResult<()> {
        self.handle.cmd(ChromeCommand::SetSinkToUse(sink_name.to_string())).await?;
        Ok(())
    }

    /// Start a tab mirroring session on the specified receiver target.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn start_tab_mirroring(&self, sink_name: &str) -> WebDriverResult<()> {
        self.handle.cmd(ChromeCommand::StartTabMirroring(sink_name.to_string())).await?;
        Ok(())
    }

    /// Stop the existing cast session on the specified receiver target.
    ///
    /// # Errors
    /// Returns an error if communication with the driver fails.
    pub async fn stop_casting(&self, sink_name: &str) -> WebDriverResult<()> {
        self.handle.cmd(ChromeCommand::StopCasting(sink_name.to_string())).await?;
        Ok(())
    }
}
