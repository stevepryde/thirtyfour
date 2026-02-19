//! WebDriver BiDi WebExtension API for managing browser extensions.
//!
//! This module provides a browser-agnostic way to install and uninstall
//! extensions using the WebDriver BiDi protocol.
//!
//! # Example
//!
//! ```ignore
//! use thirtyfour::prelude::*;
//! use thirtyfour::extensions::webextension::WebExtension;
//!
//! # async fn example() -> WebDriverResult<()> {
//! let caps = DesiredCapabilities::chrome();
//! let driver = WebDriver::new("http://localhost:4444", caps).await?;
//!
//! // Create a WebExtension instance
//! let webext = WebExtension::new(driver.handle.clone());
//!
//! // Install an extension from a file
//! let extension_id = webext.install_from_file(std::path::Path::new("/path/to/extension.crx")).await?;
//!
//! // ... use the extension ...
//!
//! // Uninstall the extension
//! webext.uninstall(&extension_id).await?;
//!
//! driver.quit().await?;
//! # Ok(())
//! # }
//! ```

mod webextension;
mod webextensioncommand;

pub use webextension::WebExtension;
pub use webextensioncommand::WebExtensionCommand;
