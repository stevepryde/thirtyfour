//! Run as follows:
//!
//!     cargo run --example driver_logging
//!
//! This example uses `WebDriver::managed` (default `manager` feature), which
//! auto-downloads the matching `chromedriver` for your installed Chrome,
//! starts it locally, and shuts it down when the `WebDriver` is dropped.

use thirtyfour::manager::BrowserKind;
use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let driver = WebDriver::managed(DesiredCapabilities::chrome())
        .driver_arg(BrowserKind::Chrome, "--verbose")
        // Print all output from chromedriver itself.
        .on_driver_log(|f| println!("Chromedriver: {}", f.line))
        // Print status output from WebDriverManager
        .on_status(|s| println!("Manager: {s}"))
        .await?;
    // Navigate to something.
    driver.goto("about:blank").await?;

    // Always explicitly close the browser. This prevents the executor from being blocked
    driver.quit().await?;

    Ok(())
}
