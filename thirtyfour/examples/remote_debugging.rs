//! Requires chromedriver running on port 9515:
//!
//!     chrome --remote-debugging-port=9222 --user-data-dir="C:\Users\username\my-browser-profile\"
//!
//! Run as follows:
//!
//!     cargo run --example remote_debugging

use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let mut caps = DesiredCapabilities::chrome();
    caps.set_debugger_address("localhost:9222")?;
    let server_url = "http://localhost:9515";
    start_webdriver_process(server_url, &caps, true)?;
    let driver = WebDriver::new(server_url, caps).await?;
    driver.goto("https://www.baidu.com").await?;
    Ok(())
}
