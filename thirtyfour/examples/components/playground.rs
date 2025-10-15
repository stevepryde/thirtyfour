//! Run as follows:
//!
//!     cargo run --example wikipedia

use std::time::Duration;

use thirtyfour::{
    components::{Component, ElementResolver},
    prelude::*,
    resolve,
    stringmatch::StringMatchable,
    support::sleep,
};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let caps = DesiredCapabilities::chrome();
    let server_url = "http://localhost:9515";
    start_webdriver_process(server_url, &caps, true)?;
    let driver = WebDriver::new(server_url, caps).await?;
    driver.goto("https://play.rust-lang.org").await?;

    let base_elem = driver.query(By::Id("playground")).single().await?;
    let page: PlaygroundPage = base_elem.into();

    let text = "components are awesome";
    resolve!(page.editor).write_text(text).await?;
    resolve!(page.header).run().await?;
    resolve!(page.output).verify_text(text).await?;

    // Sleep so you as the user can see what it did.
    sleep(Duration::from_secs(1)).await;

    // Always explicitly close the browser. This prevents the executor from being blocked
    driver.quit().await?;

    Ok(())
}

#[derive(Component)]
pub struct PlaygroundPage {
    // div[id = 'playground']
    base: WebElement,
    #[by(xpath = ".//div[@data-test-id = 'header']")]
    header: ElementResolver<Header>,
    #[by(xpath = ".//div[contains(@class, 'ace_editor')]")]
    editor: ElementResolver<Editor>,
    #[by(xpath = ".//div[@data-test-id = 'output']")]
    output: ElementResolver<Output>,
}

#[derive(Component, Clone)]
pub struct Header {
    base: WebElement,
    #[by(custom = resolve_run_button)]
    button_run: ElementResolver<WebElement>,
}

/// Example of custom resolver.
///
/// NOTE: This particular example could be done with a single XPath but sometimes you
///       may want something more complex.
async fn resolve_run_button(elem: WebElement) -> WebDriverResult<WebElement> {
    elem.query(By::Tag("button")).with_text("Run".match_partial().case_insensitive()).first().await
}

impl Header {
    pub async fn run(&self) -> WebDriverResult<()> {
        resolve!(self.button_run).click().await
    }
}

#[derive(Component, Clone)]
pub struct Editor {
    base: WebElement,
    #[by(tag = "textarea")]
    typing_area: ElementResolver<WebElement>,
    #[by(class = "ace_content")]
    content: ElementResolver<WebElement>,
}

impl Editor {
    pub async fn write_text(&self, text: &str) -> WebDriverResult<()> {
        let elem = resolve!(self.typing_area);
        elem.send_keys(Key::Control + "a").await?;

        // The element takes a brief moment to actually clear, so wait for the resulting text.
        let elem_content = resolve!(self.content);
        elem_content.wait_until().has_text("".match_full()).await?;

        // NOTE: The editor will auto-complete the closing curly brace.
        let code = format!(
            r#"fn main() {{
println!("{}");"#,
            text
        );
        elem.send_keys(code).await?;

        // Wait for the element to show this new text.
        elem_content.wait_until().has_text(text.match_partial()).await?;
        Ok(())
    }
}

#[derive(Component, Clone)]
pub struct Output {
    base: WebElement,
    #[by(xpath = ".//div[@data-test-id = 'output-stdout']//code")]
    stdout: ElementResolver<WebElement>,
}

impl Output {
    pub async fn verify_text(&self, text: &str) -> WebDriverResult<()> {
        resolve!(self.stdout)
            .wait_until()
            .error("timed out waiting for stdout text")
            .has_text(text.to_string())
            .await
    }
}
