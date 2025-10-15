//! Run as follows:
//!
//!     cargo run --example tokio_async

use thirtyfour::prelude::*;

fn main() -> color_eyre::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(run())
}

async fn run() -> color_eyre::Result<()> {
    // The use of color_eyre gives much nicer error reports, including making
    // it much easier to locate where the error occurred.
    color_eyre::install()?;

    let caps = DesiredCapabilities::chrome();
    let server_url = "http://localhost:9515";
    start_webdriver_process(server_url, &caps, true)?;
    let driver = WebDriver::new(server_url, caps).await?;
    // Navigate to https://wikipedia.org.
    driver.goto("https://wikipedia.org").await?;
    let elem_form = driver.find(By::Id("search-form")).await?;

    // Find element from element.
    let elem_text = elem_form.find(By::Id("searchInput")).await?;

    // Type in the search terms.
    elem_text.send_keys("selenium").await?;

    // Click the search button.
    let elem_button = elem_form.find(By::Css("button[type='submit']")).await?;
    elem_button.click().await?;

    // Look for header to implicitly wait for the page to load.
    driver.find(By::ClassName("firstHeading")).await?;
    assert_eq!(driver.title().await?, "Selenium - Wikipedia");

    // Always explicitly close the browser. This prevents the executor from being blocked
    driver.quit().await?;

    Ok(())
}
