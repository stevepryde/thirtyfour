#![allow(dead_code)]

use std::{
    net::SocketAddr,
    sync::{Arc, OnceLock},
    thread::JoinHandle,
};

use rstest::fixture;
use thirtyfour::manager::WebDriverManager;
use thirtyfour::prelude::*;
use thirtyfour::support::block_on;
use thirtyfour::{ChromeCapabilities, FirefoxCapabilities};
use tokio::sync::{Semaphore, SemaphorePermit};

static SERVER: OnceLock<Arc<JoinHandle<()>>> = OnceLock::new();
static LOGINIT: OnceLock<()> = OnceLock::new();

const ASSETS_DIR: &str = "tests/test_html";
const PORT: u16 = 8081;

/// Build the typed Chrome capabilities used by the integration tests.
fn make_chrome_caps() -> ChromeCapabilities {
    let mut caps = DesiredCapabilities::chrome();
    caps.set_headless().unwrap();
    caps.set_no_sandbox().unwrap();
    caps.set_disable_gpu().unwrap();
    caps.set_disable_dev_shm_usage().unwrap();
    caps.add_arg("--no-sandbox").unwrap();
    caps
}

/// Build the typed Firefox capabilities used by the integration tests.
fn make_firefox_caps() -> FirefoxCapabilities {
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless().unwrap();
    caps
}

/// Starts the web server.
pub fn start_server() -> Arc<JoinHandle<()>> {
    SERVER
        .get_or_init(|| {
            let handle = std::thread::spawn(move || {
                let rt =
                    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                rt.block_on(async {
                    tracing::debug!("starting web server on http://localhost:{PORT}");
                    let addr = SocketAddr::from(([127, 0, 0, 1], PORT));
                    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
                    let app = axum::Router::new()
                        .fallback_service(tower_http::services::ServeDir::new(ASSETS_DIR));
                    axum::serve(listener, app).await.unwrap();
                });
            });
            Arc::new(handle)
        })
        .clone()
}

pub fn init_logging() {
    LOGINIT.get_or_init(|| {
        use tracing_subscriber::{EnvFilter, fmt, prelude::*};
        tracing_subscriber::registry()
            .with(fmt::layer())
            .with(EnvFilter::from_default_env())
            .init();
    });
}

/// Get the global limiter mutex.
pub fn get_limiter() -> &'static Semaphore {
    static LIMITER: Semaphore = Semaphore::const_new(1);

    &LIMITER
}

/// Locks the Firefox browser for exclusive use.
///
/// This ensures there is only ever one Firefox browser running at a time.
pub async fn lock_firefox(browser: &str) -> Option<SemaphorePermit<'static>> {
    if browser == "firefox" {
        Some(get_limiter().acquire().await.unwrap())
    } else {
        None
    }
}

/// Per-binary, per-browser `Arc<WebDriverManager>`. Sharing one builder is
/// cheap and keeps the binary's launches funnelling through a single
/// dedup map — useful if tests ever overlap.
///
/// Note: with `--test-threads=1` the dedup map's `Weak` always fails to
/// upgrade between tests (the previous test's `Arc` is gone before the
/// next launches), so chromedriver is respawned per test. We tried
/// pinning a long-lived "anchor" session in a static `OnceCell` to keep
/// the `Arc` alive, but the anchor's tokio-bound resources (HTTP pool,
/// driver stdio readers) end up tied to the *first* test's runtime, and
/// when that runtime drops at end-of-test the anchor goes stale. On
/// Windows that wedges chromedriver hard enough to hang the next test.
/// The cross-binary download cache (default `cache_dir`) is the
/// remaining win.
fn manager_for(browser: &str) -> &'static Arc<WebDriverManager> {
    static CHROME: OnceLock<Arc<WebDriverManager>> = OnceLock::new();
    static FIREFOX: OnceLock<Arc<WebDriverManager>> = OnceLock::new();
    let cell = match browser {
        "chrome" => &CHROME,
        "firefox" => &FIREFOX,
        b => unimplemented!("unsupported browser backend {b}"),
    };
    cell.get_or_init(|| {
        let builder = WebDriverManager::builder();
        let builder = if browser == "chrome" && github_actions() {
            builder.on_driver_log(|line| {
                eprintln!("ChromeDriver {} {}: {}", line.driver_id, line.stream, line.line);
            })
        } else {
            builder
        };
        builder.build()
    })
}

fn github_actions() -> bool {
    std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true")
}

/// Launch the specified browser via the shared per-binary [`WebDriverManager`].
pub async fn launch_browser(browser: &str) -> WebDriver {
    tracing::debug!("launching browser {browser}");
    let mgr = manager_for(browser);
    match browser {
        "chrome" => {
            launch_managed_chrome(make_chrome_caps()).await.expect("Failed to launch chrome")
        }
        "firefox" => mgr.launch(make_firefox_caps()).await.expect("Failed to launch firefox"),
        b => unimplemented!("unsupported browser backend {b}"),
    }
}

/// Launch a Chrome session against the binary's shared [`WebDriverManager`].
/// Used from test files that don't go through [`TestHarness`] — the managed
/// CDP and `ElementQuery` integration tests — so all sibling tests funnel
/// through one manager instead of constructing a fresh one each call.
pub async fn launch_managed_chrome(caps: ChromeCapabilities) -> WebDriverResult<WebDriver> {
    let mgr = manager_for("chrome");
    mgr.launch(caps).await
}

/// Launch a Firefox session against the binary's shared [`WebDriverManager`].
/// Mirror of [`launch_managed_chrome`] for cross-browser tests
/// (notably the BiDi suite, which is supported by both browsers).
pub async fn launch_managed_firefox(caps: FirefoxCapabilities) -> WebDriverResult<WebDriver> {
    let mgr = manager_for("firefox");
    mgr.launch(caps).await
}

/// Launch a BiDi-enabled session for the browser named by the
/// `THIRTYFOUR_BROWSER` env var (`"chrome"` | `"firefox"`, default Chrome).
///
/// Used by the BiDi integration test suites — BiDi is a W3C cross-browser
/// protocol so the tests run on both via the env var.
pub async fn launch_managed_bidi() -> WebDriverResult<WebDriver> {
    let browser = std::env::var("THIRTYFOUR_BROWSER").unwrap_or_else(|_| "chrome".to_string());
    match browser.as_str() {
        "firefox" => {
            let mut caps = DesiredCapabilities::firefox();
            caps.set_headless()?;
            caps.enable_bidi()?;
            launch_managed_firefox(caps).await
        }
        _ => {
            let mut caps = DesiredCapabilities::chrome();
            caps.set_headless()?;
            caps.set_no_sandbox()?;
            caps.set_disable_gpu()?;
            caps.set_disable_dev_shm_usage()?;
            caps.add_arg("--no-sandbox")?;
            caps.enable_bidi()?;
            launch_managed_chrome(caps).await
        }
    }
}

/// Returns the active browser backend name for BiDi tests.
pub fn bidi_browser() -> String {
    std::env::var("THIRTYFOUR_BROWSER").unwrap_or_else(|_| "chrome".to_string())
}

/// Helper struct for running tests.
pub struct TestHarness {
    browser: String,
    server: Arc<JoinHandle<()>>,
    driver: Option<WebDriver>,
    guard: Option<SemaphorePermit<'static>>,
}

impl TestHarness {
    /// Create a new TestHarness instance.
    pub async fn new(browser: &str) -> Self {
        let server = start_server();
        let guard = lock_firefox(browser).await;
        let driver = Some(launch_browser(browser).await);
        Self {
            browser: browser.to_string(),
            server,
            driver,
            guard,
        }
    }

    /// Get the browser name.
    pub fn browser(&self) -> &str {
        &self.browser
    }

    /// Get the WebDriver instance.
    pub fn driver(&self) -> &WebDriver {
        self.driver.as_ref().expect("the driver to still be active")
    }

    /// Disable auto-closing the browser when the TestHarness is dropped.
    pub fn disable_auto_close(mut self) -> Self {
        if let Some(driver) = self.driver.take() {
            let _ = driver.leak();
        }
        self
    }
}

/// Fixture for running tests.
#[fixture]
pub fn test_harness() -> TestHarness {
    let browser = std::env::var("THIRTYFOUR_BROWSER").unwrap_or_else(|_| "chrome".to_string());
    init_logging();
    block_on(TestHarness::new(&browser))
}

pub fn sample_page_url() -> String {
    format!("http://localhost:{PORT}/sample_page.html")
}

pub fn other_page_url() -> String {
    format!("http://localhost:{PORT}/other_page.html")
}

pub fn drag_to_url() -> String {
    format!("http://localhost:{PORT}/drag_to.html")
}
