# Failure Artifacts And Logs

Diagnostics are most useful when their collection does not prevent browser
cleanup or replace the original test error. These recipes keep those outcomes
separate.

## Capture A Bounded Failure Report

Create `FailureArtifactCollector` before the test body so it can record live
managed-driver-process output. Its defaults capture URL, title, PNG bytes, a
page-source excerpt capped at 64 KiB, and the newest 200 browser and driver
process log entries. Driver-provided log text also has per-entry and total
UTF-8 byte limits.

```rust,no_run
use thirtyfour::{
    prelude::*,
    testing::{BrowserTestError, FailureArtifactCollector, run_browser_test},
};

#[tokio::test]
async fn diagnoses_checkout_failure() -> Result<(), BrowserTestError> {
    run_browser_test(
        WebDriver::managed(DesiredCapabilities::chrome()),
        |driver| async move {
            let collector = FailureArtifactCollector::new(&driver);
            let result: WebDriverResult<()> = async {
                driver.goto("https://example.test/checkout").await?;
                driver
                    .query(By::Testid("order-confirmation"))
                    .with_text("Order received")
                    .single()
                    .await?;
                Ok(())
            }
            .await;

            if result.is_err() {
                // Capture while the session is alive, before the runner quits it.
                eprintln!("{}", collector.capture().await);
            }
            result
        },
    )
    .await
}
```

Capture is best-effort and never fail-fast. Every field is independently
`Captured`, `Unavailable`, or `Disabled`, so an unsupported browser-log endpoint
does not hide a screenshot or source excerpt. The `Display` report is intended
for CI and AI debugging: it prints only the screenshot byte count, never PNG
bytes or base64. The screenshot bytes remain available on `FailureArtifacts`
for writing to a CI artifact file.

This example captures returned `Err` values only. A panicking assertion unwinds
past the `capture()` call, although `run_browser_test` still catches the unwind
to quit the session before resuming it. Prefer fallible query checks when you
need this report, or add a panic-aware wrapper or test-harness hook that calls
`capture()` before cleanup.

Browser logs require logging capabilities set before session creation and are
not supported by every driver. Managed-driver-process logs are available only
with the `manager` feature, for managed sessions, and only after collector
attachment. The manager may reuse one driver process across sessions, so this
process-scoped stream can include lines from another concurrent session. Avoid
sharing a managed driver process when per-session log isolation is required.
CDP and BiDi diagnostics are intentionally outside this portable baseline.
Customize or disable capture through `FailureArtifactConfig`; configured byte
and entry limits remain hard bounds and UTF-8 truncation never splits a code
point.

URLs, titles, source excerpts, screenshots, and logs can still contain secrets
or personal data. Disable fields that are unsafe for your application, apply
application-specific redaction before uploading artifacts, and do not treat
the default size bounds as a privacy filter.

For a compact selector-oriented view instead of raw page source, use the
bounded, redaction-aware [semantic page outline](./page-snapshots.md).

## Take A Screenshot On Failure

```rust,no_run
use std::path::Path;
use thirtyfour::prelude::*;

#[tokio::test]
async fn captures_a_failed_checkout() -> WebDriverResult<()> {
    let driver = WebDriver::managed(DesiredCapabilities::chrome()).await?;

    let test_result: WebDriverResult<()> = async {
        driver.goto("https://example.test/checkout").await?;
        driver
            .query(By::Testid("order-confirmation"))
            .with_text("Order received")
            .and_displayed()
            .desc("completed order confirmation")
            .single()
            .await?;
        Ok(())
    }
    .await;

    let screenshot_result = if test_result.is_err() {
        driver.screenshot(Path::new("checkout-failure.png")).await
    } else {
        Ok(())
    };
    let quit_result = driver.quit().await;

    test_result?;
    screenshot_result?;
    quit_result
}
```

Create the artifact directory before the test if the path includes one. The
ordering above always attempts `quit()`, then gives the original test failure
priority over a screenshot or cleanup failure. For a larger suite, move this
shape into the test fixture rather than duplicating it in every test.

This result-capture shape handles returned `Err` values, not Rust panics from
`assert!`. Prefer query-based assertions like the one above, or use a
panic-aware test harness when screenshotting panics is also required.

## Read Browser And Driver Logs

The legacy browser-log endpoint is implemented by Chromium drivers, not
`geckodriver`. Enable `goog:loggingPrefs` before creating the session. Driver
process logs are separate and are available for locally managed sessions.

```rust,no_run
use std::time::Duration;
use tokio::time::{Instant, sleep};
use thirtyfour::{LoggingPrefsLogLevel, prelude::*};

#[tokio::test]
async fn records_chromium_logs() -> WebDriverResult<()> {
    let mut caps = DesiredCapabilities::chrome();
    caps.set_browser_log_level(LoggingPrefsLogLevel::All)?;
    let driver = WebDriver::managed(caps)
        .on_driver_log(|entry| eprintln!("chromedriver: {}", entry.line))
        .await?;

    let test_result: WebDriverResult<()> = async {
        driver
            .goto("data:text/html,<script>console.error('checkout failed')</script>")
            .await?;

        // Log delivery is asynchronous and each call drains the buffer, so
        // accumulate entries until the expected marker arrives or times out.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut entries = Vec::new();
        loop {
            entries.extend(driver.browser_log().await?);
            if entries.iter().any(|entry| entry.message.contains("checkout failed")) {
                break;
            }
            if Instant::now() >= deadline {
                return Err(WebDriverError::Timeout("browser log marker did not arrive".into()));
            }
            sleep(Duration::from_millis(100)).await;
        }

        for entry in entries {
            println!("[{}] {}", entry.level, entry.message);
        }
        Ok(())
    }
    .await;

    let quit_result = driver.quit().await;
    test_result?;
    quit_result
}
```

For cross-browser console events, prefer the BiDi log subscription described
in [BiDi Events](../bidi/events.md). For manager status events, log buffering,
and per-session driver subscriptions, see
[WebDriver Manager](../features/manager.md#observing-what-the-manager-is-doing).
Driver-process subscriptions require the default-on `manager` feature and the
manager's default `StdioMode::Tracing`; `Inherit` and `Null` do not create the
stdout/stderr pumps that feed callbacks. The short polling interval above is
intentional timing behavior for asynchronous log delivery, not a page-readiness
sleep.
