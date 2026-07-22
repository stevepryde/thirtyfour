# AI/LLM Quickstart

Use this page as the compact source of truth when asking an AI coding tool to
write `thirtyfour` automation. It keeps the reliable defaults in one place and
links to the deeper documentation when a task needs more detail.

## Minimal Setup

```toml
[dependencies]
thirtyfour = "THIRTYFOUR_CRATE_VERSION"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The default `thirtyfour` features include the local
[`WebDriver::managed`](../features/manager.md) setup used below. Use
[`WebDriver::new` or `WebDriver::builder`](../appendix/manual-webdriver.md)
instead when a remote Selenium or Grid service owns the driver process.
Install Chrome before running the test; the manager downloads a matching
`chromedriver` automatically on first use.

## Starter Test

This example is marked `no_run` because `example.test` represents your
application and cannot be exercised as written.

```rust,no_run
use thirtyfour::{
    prelude::*,
    testing::{BrowserTestError, run_browser_test},
};

#[tokio::test]
async fn saves_profile_settings() -> Result<(), BrowserTestError> {
    let mut caps = DesiredCapabilities::chrome();
    caps.set_headless()?;
    run_browser_test(WebDriver::managed(caps), |driver| async move {
        driver.goto("https://example.test/settings").await?;

        let form = driver
            .query(By::Testid("profile-settings"))
            .desc("profile settings form")
            .single()
            .await?;

        form.query(By::Testid("display-name"))
            .desc("display name input")
            .single()
            .await?
            .send_keys("Ada")
            .await?;

        let save_button = form
            .query(By::Testid("save-profile"))
            .desc("save profile button")
            .single()
            .await?;
        save_button.wait_until().clickable().await?;
        save_button.click().await?;

        // Assert the user-visible outcome. This query polls until the saved
        // state appears, so it replaces a brittle fixed sleep.
        driver
            .query(By::Testid("settings-status"))
            .with_text("Saved")
            .and_displayed()
            .desc("saved settings status")
            .single()
            .await?;

        Ok(())
    })
    .await
}
```

## Rules For Generated Automation

- Prefer app-owned `By::Testid` selectors, then stable semantic CSS. Match
  visible text when the copy is the behavior under test; use XPath only when
  CSS cannot reasonably express the target.
- Use `query()` for normal lookup, `.single()` when uniqueness is a page
  contract, and `.desc(...)` on important user-facing elements.
- Never use a fixed sleep for readiness. Poll for the required state with
  `query()` or use `wait_until()` on an element you already have.
- Scope queries through a container or Component, and assert a user-visible
  outcome rather than only checking that a click returned successfully.
- Prefer `run_browser_test` in tests so asynchronous cleanup still runs after
  early errors and panicking assertions. Otherwise explicitly quit every
  session. Do not run independent concurrent flows through clones of the same
  `WebDriver`.
- On an error path, call `FailureArtifactCollector::capture()` before
  cleanup for bounded, best-effort URL, title, screenshot, source, and log
  context. See the diagnostics recipe for capability and feature limits.
- When selector debugging needs page structure, prefer the bounded,
  redaction-aware [semantic page outline](../recipes/page-snapshots.md) over an
  unbounded page-source dump.

The copyable [Reliable AI-Generated Tests](./reliable-tests.md) checklist is the
review gate for generated code. Apply it before accepting a test.

## Deeper Documentation

- [Task-Oriented Recipes](../recipes/index.md) — copyable starting points for
  common application flows, browser contexts, diagnostics, CDP, and BiDi.
- [Semantic Page Outlines](../recipes/page-snapshots.md) — bounded,
  privacy-conscious structure for selector design and failure diagnosis.
- [Selenium And Playwright Translation Guide](../tools/selenium-playwright.md) —
  map familiar APIs to queries, waits, Components, and managed sessions without
  inventing APIs from another ecosystem.
- [Element Queries](../features/queries.md) — stable selectors, descriptions,
  filters, scoping, cardinality, and intentional one-shot `find()` calls.
- [Waiting For Element Changes](../features/waiting.md) — state waits for an
  element that has already been located.
- [Components](../features/components.md) — reusable UI areas with private
  selectors and resilient element resolution.
- [WebDriver Manager](../features/manager.md) — local driver downloads,
  process lifetime, configuration, and separate sessions.
- [Failure Artifacts And Logs](../recipes/diagnostics.md) — bounded portable
  failure context suitable for CI and AI-assisted debugging.
- [Chrome DevTools Protocol](../cdp/overview.md) — Chromium-only typed commands
  and optional event support.
- [WebDriver BiDi](../bidi/overview.md) — cross-browser bidirectional commands
  and events, with feature and capability opt-in.
