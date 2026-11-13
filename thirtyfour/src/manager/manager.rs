use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::Formatter;
use std::future::{Future, IntoFuture};
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard as StdMutexGuard, Weak};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{Mutex, OnceCell};
use url::Url;

use crate::Capabilities;
use crate::common::config::WebDriverConfig;
use crate::error::{WebDriverError, WebDriverResult};
use crate::extensions::query::IntoElementPoller;
use crate::session::DriverGuard;
use crate::session::create::start_session;
use crate::session::handle::SessionHandle;
use crate::session::http::create_reqwest_client;
use crate::web_driver::WebDriver;

use super::browser::{BrowserKind, detect_local_version};
use super::download::{DownloadConfig, Mirror, ensure_driver, resolve_version};
use super::error::ManagerError;
use super::process::{ManagedDriverProcess, SpawnConfig, SpawnContext, StdioMode};
use super::status::{
    DriverId, DriverLogCallback, DriverLogLine, DriverLogSubscription, Emitter, LogSubscribers,
    Status, StatusCallback, Subscription,
};
use super::version::DriverVersion;

const DEFAULT_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Default cache directory: `<cache_dir>/thirtyfour/drivers`, falling back to the
/// system temp dir if no cache dir is available.
fn default_cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("thirtyfour").join("drivers")
}

/// `(browser, local_version, caps_version)` — see `WebDriverManager::resolved_versions`.
type ResolvedVersionKey = (BrowserKind, Option<String>, Option<String>);

/// Auto-download and lifetime-managed local WebDriver process manager.
///
/// See the [module documentation](super) for the high-level usage. The two
/// common entry points are [`WebDriver::managed`] (one-shot, fresh manager
/// per call) and [`WebDriverManager::builder`] (for sharing one manager
/// across many sessions).
///
/// [`WebDriver::managed`]: crate::WebDriver::managed
pub struct WebDriverManager {
    pub(crate) cfg: ResolvedConfig,
    /// Configuration applied to every WebDriver session launched by this manager.
    pub(crate) web_driver_config: WebDriverConfig,
    /// HTTP client used for fetching upstream metadata and binaries.
    download_client: reqwest::Client,
    /// Map from `(browser, resolved_version)` to a Weak handle on a live driver.
    drivers: Mutex<HashMap<DriverKey, Weak<ManagedDriverProcess>>>,
    /// Cache for the local-browser version probe. The installed browser
    /// doesn't change for the lifetime of a manager, so probing
    /// `<browser> --version` once per `(browser, custom_binary)` is enough.
    /// Skips repeated subprocess spawns and avoids transient
    /// `Command::output` failures on busy CI runners.
    local_versions: Mutex<HashMap<(BrowserKind, Option<String>), String>>,
    /// Cache for upstream version resolution, keyed by
    /// `(browser, local_version, caps_version)` — the inputs that can vary
    /// between launches (the requested [`DriverVersion`] is fixed per
    /// manager). One cell per key, so a slow resolution only holds up
    /// launches waiting on the same key. Unused for [`DriverVersion::Latest`].
    resolved_versions: Mutex<HashMap<ResolvedVersionKey, Arc<OnceCell<String>>>>,
    /// Status-event emitter (also forwards to `tracing`).
    pub(crate) emitter: Emitter,
    /// Manager-wide driver-log subscribers — propagated into every spawned
    /// `ManagedDriverProcess` so subscribers added at any time see lines from
    /// every live driver.
    pub(crate) log_subscribers: LogSubscribers,
    /// Monotonic counter for [`DriverId`].
    next_driver_id: Arc<AtomicU64>,
}

impl std::fmt::Debug for WebDriverManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebDriverManager")
            .field("cfg", &self.cfg)
            .field("web_driver_config", &self.web_driver_config)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub(crate) struct ResolvedConfig {
    pub version: DriverVersion,
    pub cache_dir: PathBuf,
    pub host: IpAddr,
    pub download_timeout: Duration,
    pub ready_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub offline: bool,
    pub mirror: Mirror,
    pub stdio: StdioMode,
    /// Windows only: give the driver process its own console window instead
    /// of suppressing it with `CREATE_NO_WINDOW`.
    pub show_console_window: bool,
    /// Per-browser driver-binary overrides. When a browser appears here,
    /// `ensure_driver` skips download/cache and spawns the supplied binary
    /// directly.
    pub driver_paths: HashMap<BrowserKind, PathBuf>,
    /// Additional command-line arguments, applied only to the selected browser's driver.
    pub driver_args: HashMap<BrowserKind, Vec<OsString>>,
}

impl std::fmt::Debug for ResolvedConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedConfig")
            .field("version", &self.version)
            .field("cache_dir", &self.cache_dir)
            .field("host", &self.host)
            .field("download_timeout", &self.download_timeout)
            .field("ready_timeout", &self.ready_timeout)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("offline", &self.offline)
            .field("stdio", &self.stdio)
            .field("show_console_window", &self.show_console_window)
            .field("driver_paths", &self.driver_paths)
            .field("driver_args", &self.driver_args)
            .finish_non_exhaustive()
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub(super) struct DriverKey {
    pub(super) browser: BrowserKind,
    pub(super) version: String,
    pub(super) host: IpAddr,
}

impl DriverGuard for ManagedDriverProcess {}

/// Per-session guard held by `SessionHandle::driver_guard`. Keeps the
/// underlying [`ManagedDriverProcess`] alive for the lifetime of the session,
/// and emits [`Status::SessionEnded`] when dropped.
pub(crate) struct SessionGuard {
    driver: StdMutex<Option<Arc<ManagedDriverProcess>>>,
    emitter: Emitter,
    browser: BrowserKind,
    session_id: String,
    cleanup_profile: Option<PathBuf>,
    shutdown_timeout: Duration,
}

impl std::fmt::Debug for SessionGuard {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionGuard")
            .field("browser", &self.browser)
            .field("session_id", &self.session_id)
            .field("driver", &self.driver())
            .finish_non_exhaustive()
    }
}

impl SessionGuard {
    fn driver(&self) -> StdMutexGuard<'_, Option<Arc<ManagedDriverProcess>>> {
        self.driver.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn driver_id(&self) -> Option<DriverId> {
        self.driver().as_ref().map(|driver| driver.driver_id)
    }

    pub(crate) fn subscribe_log<F>(&self, f: F) -> Option<DriverLogSubscription>
    where
        F: Fn(&DriverLogLine) + Send + Sync + 'static,
    {
        self.driver().as_ref().map(|driver| driver.subscribe_log(f))
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.emitter.emit(Status::SessionEnded {
            browser: self.browser,
            session_id: self.session_id.clone(),
        });
    }
}

impl DriverGuard for SessionGuard {
    fn release(&self) -> Pin<Box<dyn Future<Output = WebDriverResult<()>> + Send + '_>> {
        Box::pin(async move {
            let driver = self.driver().take();
            if let Some(driver) = driver {
                let deadline = tokio::time::Instant::now() + self.shutdown_timeout;
                if let Some(profile) = &self.cleanup_profile {
                    wait_for_chromium_cleanup(profile, &driver.url(), &self.session_id, deadline)
                        .await;
                }
                if let Some(process) = Arc::into_inner(driver) {
                    // Profile cleanup and process exit share one grace budget.
                    process
                        .shutdown_with_budget(
                            deadline.saturating_duration_since(tokio::time::Instant::now()),
                        )
                        .await
                        .map_err(WebDriverError::from)?;
                }
            }
            Ok(())
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// A session-scoped command is a Chromium-specific cleanup barrier: it is
// dispatched on the same session thread, behind DELETE and Session::Terminate.
// If the command-thread map entry has already been removed, destroying its
// SessionThreadInfo joins that thread first. Either route's invalid-session
// response therefore follows the session destructor, including its finite
// profile-deletion attempts. /status is NOT such a barrier.
//
// This observes completion; it cannot make ChromeDriver retry failed deletion.
// Retain profile polling as a fallback for unsupported/unexpected responses.
async fn wait_for_chromium_cleanup(
    profile: &Path,
    driver_url: &str,
    session_id: &str,
    deadline: tokio::time::Instant,
) {
    if !matches!(profile.try_exists(), Ok(true)) || tokio::time::Instant::now() >= deadline {
        return;
    }
    let barrier = async {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .ok()?;
        let mut url = Url::parse(driver_url).ok()?;
        url.path_segments_mut().ok()?.extend(["session", session_id, "url"]);
        let response = client.get(url).send().await.ok()?;
        if response.status() != reqwest::StatusCode::NOT_FOUND {
            return None;
        }
        let body: Value = response.json().await.ok()?;
        (body.pointer("/value/error").and_then(Value::as_str) == Some("invalid session id"))
            .then_some(())
    };
    let confirmed = tokio::select! {
        _ = wait_for_profile_cleanup(profile, deadline) => return,
        result = tokio::time::timeout_at(deadline, barrier) => matches!(result, Ok(Some(()))),
    };
    if confirmed {
        if matches!(profile.try_exists(), Ok(true)) {
            tracing::warn!(
                "managed Chromium session cleanup completed but its profile remains; files left untouched"
            );
        }
    } else {
        wait_for_profile_cleanup(profile, deadline).await;
    }
}

// Conservative fallback when the session-thread barrier cannot be confirmed.
// The DELETE reply alone is insufficient: it precedes session destruction.
async fn wait_for_profile_cleanup(profile: &Path, deadline: tokio::time::Instant) {
    loop {
        match profile.try_exists() {
            Ok(false) => return,
            Err(error) => {
                tracing::warn!(%error, "unable to inspect managed Chromium profile cleanup");
                return;
            }
            Ok(true) => {}
        }
        if tokio::time::Instant::now() >= deadline {
            tracing::warn!("managed Chromium profile still exists at shutdown deadline");
            return;
        }
        tokio::time::sleep_until(std::cmp::min(
            deadline,
            tokio::time::Instant::now() + Duration::from_millis(20),
        ))
        .await;
    }
}

fn chromium_profile_options(browser: BrowserKind, requested: &Capabilities) -> bool {
    let key = match browser {
        BrowserKind::Chrome => "goog:chromeOptions",
        BrowserKind::Edge => "ms:edgeOptions",
        _ => return false,
    };
    let Some(options) = requested.get(key) else {
        return true;
    };
    if options.get("detach").and_then(Value::as_bool) == Some(true)
        || options.get("debuggerAddress").is_some()
    {
        return false;
    }
    !options.get("args").and_then(Value::as_array).is_some_and(|args| {
        args.iter().filter_map(Value::as_str).any(|arg| {
            let arg = arg.trim_start_matches('-');
            arg == "user-data-dir" || arg.starts_with("user-data-dir=")
        })
    })
}

// Chromium 136 introduced quitGracefully. On Windows, force-terminating only
// the main Chrome process can leave descendants holding chrome_debug.log,
// outliving ChromeDriver's finite profile-deletion retries. Let Chrome close
// its children normally. Preserve explicit caller choices and older drivers.
fn prefer_graceful_browser_quit(
    browser: BrowserKind,
    version: &str,
    caps: &mut Capabilities,
) -> WebDriverResult<()> {
    if !chromium_profile_options(browser, caps)
        || !version
            .split('.')
            .next()
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|major| major >= 136)
    {
        return Ok(());
    }
    let key = match browser {
        BrowserKind::Chrome => "goog:chromeOptions",
        BrowserKind::Edge => "ms:edgeOptions",
        _ => return Ok(()),
    };
    if caps.get(key).is_none() {
        caps.set(key, serde_json::json!({}))?;
    }
    if let Some(options) = caps.get_mut(key).and_then(Value::as_object_mut) {
        options.entry("quitGracefully").or_insert(Value::Bool(true));
    }
    Ok(())
}

fn generated_profile(browser: BrowserKind, returned: &Capabilities) -> Option<PathBuf> {
    let vendor = match browser {
        BrowserKind::Chrome => "chrome",
        BrowserKind::Edge => "msedge",
        _ => return None,
    };
    let path = PathBuf::from(returned.get(vendor)?.get("userDataDir")?.as_str()?);
    // Never wait for user-owned directories or delete anything ourselves.
    let name = path.file_name()?.to_str()?;
    (name.starts_with("scoped_dir") || name.contains(".scoped_dir.")).then_some(path)
}

/// Builder for [`WebDriverManager`]. The same type is used both via
/// `WebDriverManager::builder()` and (with capabilities preloaded) via
/// [`WebDriver::managed`]. See the [module documentation](super) for examples.
///
/// [`WebDriver::managed`]: crate::WebDriver::managed
#[derive(Default)]
pub struct WebDriverManagerBuilder {
    pub(crate) version: DriverVersion,
    pub(crate) cache_dir: Option<PathBuf>,
    pub(crate) host: Option<IpAddr>,
    pub(crate) download_timeout: Option<Duration>,
    pub(crate) ready_timeout: Option<Duration>,
    pub(crate) shutdown_timeout: Option<Duration>,
    pub(crate) offline: Option<bool>,
    pub(crate) mirror: Option<Mirror>,
    pub(crate) stdio: Option<StdioMode>,
    pub(crate) show_console_window: Option<bool>,
    /// Per-browser driver-binary overrides registered via
    /// [`WebDriverManagerBuilder::driver_binary`].
    pub(crate) driver_paths: HashMap<BrowserKind, PathBuf>,
    /// Per-browser driver-process arguments registered via
    /// [`WebDriverManagerBuilder::driver_arg`].
    pub(crate) driver_args: HashMap<BrowserKind, Vec<OsString>>,
    /// Status subscribers registered before `build`.
    pub(crate) status_subscribers: Vec<StatusCallback>,
    /// Driver-log subscribers registered before `build`.
    pub(crate) log_subscribers: Vec<DriverLogCallback>,
    /// Configuration applied to every WebDriver session launched by the manager.
    pub(crate) web_driver_config: WebDriverConfig,
    /// Set when constructed via `WebDriver::managed(caps)`.
    pub(crate) preloaded_caps: Option<Capabilities>,
}

impl std::fmt::Debug for WebDriverManagerBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebDriverManagerBuilder")
            .field("version", &self.version)
            .field("cache_dir", &self.cache_dir)
            .field("host", &self.host)
            .field("download_timeout", &self.download_timeout)
            .field("ready_timeout", &self.ready_timeout)
            .field("shutdown_timeout", &self.shutdown_timeout)
            .field("offline", &self.offline)
            .field("stdio", &self.stdio)
            .field("show_console_window", &self.show_console_window)
            .field("driver_paths", &self.driver_paths)
            .field("driver_args", &self.driver_args)
            .field("status_subscribers", &self.status_subscribers.len())
            .field("log_subscribers", &self.log_subscribers.len())
            .field("web_driver_config", &self.web_driver_config)
            .finish_non_exhaustive()
    }
}

impl Clone for WebDriverManagerBuilder {
    fn clone(&self) -> Self {
        Self {
            version: self.version.clone(),
            cache_dir: self.cache_dir.clone(),
            host: self.host,
            download_timeout: self.download_timeout,
            ready_timeout: self.ready_timeout,
            shutdown_timeout: self.shutdown_timeout,
            offline: self.offline,
            mirror: self.mirror.clone(),
            stdio: self.stdio,
            show_console_window: self.show_console_window,
            driver_paths: self.driver_paths.clone(),
            driver_args: self.driver_args.clone(),
            status_subscribers: self.status_subscribers.iter().map(Arc::clone).collect(),
            log_subscribers: self.log_subscribers.iter().map(Arc::clone).collect(),
            web_driver_config: self.web_driver_config.clone(),
            preloaded_caps: self.preloaded_caps.clone(),
        }
    }
}

impl WebDriverManagerBuilder {
    /// Set the configuration used by every [`WebDriver`] session launched by
    /// the resulting manager.
    pub fn config(mut self, config: WebDriverConfig) -> Self {
        self.web_driver_config = config;
        self
    }

    /// Set the element poller used by every [`WebDriver`] session launched by
    /// the resulting manager.
    ///
    /// This replaces the poller in the current [`WebDriverConfig`], including
    /// a configuration previously supplied via
    /// [`WebDriverManagerBuilder::config`].
    pub fn poller(mut self, poller: Arc<dyn IntoElementPoller + Send + Sync>) -> Self {
        self.web_driver_config.poller = poller;
        self
    }

    /// Pick a driver version. See [`DriverVersion`] for variants.
    pub fn version(mut self, v: DriverVersion) -> Self {
        self.version = v;
        self
    }

    /// Use the latest stable driver from upstream metadata, checked on every
    /// launch.
    ///
    /// <div class="warning">
    ///
    /// **Warning:** In offline mode, `.latest()` selects the newest cached
    /// driver, which may be older than the latest upstream release.
    ///
    /// </div>
    pub fn latest(self) -> Self {
        self.version(DriverVersion::Latest)
    }

    /// Probe the locally-installed browser and use a matching driver. This is
    /// the default; calling it explicitly is a no-op.
    pub fn match_local(self) -> Self {
        self.version(DriverVersion::MatchLocalBrowser)
    }

    /// Read `browserVersion` from the capabilities supplied to `launch()` /
    /// `WebDriver::managed()`.
    pub fn from_caps(self) -> Self {
        self.version(DriverVersion::FromCapabilities)
    }

    /// Pin a specific version (full like `"126.0.6478.126"` or major-only like `"126"`).
    pub fn exact(self, v: impl Into<String>) -> Self {
        self.version(DriverVersion::Exact(v.into()))
    }

    /// Override the cache directory. Default: `<system cache dir>/thirtyfour/drivers`.
    pub fn cache_dir(mut self, p: PathBuf) -> Self {
        self.cache_dir = Some(p);
        self
    }

    /// Override the host the driver binds to. Default: `127.0.0.1`.
    pub fn host(mut self, h: IpAddr) -> Self {
        self.host = Some(h);
        self
    }

    /// Override the timeout for upstream metadata + binary downloads. Default: 60s.
    pub fn download_timeout(mut self, d: Duration) -> Self {
        self.download_timeout = Some(d);
        self
    }

    /// Override the timeout for waiting on driver `/status` readiness. Default: 30s.
    pub fn ready_timeout(mut self, d: Duration) -> Self {
        self.ready_timeout = Some(d);
        self
    }

    /// Grace budget for generated Chrome/Edge profile cleanup after each
    /// successful session quit, plus process shutdown for the last session.
    /// Defaults to 10 seconds; successful cleanup coordination returns early, even
    /// if the driver left files behind. Zero skips the cleanup wait and graceful shutdown.
    ///
    /// The driver is killed if it has not exited within this budget. This does
    /// not bound the preceding WebDriver DELETE request or OS process reaping.
    /// Other browsers retain their force-kill shutdown behavior.
    pub fn shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = Some(timeout);
        self
    }

    /// Refuse to download anything; require the driver to already be in cache.
    ///
    /// No upstream metadata is fetched. Chrome uses the exact cached version
    /// if present, otherwise the newest cached driver of the same major.
    /// Edge major-only versions select the newest cached driver of that major;
    /// full versions must be cached exactly. Firefox requires the literal
    /// geckodriver tag for [`DriverVersion::Exact`], or the version selected by
    /// the embedded compatibility table for browser-matching modes, to be
    /// cached. [`DriverVersion::Latest`] selects the newest cached driver.
    ///
    /// <div class="warning">
    ///
    /// **Warning:** In offline mode, `.latest()` selects the newest cached
    /// driver, which may be older than the latest upstream release.
    ///
    /// </div>
    pub fn offline(self) -> Self {
        self.offline_mode(true)
    }

    /// Allow downloads. This is the default.
    pub fn online(self) -> Self {
        self.offline_mode(false)
    }

    /// Set offline mode explicitly.
    pub fn offline_mode(mut self, yes: bool) -> Self {
        self.offline = Some(yes);
        self
    }

    /// Override the Chrome-for-Testing metadata base URL.
    pub fn chrome_metadata_mirror(mut self, base: Url) -> Self {
        let m = self.mirror.get_or_insert_with(Mirror::default);
        m.chrome_metadata = base;
        self
    }

    /// Override the geckodriver release metadata base URL, used to look up
    /// the latest release: `<base>latest` must redirect to
    /// `<base>tag/v<version>`. Default:
    /// `https://github.com/mozilla/geckodriver/releases/`.
    pub fn geckodriver_metadata_mirror(mut self, base: Url) -> Self {
        let m = self.mirror.get_or_insert_with(Mirror::default);
        m.geckodriver_metadata = base;
        self
    }

    /// Override the geckodriver binary download base URL.
    pub fn geckodriver_downloads_mirror(mut self, base: Url) -> Self {
        let m = self.mirror.get_or_insert_with(Mirror::default);
        m.geckodriver_downloads = base;
        self
    }

    /// Override the msedgedriver download host.
    pub fn edge_downloads_mirror(mut self, base: Url) -> Self {
        let m = self.mirror.get_or_insert_with(Mirror::default);
        m.edge_downloads = base;
        self
    }

    /// Set how driver-process stdout/stderr is handled. Default:
    /// [`StdioMode::Tracing`].
    pub fn stdio(mut self, mode: StdioMode) -> Self {
        self.stdio = Some(mode);
        self
    }

    /// Windows only: give the driver process its own console window.
    ///
    /// By default the driver is spawned with the `CREATE_NO_WINDOW` creation
    /// flag so that no console window flashes up — this matters for GUI apps
    /// built with `#![windows_subsystem = "windows"]`, where the console
    /// subprocess would otherwise pop open a visible console. Driver output
    /// is unaffected either way: stdout/stderr are still delivered according
    /// to [`WebDriverManagerBuilder::stdio`].
    ///
    /// No-op on non-Windows platforms.
    pub fn show_console_window(mut self, yes: bool) -> Self {
        self.show_console_window = Some(yes);
        self
    }

    /// Use an already-installed driver binary at `path` for the given
    /// browser instead of resolving and downloading one. Skips the
    /// version-resolution and download/cache flow entirely; the binary is
    /// spawned as-is. Bare command names (e.g. `"chromedriver"`) are
    /// resolved against the OS `PATH`.
    ///
    /// This is intended for environments that ship their own driver — CI
    /// images with pinned binaries, sandboxes with no network access, or
    /// users who simply prefer to manage driver versions themselves. If
    /// the binary doesn't match the installed browser's version, expect a
    /// runtime error from the driver when the session is started.
    ///
    /// Call once per browser to override drivers for a multi-browser
    /// manager:
    ///
    /// ```no_run
    /// # use thirtyfour::manager::{WebDriverManager, BrowserKind};
    /// let mgr = WebDriverManager::builder()
    ///     .driver_binary(BrowserKind::Chrome, "/usr/local/bin/chromedriver")
    ///     .driver_binary(BrowserKind::Firefox, "/usr/local/bin/geckodriver")
    ///     .build();
    /// ```
    pub fn driver_binary(mut self, browser: BrowserKind, path: impl Into<PathBuf>) -> Self {
        self.driver_paths.insert(browser, path.into());
        self
    }

    /// Append one command-line argument for the given browser's driver process.
    ///
    /// Arguments are passed directly to the driver, without shell expansion or
    /// splitting. Call repeatedly to add multiple arguments; their order is
    /// preserved. They apply to every process spawned for this browser by the
    /// manager, including when using [`WebDriverManagerBuilder::driver_binary`].
    ///
    /// These configure the driver server, whereas arguments in browser
    /// capabilities configure the browser itself. Do not override
    /// manager-controlled flags such as `--port`, `--allowed-ips`, or
    /// `--websocket-port`.
    ///
    /// To display ChromeDriver's verbose output directly in the parent terminal:
    ///
    /// ```no_run
    /// # use thirtyfour::prelude::*;
    /// # use thirtyfour::manager::{BrowserKind, StdioMode};
    /// # async fn run() -> WebDriverResult<()> {
    /// let driver = WebDriver::managed(DesiredCapabilities::chrome())
    ///     .driver_arg(BrowserKind::Chrome, "--verbose")
    ///     .stdio(StdioMode::Inherit)
    ///     .await?;
    /// driver.quit().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// With the default [`StdioMode::Tracing`], output is instead forwarded to
    /// `tracing` at debug level and to [`WebDriverManagerBuilder::on_driver_log`]
    /// subscribers, including during startup.
    pub fn driver_arg(mut self, browser: BrowserKind, arg: impl Into<OsString>) -> Self {
        self.driver_args.entry(browser).or_default().push(arg.into());
        self
    }

    /// Register a closure to receive every [`Status`] event emitted by the
    /// resulting manager. Equivalent to calling
    /// [`WebDriverManager::subscribe`] right after `build`, except the
    /// subscriber is attached for the manager's whole lifetime — not removable.
    pub fn on_status<F>(mut self, f: F) -> Self
    where
        F: Fn(&Status) + Send + Sync + 'static,
    {
        self.status_subscribers.push(Arc::new(f));
        self
    }

    /// Register a closure to receive every [`DriverLogLine`] from drivers
    /// spawned by the resulting manager. Equivalent to
    /// [`WebDriverManager::on_driver_log`] applied right after `build`, except
    /// the subscriber is attached for the manager's whole lifetime — not
    /// removable.
    pub fn on_driver_log<F>(mut self, f: F) -> Self
    where
        F: Fn(&DriverLogLine) + Send + Sync + 'static,
    {
        self.log_subscribers.push(Arc::new(f));
        self
    }

    /// Build the manager.
    pub fn build(self) -> Arc<WebDriverManager> {
        let cfg = ResolvedConfig {
            version: self.version,
            cache_dir: self.cache_dir.unwrap_or_else(default_cache_dir),
            host: self.host.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            download_timeout: self.download_timeout.unwrap_or(DEFAULT_DOWNLOAD_TIMEOUT),
            ready_timeout: self.ready_timeout.unwrap_or(DEFAULT_READY_TIMEOUT),
            shutdown_timeout: self.shutdown_timeout.unwrap_or(Duration::from_secs(10)),
            offline: self.offline.unwrap_or(false),
            mirror: self.mirror.unwrap_or_default(),
            stdio: self.stdio.unwrap_or_default(),
            show_console_window: self.show_console_window.unwrap_or(false),
            driver_paths: self.driver_paths,
            driver_args: self.driver_args,
        };
        let emitter = Emitter::new();
        for cb in self.status_subscribers {
            // `forget` so the subscriber lives for the manager's lifetime.
            std::mem::forget(emitter.add_arc(cb));
        }
        let log_subscribers = LogSubscribers::new();
        for cb in self.log_subscribers {
            std::mem::forget(log_subscribers.add_arc(cb));
        }
        Arc::new(WebDriverManager {
            web_driver_config: self.web_driver_config,
            download_client: reqwest::Client::builder()
                .build()
                .expect("default reqwest client should always build"),
            cfg,
            drivers: Mutex::new(HashMap::new()),
            local_versions: Mutex::new(HashMap::new()),
            resolved_versions: Mutex::new(HashMap::new()),
            emitter,
            log_subscribers,
            next_driver_id: Arc::new(AtomicU64::new(0)),
        })
    }
}

impl IntoFuture for WebDriverManagerBuilder {
    type Output = WebDriverResult<WebDriver>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let caps = self
                .preloaded_caps
                .clone()
                .ok_or_else(|| WebDriverError::from(ManagerError::NoCapabilities))?;
            self.build().launch(caps).await
        })
    }
}

impl WebDriverManager {
    /// Construct an empty builder.
    pub fn builder() -> WebDriverManagerBuilder {
        WebDriverManagerBuilder::default()
    }

    /// Register a closure to receive every [`Status`] event emitted by this
    /// manager. Returns an RAII guard; dropping it removes the subscriber.
    /// `mem::forget` keeps the subscriber alive for the manager's lifetime.
    ///
    /// Subscribers are also forwarded to the `tracing` ecosystem under the
    /// `thirtyfour::manager` target — they're additive, not a replacement.
    pub fn subscribe<F>(&self, f: F) -> Subscription
    where
        F: Fn(&Status) + Send + Sync + 'static,
    {
        self.emitter.add(f)
    }

    /// Register a closure to receive every [`DriverLogLine`] from drivers
    /// spawned (or to be spawned) by this manager. Returns an RAII guard;
    /// dropping it removes the subscriber.
    pub fn on_driver_log<F>(&self, f: F) -> DriverLogSubscription
    where
        F: Fn(&DriverLogLine) + Send + Sync + 'static,
    {
        self.log_subscribers.add(f)
    }

    pub(crate) fn mint_driver_id(&self) -> DriverId {
        DriverId::from_raw(self.next_driver_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Spawn (or reuse) the appropriate driver and start a session.
    ///
    /// If a driver process for the same `(browser, resolved_version, host)` is
    /// already alive (held by another `WebDriver`), it's reused. Otherwise a
    /// new one is downloaded if needed and spawned.
    pub async fn launch(
        self: &Arc<Self>,
        capabilities: impl Into<Capabilities>,
    ) -> WebDriverResult<WebDriver> {
        let mut caps: Capabilities = capabilities.into();
        let driver = self.ensure_driver(&caps).await.map_err(WebDriverError::from)?;
        let browser = driver.browser;
        if cfg!(windows) {
            prefer_graceful_browser_quit(browser, &driver.version, &mut caps)?;
        }
        let server_url: Url = driver
            .url()
            .parse()
            .map_err(|e| WebDriverError::ParseError(format!("invalid driver url: {e}")))?;

        let config = self.web_driver_config.clone();
        let client = create_reqwest_client(config.request_timeout);
        let client_arc: Arc<dyn crate::session::http::HttpClient> = Arc::new(client);
        self.emitter.emit(Status::SessionStarting {
            browser,
            url: server_url.to_string(),
        });
        let wait_for_profile = chromium_profile_options(browser, &caps);
        let started = start_session(client_arc.as_ref(), &server_url, &config, caps).await?;
        let cleanup_profile =
            wait_for_profile.then(|| generated_profile(browser, &started.capabilities)).flatten();
        let session_id = started.session_id.clone();
        self.emitter.emit(Status::SessionStarted {
            browser,
            session_id: session_id.to_string(),
            url: server_url.to_string(),
        });
        let guard: Arc<dyn DriverGuard> = Arc::new(SessionGuard {
            driver: StdMutex::new(Some(driver)),
            emitter: self.emitter.clone(),
            browser,
            session_id: session_id.to_string(),
            cleanup_profile,
            shutdown_timeout: self.cfg.shutdown_timeout,
        });
        let handle = SessionHandle::new_with_config_guard_and_caps(
            client_arc,
            server_url,
            session_id,
            config,
            Some(guard),
            started.capabilities,
        )?;
        Ok(WebDriver {
            handle: Arc::new(handle),
        })
    }

    /// Ensure a driver is running and return an `Arc<ManagedDriverProcess>`
    /// whose existence keeps the child alive.
    async fn ensure_driver(
        &self,
        caps: &Capabilities,
    ) -> Result<Arc<ManagedDriverProcess>, ManagerError> {
        let browser = BrowserKind::from_capabilities(caps)?;
        self.emitter.emit(Status::BrowserKindResolved {
            browser,
        });

        // Manual binary override: skip the resolve/download flow entirely
        // and spawn the supplied path directly. Version string for the
        // DriverKey + Status events is synthesized from the path so manual
        // entries don't collide with cache-keyed (browser, version) tuples.
        if let Some(binary) = self.cfg.driver_paths.get(&browser).cloned() {
            let version = format!("manual:{}", binary.display());
            return self.spawn_or_reuse(browser, version, &binary).await;
        }

        // For MatchLocalBrowser we probe the binary up front (potentially using a
        // capabilities-supplied path). Cached per `(browser, custom_binary)` so
        // we don't re-spawn `<browser> --version` for every session.
        let local = match self.cfg.version {
            DriverVersion::MatchLocalBrowser => {
                let custom = browser.binary_from_caps(caps);
                let key = (browser, custom.clone());
                let mut cache = self.local_versions.lock().await;
                let version = match cache.get(&key) {
                    Some(v) => v.clone(),
                    None => {
                        let v = detect_local_version(browser, custom.as_deref(), &self.emitter)?;
                        cache.insert(key, v.clone());
                        v
                    }
                };
                Some(version)
            }
            _ => None,
        };
        let caps_version = caps.get("browserVersion").and_then(Value::as_str);

        let resolved = self.resolve_version_cached(browser, local.as_deref(), caps_version).await?;

        // Fast path: live driver matching `(browser, resolved, host)` already exists.
        {
            let key = DriverKey {
                browser,
                version: resolved.clone(),
                host: self.cfg.host,
            };
            let map = self.drivers.lock().await;
            if let Some(existing) = map.get(&key).and_then(Weak::upgrade) {
                self.emitter.emit(Status::DriverReused {
                    browser,
                    version: resolved,
                    url: existing.url(),
                });
                return Ok(existing);
            }
        }

        let driver_path = ensure_driver(
            &self.download_client,
            &self.download_config(),
            browser,
            &resolved,
            &self.emitter,
        )
        .await?;
        self.spawn_or_reuse(browser, resolved, &driver_path.binary).await
    }

    /// Resolve the driver version for a launch. Done once per
    /// `(browser, local_version, caps_version)`; later launches reuse the
    /// result without emitting the resolution [`Status`] events again.
    /// [`DriverVersion::Latest`] is resolved on every launch.
    pub(crate) async fn resolve_version_cached(
        &self,
        browser: BrowserKind,
        local: Option<&str>,
        caps_version: Option<&str>,
    ) -> Result<String, ManagerError> {
        let resolve = || {
            let cfg = self.download_config();
            async move {
                resolve_version(
                    &self.download_client,
                    &cfg,
                    browser,
                    &self.cfg.version,
                    local,
                    caps_version,
                    &self.emitter,
                )
                .await
            }
        };
        if self.cfg.version == DriverVersion::Latest {
            return resolve().await;
        }

        let key = (browser, local.map(str::to_owned), caps_version.map(str::to_owned));
        let cell = self.resolved_versions.lock().await.entry(key).or_default().clone();
        // Concurrent launches for the same key wait for the first one instead
        // of each fetching the same metadata. A failed resolution leaves the
        // cell empty; the next launch tries again.
        cell.get_or_try_init(resolve).await.cloned()
    }

    fn download_config(&self) -> DownloadConfig {
        DownloadConfig {
            cache_dir: self.cfg.cache_dir.clone(),
            mirror: self.cfg.mirror.clone(),
            download_timeout: self.cfg.download_timeout,
            offline: self.cfg.offline,
        }
    }

    /// Cache-check, spawn, and register a managed driver process for
    /// `(browser, version, host)`.
    async fn spawn_or_reuse(
        &self,
        browser: BrowserKind,
        version: String,
        binary: &Path,
    ) -> Result<Arc<ManagedDriverProcess>, ManagerError> {
        let key = DriverKey {
            browser,
            version: version.clone(),
            host: self.cfg.host,
        };

        {
            let map = self.drivers.lock().await;
            if let Some(existing) = map.get(&key).and_then(Weak::upgrade) {
                self.emitter.emit(Status::DriverReused {
                    browser,
                    version: version.clone(),
                    url: existing.url(),
                });
                return Ok(existing);
            }
        }

        let process = ManagedDriverProcess::spawn(
            binary,
            browser,
            &SpawnConfig {
                host: self.cfg.host,
                ready_timeout: self.cfg.ready_timeout,
                shutdown_timeout: self.cfg.shutdown_timeout,
                stdio: self.cfg.stdio,
                show_console_window: self.cfg.show_console_window,
                args: self.cfg.driver_args.get(&browser).cloned().unwrap_or_default(),
            },
            SpawnContext {
                driver_id: self.mint_driver_id(),
                version: &version,
                emitter: &self.emitter,
                manager_log_subscribers: self.log_subscribers.clone(),
            },
        )
        .await?;

        let arc = Arc::new(process);
        let mut map = self.drivers.lock().await;
        // Re-check after re-locking — another caller may have raced us.
        if let Some(existing) = map.get(&key).and_then(Weak::upgrade) {
            self.emitter.emit(Status::DriverReused {
                browser,
                version,
                url: existing.url(),
            });
            return Ok(existing);
        }
        map.insert(key, Arc::downgrade(&arc));
        Ok(arc)
    }
}

#[cfg(test)]
mod profile_cleanup_tests {
    #[test]
    fn graceful_browser_default_is_version_gated_and_preserves_caller_options() {
        for (browser, key) in
            [(BrowserKind::Chrome, "goog:chromeOptions"), (BrowserKind::Edge, "ms:edgeOptions")]
        {
            for version in ["135.0.1", "unknown"] {
                let mut caps = Capabilities::new();
                prefer_graceful_browser_quit(browser, version, &mut caps).unwrap();
                assert!(caps.get(key).is_none());
            }
            let mut caps = Capabilities::new();
            prefer_graceful_browser_quit(browser, "136.0.1", &mut caps).unwrap();
            assert_eq!(caps.get(key).unwrap()["quitGracefully"], true);
            caps.set(key, serde_json::json!({"quitGracefully": false})).unwrap();
            prefer_graceful_browser_quit(browser, "153.0.1", &mut caps).unwrap();
            assert_eq!(caps.get(key).unwrap()["quitGracefully"], false);
            for options in [
                serde_json::json!({"args": ["--user-data-dir=owned"]}),
                serde_json::json!({"detach": true}),
                serde_json::json!({"debuggerAddress":"localhost:1234"}),
            ] {
                caps.set(key, options.clone()).unwrap();
                prefer_graceful_browser_quit(browser, "153.0.1", &mut caps).unwrap();
                assert_eq!(caps.get(key), Some(&options));
            }
        }
    }

    use super::*;
    use serde_json::json;

    fn caps(value: Value) -> Capabilities {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn only_owned_chromium_profiles_are_eligible() {
        for (browser, key) in
            [(BrowserKind::Chrome, "goog:chromeOptions"), (BrowserKind::Edge, "ms:edgeOptions")]
        {
            assert!(chromium_profile_options(browser, &Capabilities::new()));
            assert!(chromium_profile_options(
                browser,
                &caps(json!({key: {"detach": false, "args": ["--headless"]}}))
            ));
            for options in [
                json!({"detach": true}),
                json!({"debuggerAddress": "127.0.0.1:9222"}),
                json!({"args": ["--user-data-dir=/some/profile"]}),
                json!({"args": ["--user-data-dir", "/some/profile"]}),
                json!({"args": ["user-data-dir=/some/profile"]}),
                json!({"args": ["--user-data-dir="]}),
            ] {
                assert!(
                    !chromium_profile_options(browser, &caps(json!({key: options}))),
                    "must exclude {options}"
                );
            }
        }
        assert!(!chromium_profile_options(BrowserKind::Firefox, &Capabilities::new()));
        assert!(!chromium_profile_options(BrowserKind::Safari, &Capabilities::new()));
    }

    #[test]
    fn generated_profile_requires_matching_vendor_and_temporary_name() {
        let root = tempfile::tempdir().unwrap();
        for (browser, vendor) in [(BrowserKind::Chrome, "chrome"), (BrowserKind::Edge, "msedge")] {
            for name in [
                "scoped_dir123_456",
                "com.google.Chrome.scoped_dir.ABCDEF",
                ".org.chromium.Chromium.scoped_dir.ABCDEF",
            ] {
                let path = root.path().join(name);
                assert_eq!(
                    generated_profile(browser, &caps(json!({vendor: {"userDataDir": path}}))),
                    Some(path)
                );
            }
            for invalid in
                [json!(""), json!(root.path().join("my-profile")), json!(42), Value::Null]
            {
                assert_eq!(
                    generated_profile(browser, &caps(json!({vendor: {"userDataDir": invalid}}))),
                    None
                );
            }
            assert_eq!(generated_profile(browser, &Capabilities::new()), None);
            assert_eq!(generated_profile(browser, &caps(json!({vendor: {}}))), None);
        }
        let chrome = caps(json!({"chrome": {"userDataDir": root.path().join("scoped_dir123")}}));
        assert_eq!(generated_profile(BrowserKind::Edge, &chrome), None);
        assert_eq!(generated_profile(BrowserKind::Firefox, &chrome), None);
        assert_eq!(generated_profile(BrowserKind::Safari, &chrome), None);
    }

    #[tokio::test]
    async fn absent_profile_and_zero_budget_do_not_probe_the_driver() {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(wiremock::matchers::method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;
        let root = tempfile::tempdir().unwrap();
        wait_for_chromium_cleanup(
            &root.path().join("absent"),
            &server.uri(),
            "ended",
            tokio::time::Instant::now() + Duration::from_secs(10),
        )
        .await;
        wait_for_chromium_cleanup(root.path(), &server.uri(), "ended", tokio::time::Instant::now())
            .await;
        assert!(root.path().exists());
    }

    #[tokio::test]
    async fn session_barrier_waits_for_response_but_preserves_residual_profile() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/session/ended/url"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_json(json!({"value": {"error": "invalid session id"}}))
                    .set_delay(Duration::from_millis(80)),
            )
            .expect(1)
            .mount(&server)
            .await;
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join("scoped_dir123");
        std::fs::create_dir(&profile).unwrap();
        std::fs::write(profile.join("chrome_debug.log"), "still held externally").unwrap();
        let started = tokio::time::Instant::now();
        tokio::time::timeout(
            Duration::from_secs(2),
            wait_for_chromium_cleanup(
                &profile,
                &server.uri(),
                "ended",
                started + Duration::from_secs(10),
            ),
        )
        .await
        .unwrap();
        assert!(started.elapsed() >= Duration::from_millis(80));
        assert_eq!(
            std::fs::read_to_string(profile.join("chrome_debug.log")).unwrap(),
            "still held externally"
        );
    }

    #[tokio::test]
    async fn unconfirmed_barriers_fall_back_to_bounded_profile_wait() {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        for (status, error, delay) in [
            (200, "invalid session id", Duration::ZERO),
            (404, "unknown command", Duration::ZERO),
            (500, "unknown error", Duration::ZERO),
            (404, "invalid session id", Duration::from_secs(3)),
        ] {
            let server = MockServer::start().await;
            Mock::given(wiremock::matchers::method("GET"))
                .respond_with(
                    ResponseTemplate::new(status)
                        .set_body_json(json!({"value": {"error": error}}))
                        .set_delay(delay),
                )
                .mount(&server)
                .await;
            let profile = tempfile::tempdir().unwrap();
            let started = tokio::time::Instant::now();
            let budget = Duration::from_millis(100);
            tokio::time::timeout(
                Duration::from_secs(1),
                wait_for_chromium_cleanup(profile.path(), &server.uri(), "ended", started + budget),
            )
            .await
            .unwrap();
            assert!(started.elapsed() >= budget);
            assert!(profile.path().exists());
        }
    }

    #[tokio::test]
    async fn profile_disappearance_wins_over_stalled_barrier() {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(wiremock::matchers::method("GET"))
            .respond_with(ResponseTemplate::new(404).set_delay(Duration::from_secs(30)))
            .mount(&server)
            .await;
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join("scoped_dir123");
        std::fs::create_dir(&profile).unwrap();
        let url = server.uri();
        let wait = wait_for_chromium_cleanup(
            &profile,
            &url,
            "ended",
            tokio::time::Instant::now() + Duration::from_secs(10),
        );
        tokio::pin!(wait);
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(wait.as_mut().poll(cx).is_pending()))
                .await
        );
        std::fs::remove_dir(&profile).unwrap(); // Simulate driver-owned cleanup.
        tokio::time::timeout(Duration::from_secs(1), wait).await.unwrap();
    }

    #[tokio::test]
    async fn waits_for_profile_to_disappear_without_removing_it() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("scoped_dir123");
        std::fs::create_dir(&path).unwrap();
        let wait =
            wait_for_profile_cleanup(&path, tokio::time::Instant::now() + Duration::from_secs(3));
        tokio::pin!(wait);
        // Poll once to prove it waits while the profile is present, rather
        // than relying on scheduler timing or simply asserting eventual exit.
        let pending =
            std::future::poll_fn(|cx| std::task::Poll::Ready(wait.as_mut().poll(cx).is_pending()))
                .await;
        assert!(pending);
        assert!(path.exists(), "the waiter must never remove a profile");
        std::fs::remove_dir(&path).unwrap(); // Stand in for ChromeDriver cleanup.
        tokio::time::timeout(Duration::from_secs(1), wait).await.unwrap();
    }

    #[tokio::test]
    async fn deadline_preserves_existing_profile_and_absent_profile_returns() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("scoped_dir123");
        std::fs::create_dir(&path).unwrap();
        for budget in [Duration::ZERO, Duration::from_millis(40)] {
            let started = tokio::time::Instant::now();
            tokio::time::timeout(
                Duration::from_secs(1),
                wait_for_profile_cleanup(&path, started + budget),
            )
            .await
            .unwrap();
            assert!(started.elapsed() >= budget);
            assert!(path.exists(), "timeout must not delete the profile");
        }
        let absent = root.path().join("absent");
        tokio::time::timeout(
            Duration::from_millis(100),
            wait_for_profile_cleanup(
                &absent,
                tokio::time::Instant::now() + Duration::from_secs(30),
            ),
        )
        .await
        .unwrap();
    }
}
