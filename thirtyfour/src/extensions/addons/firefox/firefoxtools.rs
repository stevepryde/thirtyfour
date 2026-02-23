use std::path::Path;
use std::sync::Arc;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;

use super::FirefoxCommand;
use crate::error::WebDriverResult;
use crate::session::handle::SessionHandle;
use crate::support;

/// Provider of Firefox-specific commands.
#[derive(Debug, Clone)]
pub struct FirefoxTools {
    /// The underlying session handle.
    pub handle: Arc<SessionHandle>,
}

impl FirefoxTools {
    /// Create a new `FirefoxTools` struct.
    ///
    /// # Example:
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::support::block_on;
    /// use thirtyfour::extensions::addons::firefox::FirefoxTools;
    /// #
    /// # fn main() -> WebDriverResult<()> {
    /// #     block_on(async {
    /// #         // NOTE: this only tests creation of FirefoxTools, so chrome is fine.
    /// #         //       The test environment currently doesn't support firefox.
    /// #         let caps = DesiredCapabilities::chrome();
    /// #         let driver = WebDriver::new("http://localhost:4444", caps).await?;
    /// let tools = FirefoxTools::new(driver.handle.clone());
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

    /// Install the specified firefox add-on.
    pub async fn install_addon(&self, path: &str, temporary: Option<bool>) -> WebDriverResult<()> {
        self.handle
            .cmd(FirefoxCommand::InstallAddon {
                path: path.to_string(),
                temporary,
            })
            .await?;
        Ok(())
    }

    /// Take a full-page screenshot of the current window and return it as PNG bytes.
    pub async fn full_screenshot_as_png(&self) -> WebDriverResult<Vec<u8>> {
        let r = self.handle.cmd(FirefoxCommand::FullScreenshot {}).await?;
        let encoded: String = r.value()?;
        Ok(BASE64_STANDARD.decode(encoded)?)
    }

    /// Take a full-page screenshot of the current window and write it to the specified filename.
    pub async fn full_screenshot(&self, path: &Path) -> WebDriverResult<()> {
        let png = self.full_screenshot_as_png().await?;
        support::write_file(path, png).await?;
        Ok(())
    }
}
