//! BiDi Permissions API for managing browser permissions.
//!
//! This module provides a browser-agnostic way to set permission states
//! using the WebDriver BiDi protocol. Useful for headless automation
//! where permission prompts would block execution.
//!
//! # Example
//!
//! ```ignore
//! use thirtyfour::prelude::*;
//! use thirtyfour::extensions::permissions::{Permissions, PermissionState};
//!
//! # async fn example() -> WebDriverResult<()> {
//! let caps = DesiredCapabilities::chrome();
//! let driver = WebDriver::new("http://localhost:4444", caps).await?;
//!
//! let permissions = Permissions::new(driver.handle.clone());
//!
//! // Grant geolocation permission for a specific origin
//! permissions.set_permission("geolocation", PermissionState::Granted, Some("https://example.com")).await?;
//!
//! // Grant all common extension permissions
//! permissions.grant_extension_permissions("chrome-extension://abc123").await?;
//!
//! driver.quit().await?;
//! # Ok(())
//! # }
//! ```

mod permissions;
mod permissionscommand;

pub use permissions::{extension_permissions, PermissionState, Permissions};
pub use permissionscommand::PermissionsCommand;
