//! Auto-download and lifetime-managed local WebDriver process management.
//!
//! Enabled via the `manager` feature (default-on). When enabled, you can use
//! [`WebDriver::managed`] to launch a session with no external driver server:
//!
//! ```no_run
//! # use thirtyfour::prelude::*;
//! # async fn run() -> WebDriverResult<()> {
//! let driver = WebDriver::managed(DesiredCapabilities::chrome()).await?;
//! driver.goto("https://www.rust-lang.org/").await?;
//! driver.quit().await?;
//! # Ok(()) }
//! ```
//!
//! For more control (multi-browser, custom cache dir, offline mode, etc.) construct a
//! [`WebDriverManager`] explicitly:
//!
//! ```no_run
//! # use thirtyfour::prelude::*;
//! # use thirtyfour::manager::WebDriverManager;
//! # async fn run() -> WebDriverResult<()> {
//! let mgr = WebDriverManager::builder().latest().build();
//! let chrome  = mgr.launch(DesiredCapabilities::chrome()).await?;
//! let firefox = mgr.launch(DesiredCapabilities::firefox()).await?;
//! # Ok(()) }
//! ```
//!
//! # Shutdown and temporary profiles
//!
//! Call [`WebDriver::quit`](crate::WebDriver::quit) explicitly. After the last
//! managed session successfully quits, Chrome and Edge drivers are asked to
//! shut down gracefully so they can delete temporary profiles and extension
//! directories. Each successful quit first waits for its generated profile to
//! disappear while retaining the process lease, since the DELETE reply can
//! precede session destruction. [`WebDriverManagerBuilder::shutdown_timeout`](crate::manager::WebDriverManagerBuilder::shutdown_timeout) bounds the grace
//! period for that wait and final process shutdown together (10 seconds by
//! default), followed by force-kill and process reaping. Explicit user profiles,
//! detached browsers and debugger attachments are not polled or deleted.
//! On Windows with Chromium driver version 136 or newer, generated-profile
//! sessions default `quitGracefully` to true so Chrome/Edge can close child
//! processes before profile deletion. An explicit caller value is preserved.
//! Older or unknown driver versions retain their existing browser-quit behavior;
//! cleanup can still fail if another process retains a Windows file handle.
//! Firefox and Safari still use force-kill after the session ends.
//!
//! Cancellation, session-creation failure, and dropping without explicit quit
//! can still reach the synchronous force-kill fallback and leave temporary
//! files. A failed DELETE request is returned to the caller without releasing
//! the process lease, permitting retry. `leak()` skips session cleanup but does
//! not detach a managed subprocess: it still dies when its last lease drops.
//!
//! [`WebDriver::managed`]: crate::WebDriver::managed
//! [`WebDriverManager`]: crate::manager::WebDriverManager

mod browser;
mod download;
mod error;
#[allow(clippy::module_inception)]
mod manager;
mod process;
mod status;
mod version;

#[cfg(test)]
mod tests;

pub use browser::BrowserKind;
pub use error::ManagerError;
pub use manager::{WebDriverManager, WebDriverManagerBuilder};
pub use process::StdioMode;
pub use status::{
    DriverId, DriverLogLine, DriverLogSubscription, DriverStream, Status, Subscription,
    VersionSource,
};
pub use version::DriverVersion;

/// Crate-internal access to types that have to be reachable from `web_driver.rs`
/// (e.g. for the typed downcast in `WebDriver::driver_id`) but should not be
/// part of the public API.
#[doc(hidden)]
pub(crate) mod manager_internal {
    pub(crate) use super::manager::SessionGuard;
}
