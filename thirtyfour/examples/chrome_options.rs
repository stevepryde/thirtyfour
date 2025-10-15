//! Run as follows:
//!
//!     cargo run --example chrome_options

use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut caps = DesiredCapabilities::chrome();
    caps.insert_browser_option(
        "prefs",
        serde_json::json!({
            "profile.default_content_settings": {
                "images": 2
            },
            "profile.managed_default_content_settings": {
                "images": 2
            }
        }),
    )?;
    let server_url = "http://localhost:9515";
    start_webdriver_process(server_url, &caps, true)?;
    let driver = WebDriver::new(server_url, caps).await?;

    // Navigate to https://wikipedia.org.
    driver.goto("https://wikipedia.org").await?;

    // Always explicitly close the browser. This prevents the executor from being blocked
    driver.quit().await?;

    Ok(())
}
