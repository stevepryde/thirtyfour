//! Run as follows:
//!
//!     cargo run --example wikipedia

use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let caps = DesiredCapabilities::chrome();
    let server_url = "http://localhost:9515";
    start_webdriver_process(server_url, &caps, true)?;
    let driver = WebDriver::new(server_url, caps).await?;

    // Navigate to https://wikipedia.org.
    driver.goto("https://wikipedia.org").await?;

    let elem_form = driver.query(By::Id("search-form")).nowait().single().await?;

    // Find element from element using multiple selectors.
    // Each selector will be executed once per poll iteration.
    // The first element to match will be returned.
    let elem_text = elem_form
        .query(By::Css("thiswont.match"))
        .or(By::Id("searchInput"))
        .desc("search input")
        .first()
        .await?;

    // Type in the search terms.
    elem_text.send_keys("selenium").await?;

    // Click the search button. Optionally name the element to make error messages more readable.
    let elem_button =
        elem_form.query(By::Css("button[type='submit']")).desc("search button").first().await?;
    elem_button.click().await?;

    // Wait until the button no longer exists (two different ways).
    elem_button.wait_until().error("Timed out waiting for button to become stale").stale().await?;
    driver.query(By::Css("button[type='submit']")).nowait().not_exists().await?;

    // Look for header to implicitly wait for the page to load.
    driver.query(By::ClassName("firstHeading")).first().await?;
    assert_eq!(driver.title().await?, "Selenium - Wikipedia");

    // Always explicitly close the browser. This prevents the executor from being blocked
    driver.quit().await?;

    Ok(())
}
