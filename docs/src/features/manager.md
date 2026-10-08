# WebDriver Manager

`thirtyfour` automatically downloads and runs the appropriate webdriver
binary (`chromedriver`, `geckodriver`, `msedgedriver`) for your
installed browser, so you don't have to manage the driver process
yourself. The simple form is `WebDriver::managed()`:

```rust
use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> WebDriverResult<()> {
    let driver = WebDriver::managed(DesiredCapabilities::chrome()).await?;
    driver.goto("https://www.rust-lang.org/").await?;
    driver.quit().await?;
    Ok(())
}
```

The first run downloads a matching driver into your system cache
directory; subsequent runs reuse the cached binary. The driver
subprocess starts when you create the session and is torn down when
your last `WebDriver` handle drops.

Supported browsers: Chrome / Chromium, Firefox, Microsoft Edge, and
Safari (macOS only — uses the system `safaridriver`, no download).

The rest of this chapter covers what else the manager can do — version
pinning, sharing one manager across many sessions, supplying a
pre-installed driver binary, and observing what's happening as the
manager works.

## Picking A Driver Version

`WebDriver::managed(caps)` returns a builder. Awaiting it directly uses the
defaults; chained methods customize what gets downloaded:

```rust
# use thirtyfour::prelude::*;
# use thirtyfour::manager::BrowserKind;
# async fn run() -> WebDriverResult<()> {
let caps = DesiredCapabilities::chrome();

// Default: match the locally-installed browser.
let driver = WebDriver::managed(caps.clone()).await?;

// Latest stable from upstream metadata.
let driver = WebDriver::managed(caps.clone()).latest().await?;

// Pin a specific version (full or major-only for Chrome/Edge).
let driver = WebDriver::managed(caps.clone()).exact("126").await?;

// Read `browserVersion` from the capabilities object.
let driver = WebDriver::managed(caps.clone()).from_caps().await?;

// Skip the download/cache flow entirely — use an already-installed
// driver binary at this path. See "Using A Pre-Installed Driver" below.
let driver = WebDriver::managed(caps)
    .driver_binary(BrowserKind::Chrome, "/usr/local/bin/chromedriver")
    .await?;
# Ok(()) }
```

> **Note on Firefox:** Firefox releases and `geckodriver` releases don't
> share version numbers (Firefox is on `150.x` while `geckodriver` is on
> `0.36.0`). The manager picks a compatible `geckodriver` from an embedded
> compatibility table. For `.exact(...)` on Firefox, pass a `geckodriver`
> tag like `"0.36.0"` — not a Firefox version.

## Sharing One Manager Across Sessions

Each `WebDriver::managed(caps)` call constructs its own manager and
spawns its own driver subprocess. To share one manager — and the driver
subprocesses it owns — across many sessions, construct it explicitly
with `WebDriverManager::builder()` and call `.launch(caps)` for each
session:

```rust
# use thirtyfour::prelude::*;
# use thirtyfour::manager::WebDriverManager;
# async fn run() -> WebDriverResult<()> {
let manager = WebDriverManager::builder().latest().build();

// One manager, multiple browsers.
let chrome  = manager.launch(DesiredCapabilities::chrome()).await?;
let firefox = manager.launch(DesiredCapabilities::firefox()).await?;
# Ok(()) }
```

A single manager can drive any combination of supported browsers; it
spawns one driver subprocess per `(browser, version, host)` combination
as needed, and reuses an existing subprocess when a later `.launch()`
call matches one that's still alive.

## Configuration

The most useful builder methods (all available on both
`WebDriverManager::builder()` and `WebDriver::managed(caps)`):

| Method                            | Purpose                                               |
| --------------------------------- | ----------------------------------------------------- |
| `.latest()`                       | Use the latest stable driver from upstream metadata.  |
| `.match_local()`                  | Match the locally-installed browser (default).        |
| `.from_caps()`                    | Read `browserVersion` from the capabilities.          |
| `.exact("126")`                   | Pin a specific driver version.                        |
| `.driver_binary(browser, path)`   | Use an already-installed driver binary; skip the download/cache flow for that browser. |
| `.driver_arg(browser, arg)`       | Append an argument to that browser's driver process.  |
| `.cache_dir(path)`                | Override the on-disk driver cache.                    |
| `.host(addr)`                     | Bind the driver to an address other than `127.0.0.1`. |
| `.download_timeout(d)`            | Cap upstream metadata + download time (default 60s).  |
| `.ready_timeout(d)`               | Cap how long to wait for `/status` (default 30s).     |
| `.offline()` / `.online()`        | Forbid / allow downloads (default: online).           |
| `.stdio(StdioMode::Inherit)`      | Show driver stdout/stderr in the parent terminal.     |
| `.on_status(fn)`                  | Attach a permanent status-event subscriber.           |
| `.on_driver_log(fn)`              | Attach a permanent driver-log subscriber.             |

The default cache directory is `<system cache dir>/thirtyfour/drivers`.

### Driver Arguments

Use `.driver_arg()` for driver-server options such as ChromeDriver's
`--verbose`. Arguments in capabilities configure the browser itself.
Repeated calls append arguments in order, only for the specified browser;
each argument is passed directly without shell expansion or splitting.
Do not override manager-controlled flags such as `--port`, `--allowed-ips`,
or `--websocket-port`.

```rust
# use thirtyfour::prelude::*;
# use thirtyfour::manager::{BrowserKind, StdioMode};
# async fn run() -> WebDriverResult<()> {
let driver = WebDriver::managed(DesiredCapabilities::chrome())
    .driver_arg(BrowserKind::Chrome, "--verbose")
    .stdio(StdioMode::Inherit)
    .await?;
driver.quit().await?;
# Ok(()) }
```

`StdioMode::Inherit` displays driver output in the parent terminal. With
the default `StdioMode::Tracing`, output goes to `tracing` at debug level
and to `.on_driver_log()` subscribers. Register a callback on the builder
to capture startup logs before a session exists.

## Using A Pre-Installed Driver

If you'd rather manage the driver binary yourself — for instance because
you ship a pinned `chromedriver` in a CI image — point the manager at it
with `.driver_binary(browser, path)`:

```rust
# use thirtyfour::prelude::*;
# use thirtyfour::manager::{WebDriverManager, BrowserKind};
# async fn run() -> WebDriverResult<()> {
let manager = WebDriverManager::builder()
    .driver_binary(BrowserKind::Chrome, "/usr/local/bin/chromedriver")
    .build();
let driver = manager.launch(DesiredCapabilities::chrome()).await?;
# Ok(()) }
```

Bare command names (e.g. `"chromedriver"`) are resolved against the OS
`PATH`. The version-resolution and download/cache flow is skipped for
that browser; the binary is spawned as-is. If it doesn't match the
installed browser's version, expect a runtime error from the driver
when the session is started.

## Offline Mode

If the driver you want is already in the cache, you can launch with no
network access at all:

```rust
# use thirtyfour::prelude::*;
# use thirtyfour::manager::WebDriverManager;
# async fn run() -> WebDriverResult<()> {
let manager = WebDriverManager::builder().offline().build();
let driver = manager.launch(DesiredCapabilities::chrome()).await?;
# Ok(()) }
```

In offline mode, a cache miss returns an error rather than reaching out
to the network.

## Observing What The Manager Is Doing

The manager emits structured `Status` events at every step (resolving the
version, hitting the cache, downloading, spawning the process, waiting
for `/status`, starting / ending sessions, shutting the driver down).
These events are also forwarded to the `tracing` ecosystem under the
`thirtyfour::manager` target — for human-readable logs, just install a
`tracing-subscriber` and you're done.

For programmatic access, register a subscriber:

```rust
# use thirtyfour::prelude::*;
# use thirtyfour::manager::{WebDriverManager, Status};
# async fn run() -> WebDriverResult<()> {
let manager = WebDriverManager::builder()
    .on_status(|s: &Status| println!("manager: {s}"))
    .build();
let driver = manager.launch(DesiredCapabilities::chrome()).await?;
# Ok(()) }
```

You can also subscribe to raw stdout/stderr lines from the driver process
itself via `WebDriverManager::on_driver_log` (manager-wide) or
`WebDriver::on_driver_log` (just one session's driver).

## Further Reading

See the [`thirtyfour::manager`](https://docs.rs/thirtyfour/latest/thirtyfour/manager/index.html)
module documentation for the full API, including
[`WebDriverManager`](https://docs.rs/thirtyfour/latest/thirtyfour/manager/struct.WebDriverManager.html),
[`WebDriverManagerBuilder`](https://docs.rs/thirtyfour/latest/thirtyfour/manager/struct.WebDriverManagerBuilder.html),
[`DriverVersion`](https://docs.rs/thirtyfour/latest/thirtyfour/manager/enum.DriverVersion.html),
and the [`Status`](https://docs.rs/thirtyfour/latest/thirtyfour/manager/enum.Status.html)
event vocabulary.
