---
title: Quickstart
description: Install thirtyfour and run your first browser session
---

# Quickstart

Add thirtyfour to your project:

```toml
[dependencies]
thirtyfour = "0.38"
tokio = { version = "1", features = ["macros", "rtime"] }
```

The example below is from the project README. It starts a managed Chrome session, navigates to Wikipedia, finds the search form, and types a search term:

```rust
use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> WebDriverResult<()> {
    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::managed(caps).await?;

    // Navigate to https://wikipedia.org.
    driver.goto("https://wikipedia.org").await?;
    let elem_form = driver
        .query(By::Id("search-form"))
        .desc("Wikipedia search form")
        .single()
        .await?;

    // Querying from an element scopes the search to its subtree.
    let elem_text = elem_form
        .query(By::Id("searchInput"))
        .desc("Wikipedia search input")
        .single()
        .await?;

    // Type in the search terms.
    elem_text.send_keys("selenium").await?;
    Ok(())
}
```

`WebDriver::managed` downloads and lifetime-manages its own chromedriver or geckodriver. Set `THIRTYFOUR_BROWSER=firefox` to switch browsers. Feature flags worth knowing: `rustls-tls` (default), `native-tls`, `component` (default), `cdp` (default), `cdp-events`, and `bidi`.
