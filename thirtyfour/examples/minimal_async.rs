//! Run as follows:
//!
//!     cargo run --example minimal_async

use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let caps = DesiredCapabilities::chrome();
    let server_url = "http://localhost:9515";
    start_webdriver_process(server_url, &caps, true)?;
    let driver = WebDriver::new(server_url, caps).await?;
    // Navigate to https://wikipedia.org.
    driver.goto("https://wikipedia.org").await?;

    // Find element.
    let elem_form = driver.find(By::Id("search-form")).await?;

    // Find element from element.
    let elem_text = elem_form.find(By::Id("searchInput")).await?;

    // Type in the search terms.
    elem_text.send_keys("selenium").await?;

    // Click the search button.
    let elem_button = elem_form.find(By::Css("button[type='submit']")).await?;
    elem_button.click().await?;

    // Always explicitly close the browser. This prevents the executor from being blocked
    driver.quit().await?;

    Ok(())
}
