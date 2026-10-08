use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fs4::tokio::AsyncFileExt;
use serde::Deserialize;
use url::Url;

use super::browser::{BrowserKind, major};
use super::error::ManagerError;
use super::status::{Emitter, Status, VersionSource};
use super::version::DriverVersion;

/// Built-in upstream metadata sources.
///
/// The chromedriver download URL comes fully-qualified from the
/// Chrome-for-Testing JSON index, so there's no separate base-URL field for
/// it. Geckodriver version resolution is fully offline (driven by the
/// embedded compatibility table) so no metadata mirror is needed for it
/// — only the binary download base.
#[derive(Debug, Clone)]
pub(crate) struct Mirror {
    /// Base URL for the Chrome-for-Testing JSON metadata. Defaults to the
    /// public CfT endpoint.
    pub chrome_metadata: Url,
    /// Base URL for geckodriver binary downloads. Defaults to the public
    /// `github.com/mozilla/geckodriver/releases/download/` host.
    pub geckodriver_downloads: Url,
    /// Base URL for the msedgedriver download host.
    pub edge_downloads: Url,
}

impl Default for Mirror {
    fn default() -> Self {
        Self {
            chrome_metadata: Url::parse("https://googlechromelabs.github.io/").unwrap(),
            geckodriver_downloads: Url::parse(
                "https://github.com/mozilla/geckodriver/releases/download/",
            )
            .unwrap(),
            edge_downloads: Url::parse("https://msedgedriver.microsoft.com/").unwrap(),
        }
    }
}

/// Configuration values consumed by the download / cache logic.
pub(crate) struct DownloadConfig {
    pub cache_dir: PathBuf,
    pub mirror: Mirror,
    pub download_timeout: Duration,
    pub offline: bool,
}

/// Resolve [`DriverVersion`] to a concrete version string.
///
/// `local_version` must be supplied when the variant is `MatchLocalBrowser`;
/// `caps_version` when the variant is `FromCapabilities`. For system-managed
/// browsers (Safari) the version resolution is skipped entirely — the caller
/// is expected to short-circuit before calling this function.
///
/// For Firefox, `MatchLocalBrowser` and `FromCapabilities` apply the
/// geckodriver→Firefox compatibility table from [`geckodriver_for_firefox`]:
/// modern Firefox (≥115) gets latest geckodriver; older Firefox installs are
/// mapped to a pinned older geckodriver tag.
pub(crate) async fn resolve_version(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
    browser: BrowserKind,
    spec: &DriverVersion,
    local_version: Option<&str>,
    caps_version: Option<&str>,
    emitter: &Emitter,
) -> Result<String, ManagerError> {
    if browser == BrowserKind::Safari {
        return Ok("system".to_string());
    }

    emitter.emit(Status::DriverVersionResolving {
        browser,
        requested: spec.clone(),
    });

    let resolve_firefox_for_browser_version = |fx: &str| -> Result<String, ManagerError> {
        geckodriver_for_firefox(fx).map(str::to_owned).ok_or_else(|| {
            ManagerError::Parse(format!(
                "no geckodriver release in compatibility table covers Firefox {fx}"
            ))
        })
    };

    let resolved = match spec {
        DriverVersion::Exact(v) => match browser {
            BrowserKind::Chrome => resolve_chrome_exact(client, cfg, v).await?,
            BrowserKind::Firefox => resolve_firefox_exact(client, cfg, v).await?,
            BrowserKind::Edge => resolve_edge_exact(client, cfg, v).await?,
            BrowserKind::Safari => unreachable!("safari short-circuited above"),
        },
        DriverVersion::Latest => match browser {
            BrowserKind::Chrome => fetch_chrome_latest(client, cfg).await?,
            BrowserKind::Firefox => firefox_latest_from_table().to_owned(),
            BrowserKind::Edge => fetch_edge_latest(client, cfg).await?,
            BrowserKind::Safari => unreachable!("safari short-circuited above"),
        },
        DriverVersion::MatchLocalBrowser => {
            let v = local_version.ok_or(ManagerError::LocalBrowserNotFound {
                browser: browser.display_name(),
                hint: "local version probe returned no value",
            })?;
            match browser {
                BrowserKind::Chrome => resolve_chrome_exact(client, cfg, v).await?,
                BrowserKind::Edge => resolve_edge_exact(client, cfg, v).await?,
                BrowserKind::Firefox => resolve_firefox_for_browser_version(v)?,
                BrowserKind::Safari => unreachable!("safari short-circuited above"),
            }
        }
        DriverVersion::FromCapabilities => {
            let v = caps_version.ok_or(ManagerError::MissingCapabilityVersion)?;
            match browser {
                BrowserKind::Chrome => resolve_chrome_exact(client, cfg, v).await?,
                BrowserKind::Edge => resolve_edge_exact(client, cfg, v).await?,
                BrowserKind::Firefox => resolve_firefox_for_browser_version(v)?,
                BrowserKind::Safari => unreachable!("safari short-circuited above"),
            }
        }
    };

    emitter.emit(Status::DriverVersionResolved {
        browser,
        version: resolved.clone(),
        source: VersionSource::from_spec(spec),
    });
    Ok(resolved)
}

/// One entry in the geckodriver↔Firefox compatibility table.
struct GeckodriverRelease {
    /// Tag without the leading `v` (e.g. `"0.36.0"`).
    version: &'static str,
    /// Minimum Firefox major version supported by this geckodriver.
    min_firefox: u32,
    /// Maximum Firefox major version, if this geckodriver was superseded.
    /// `None` means "still supported" — typical for the entries at the top
    /// of the table.
    max_firefox: Option<u32>,
}

/// Embedded geckodriver compatibility table, **sorted descending by
/// geckodriver version** (highest first).
///
/// Modelled after [SeleniumHQ's `geckodriver-support.json`][upstream] but
/// embedded here so we don't hit any network endpoint to resolve a Firefox →
/// geckodriver mapping. Update when a new geckodriver release ships (roughly
/// every 6–12 months); the manager-tests CI workflow will catch staleness if
/// upstream introduces a Firefox version that no entry covers.
///
/// [upstream]: https://github.com/SeleniumHQ/selenium/blob/trunk/common/geckodriver/geckodriver-support.json
const GECKODRIVER_RELEASES: &[GeckodriverRelease] = &[
    GeckodriverRelease {
        version: "0.36.0",
        min_firefox: 128,
        max_firefox: None,
    },
    GeckodriverRelease {
        version: "0.35.0",
        min_firefox: 115,
        max_firefox: None,
    },
    GeckodriverRelease {
        version: "0.34.0",
        min_firefox: 115,
        max_firefox: None,
    },
    GeckodriverRelease {
        version: "0.33.0",
        min_firefox: 102,
        max_firefox: Some(120),
    },
    GeckodriverRelease {
        version: "0.32.2",
        min_firefox: 102,
        max_firefox: Some(120),
    },
    GeckodriverRelease {
        version: "0.31.0",
        min_firefox: 91,
        max_firefox: Some(120),
    },
    GeckodriverRelease {
        version: "0.30.0",
        min_firefox: 78,
        max_firefox: Some(90),
    },
    GeckodriverRelease {
        version: "0.29.1",
        min_firefox: 60,
        max_firefox: Some(90),
    },
];

/// Pick the best geckodriver release for the given Firefox version. Returns
/// the highest-versioned entry whose `[min_firefox, max_firefox]` range
/// covers `firefox_version`. Returns `None` when no entry matches (e.g.
/// Firefox is older than anything in the table).
fn geckodriver_for_firefox(firefox_version: &str) -> Option<&'static str> {
    let major: u32 = firefox_version.split('.').next().and_then(|s| s.parse().ok()).unwrap_or(0);
    GECKODRIVER_RELEASES
        .iter()
        .find(|r| major >= r.min_firefox && r.max_firefox.is_none_or(|max| major <= max))
        .map(|r| r.version)
}

#[derive(Deserialize)]
struct ChromeKnownGoodVersions {
    versions: Vec<ChromeVersionEntry>,
}

#[derive(Deserialize)]
struct ChromeVersionEntry {
    version: String,
    downloads: ChromeDownloads,
}

#[derive(Deserialize)]
struct ChromeDownloads {
    #[serde(default)]
    chromedriver: Vec<ChromePlatformDownload>,
}

#[derive(Deserialize, Clone)]
struct ChromePlatformDownload {
    platform: String,
    url: String,
}

async fn fetch_chrome_index(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
) -> Result<ChromeKnownGoodVersions, ManagerError> {
    let url = cfg
        .mirror
        .chrome_metadata
        .join("chrome-for-testing/known-good-versions-with-downloads.json")
        .map_err(|e| ManagerError::Parse(e.to_string()))?;
    let resp = client.get(url).timeout(cfg.download_timeout).send().await?.error_for_status()?;
    let body: ChromeKnownGoodVersions = resp.json().await?;
    Ok(body)
}

async fn fetch_chrome_latest(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
) -> Result<String, ManagerError> {
    let url = cfg
        .mirror
        .chrome_metadata
        .join("chrome-for-testing/LATEST_RELEASE_STABLE")
        .map_err(|e| ManagerError::Parse(e.to_string()))?;
    let resp = client.get(url).timeout(cfg.download_timeout).send().await?.error_for_status()?;
    Ok(resp.text().await?.trim().to_string())
}

async fn resolve_chrome_exact(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
    version: &str,
) -> Result<String, ManagerError> {
    let index = fetch_chrome_index(client, cfg).await?;
    if version.contains('.') {
        // Try exact match; fall back to latest matching the same major if the
        // exact build isn't present in CfT (common for older Chrome installs).
        if index.versions.iter().any(|v| v.version == version) {
            return Ok(version.to_string());
        }
    }
    let m = major(version);
    let mut matches: Vec<&ChromeVersionEntry> =
        index.versions.iter().filter(|v| major(&v.version) == m).collect();
    matches.sort_by(|a, b| sort_semver(&a.version, &b.version));
    matches
        .last()
        .map(|v| v.version.clone())
        .ok_or_else(|| ManagerError::Parse(format!("no chromedriver release for major {m}")))
}

fn sort_semver(a: &str, b: &str) -> std::cmp::Ordering {
    let parse =
        |s: &str| -> Vec<u32> { s.split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect() };
    parse(a).cmp(&parse(b))
}

/// `DriverVersion::Latest` for Firefox — returns the highest geckodriver tag
/// in the embedded compatibility table. Note that this is "latest known to
/// this thirtyfour release" rather than "literally the latest from upstream"
/// — geckodriver releases roughly twice a year, and we do not make a network
/// call to discover newer versions. Users who need a specific newer release
/// should use [`DriverVersion::Exact`].
fn firefox_latest_from_table() -> &'static str {
    GECKODRIVER_RELEASES.first().expect("GECKODRIVER_RELEASES must not be empty").version
}

async fn fetch_edge_latest(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
) -> Result<String, ManagerError> {
    let url = cfg
        .mirror
        .edge_downloads
        .join("LATEST_STABLE")
        .map_err(|e| ManagerError::Parse(e.to_string()))?;
    let bytes = client
        .get(url)
        .timeout(cfg.download_timeout)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    Ok(decode_edge_text(&bytes).trim().to_string())
}

async fn resolve_edge_exact(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
    version: &str,
) -> Result<String, ManagerError> {
    if version.contains('.') {
        return Ok(version.to_string());
    }
    // Major-only request: msedgedriver doesn't expose an index; fetch
    // `LATEST_RELEASE_<major>` from the same host.
    let url = cfg
        .mirror
        .edge_downloads
        .join(&format!("LATEST_RELEASE_{version}"))
        .map_err(|e| ManagerError::Parse(e.to_string()))?;
    let resp = client.get(url).timeout(cfg.download_timeout).send().await?;
    if !resp.status().is_success() {
        // Fall back to latest stable rather than error — best-effort resolution.
        return fetch_edge_latest(client, cfg).await;
    }
    let bytes = resp.bytes().await?;
    Ok(decode_edge_text(&bytes).trim().to_string())
}

/// The msedgedriver `LATEST_*` endpoints return text encoded as UTF-16 LE with
/// a BOM (older endpoint behavior preserved by Microsoft). Decode that, falling
/// back to UTF-8 if no BOM is present.
fn decode_edge_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let utf16: Vec<u16> =
            bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&utf16)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

async fn resolve_firefox_exact(
    _client: &reqwest::Client,
    _cfg: &DownloadConfig,
    version: &str,
) -> Result<String, ManagerError> {
    // `DriverVersion::Exact(...)` for Firefox is a literal geckodriver tag
    // (e.g. "0.36.0"). We don't probe upstream to verify it exists — if the
    // user supplies a wrong tag, the download itself will 404 with a clear
    // error message. Probing would just double the network calls and the
    // failure modes.
    Ok(version.trim_start_matches('v').to_string())
}

/// Where a downloaded driver binary lives in the cache.
pub(crate) struct DriverPath {
    pub binary: PathBuf,
}

/// Ensure the driver for `(browser, version)` is present in the cache; download
/// it if not. Returns the path to the executable.
///
/// For system-managed browsers (Safari) this returns the path to the
/// pre-installed system driver and skips download / cache entirely.
pub(crate) async fn ensure_driver(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
    browser: BrowserKind,
    version: &str,
    emitter: &Emitter,
) -> Result<DriverPath, ManagerError> {
    if browser == BrowserKind::Safari {
        return safari_driver_path();
    }

    let platform = match browser {
        BrowserKind::Chrome | BrowserKind::Firefox => chrome_platform(),
        BrowserKind::Edge => edge_platform(),
        BrowserKind::Safari => unreachable!("safari short-circuited above"),
    };
    let dir = cfg.cache_dir.join(browser.cache_dir_name()).join(version).join(platform);
    let bin_name = exe_name(browser);
    let bin_path = dir.join(&bin_name);

    if bin_path.exists() {
        emitter.emit(Status::DriverCacheHit {
            browser,
            version: version.to_string(),
            path: bin_path.clone(),
        });
        return Ok(DriverPath {
            binary: bin_path,
        });
    }

    if cfg.offline {
        return Err(ManagerError::Offline);
    }

    tokio::fs::create_dir_all(&dir).await?;
    let lock_path = cfg.cache_dir.join(format!("{}-{}.lock", browser.cache_dir_name(), version));
    if let Some(parent) = lock_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Hold an OS-level exclusive lock on the lock file while we
    // download+extract, so concurrent test processes don't race the same cache
    // entry. The lock is released when `lock_file` drops at end of scope.
    //
    // `AsyncFileExt::lock` is sync under the hood (it calls flock /
    // LockFileEx) — it briefly blocks this executor thread while waiting. Hold
    // time is bounded by download+extract; contention only happens between
    // concurrent test processes touching the same cache entry.
    let lock_file = tokio::fs::File::create(&lock_path)
        .await
        .map_err(|e| ManagerError::Lock(format!("create lock: {e}")))?;
    AsyncFileExt::lock(&lock_file).map_err(|e| ManagerError::Lock(format!("acquire: {e}")))?;

    // Re-check after acquiring the lock — another process may have downloaded.
    if bin_path.exists() {
        emitter.emit(Status::DriverCacheHit {
            browser,
            version: version.to_string(),
            path: bin_path.clone(),
        });
        return Ok(DriverPath {
            binary: bin_path,
        });
    }

    download_and_extract(client, cfg, browser, version, &dir, emitter).await?;
    emitter.emit(Status::DriverArchiveExtracted {
        browser,
        path: bin_path.clone(),
    });

    // Make the binary executable on Unix.
    #[cfg(unix)]
    if bin_path.exists() {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&bin_path).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&bin_path, perms).await?;
    }

    Ok(DriverPath {
        binary: bin_path,
    })
}

/// Locate the system-installed `safaridriver`. Currently only macOS ships one.
fn safari_driver_path() -> Result<DriverPath, ManagerError> {
    #[cfg(target_os = "macos")]
    {
        let p = Path::new("/usr/bin/safaridriver");
        if p.exists() {
            return Ok(DriverPath {
                binary: p.to_path_buf(),
            });
        }
        Err(ManagerError::LocalBrowserNotFound {
            browser: "Safari",
            hint: "/usr/bin/safaridriver not found; install Safari and run `safaridriver --enable`",
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ManagerError::UnsupportedBrowser("safari (only available on macOS)".to_string()))
    }
}

fn exe_name(browser: BrowserKind) -> String {
    let stem = browser.driver_binary_stem();
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    }
}

async fn download_and_extract(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
    browser: BrowserKind,
    version: &str,
    target_dir: &Path,
    emitter: &Emitter,
) -> Result<(), ManagerError> {
    match browser {
        BrowserKind::Chrome => {
            download_chromedriver(client, cfg, version, target_dir, emitter).await
        }
        BrowserKind::Firefox => {
            download_geckodriver(client, cfg, version, target_dir, emitter).await
        }
        BrowserKind::Edge => download_msedgedriver(client, cfg, version, target_dir, emitter).await,
        BrowserKind::Safari => Err(ManagerError::Spawn(
            "Safari is system-managed; ensure_driver() should not have reached download path"
                .into(),
        )),
    }
}

async fn download_msedgedriver(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
    version: &str,
    target_dir: &Path,
    emitter: &Emitter,
) -> Result<(), ManagerError> {
    let platform = edge_platform();
    let url = cfg
        .mirror
        .edge_downloads
        .join(&format!("{version}/edgedriver_{platform}.zip"))
        .map_err(|e| ManagerError::Parse(e.to_string()))?;
    let bytes = fetch_bytes_with_retry(
        client,
        &url,
        cfg.download_timeout,
        BrowserKind::Edge,
        version,
        emitter,
    )
    .await?;
    extract_zip(&bytes, target_dir, BrowserKind::Edge)
}

async fn download_chromedriver(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
    version: &str,
    target_dir: &Path,
    emitter: &Emitter,
) -> Result<(), ManagerError> {
    let index = fetch_chrome_index(client, cfg).await?;
    let entry = index.versions.iter().find(|v| v.version == version).ok_or_else(|| {
        ManagerError::Parse(format!("chromedriver version {version} not in CfT index"))
    })?;
    let platform = chrome_platform();
    let download =
        entry.downloads.chromedriver.iter().find(|d| d.platform == platform).ok_or_else(|| {
            ManagerError::Parse(format!(
                "chromedriver {version} has no download for platform {platform}"
            ))
        })?;
    let url = download
        .url
        .parse::<Url>()
        .map_err(|e| ManagerError::Parse(format!("invalid chromedriver download URL: {e}")))?;
    let bytes = fetch_bytes_with_retry(
        client,
        &url,
        cfg.download_timeout,
        BrowserKind::Chrome,
        version,
        emitter,
    )
    .await?;
    extract_zip(&bytes, target_dir, BrowserKind::Chrome)
}

/// GET a URL into bytes, retrying on transient failures (5xx and network
/// errors). 4xx errors surface immediately — they won't get better with
/// retrying, and retrying could mask a real misconfiguration. Backoff is
/// fixed at 1s, 2s, 4s.
async fn fetch_bytes_with_retry(
    client: &reqwest::Client,
    url: &Url,
    timeout: Duration,
    browser: BrowserKind,
    version: &str,
    emitter: &Emitter,
) -> Result<bytes::Bytes, ManagerError> {
    const MAX_ATTEMPTS: u32 = 3;
    let mut last_err: Option<ManagerError> = None;
    let started = Instant::now();
    emitter.emit(Status::DriverDownloadStarted {
        browser,
        version: version.to_string(),
        url: url.to_string(),
    });
    for attempt in 0..MAX_ATTEMPTS {
        let result: Result<bytes::Bytes, ManagerError> = async {
            let resp = client
                .get(url.clone())
                .header("User-Agent", "thirtyfour-manager")
                .timeout(timeout)
                .send()
                .await?;
            let status = resp.status();
            if status.is_client_error() {
                // 4xx — won't get better with retry; surface immediately.
                return Err(ManagerError::Http(format!("HTTP {status} for {url}")));
            }
            if !status.is_success() {
                // 5xx — retryable.
                return Err(ManagerError::Http(format!("HTTP {status} for {url}")));
            }
            Ok(resp.bytes().await?)
        }
        .await;

        match result {
            Ok(b) => {
                emitter.emit(Status::DriverDownloadComplete {
                    browser,
                    version: version.to_string(),
                    bytes: b.len() as u64,
                    duration: started.elapsed(),
                });
                return Ok(b);
            }
            Err(e @ ManagerError::Http(_)) if is_transient(&e) => {
                emitter.emit(Status::DriverDownloadRetry {
                    attempt: attempt + 1,
                    error: e.to_string(),
                });
                last_err = Some(e);
            }
            Err(e) => return Err(e),
        }

        if attempt + 1 < MAX_ATTEMPTS {
            let backoff = Duration::from_secs(1u64 << attempt); // 1s, 2s, 4s
            tokio::time::sleep(backoff).await;
        }
    }
    Err(last_err.unwrap_or_else(|| ManagerError::Http("retry budget exhausted".into())))
}

/// `true` if the error message indicates a transient failure (5xx, network
/// error, timeout). Used to decide whether to retry.
fn is_transient(err: &ManagerError) -> bool {
    let ManagerError::Http(msg) = err else {
        return false;
    };
    // 5xx codes cover server / gateway / unavailable.
    msg.contains("HTTP 5")
        // reqwest message strings for connect/timeout/connection-closed.
        || msg.contains("operation timed out")
        || msg.contains("connection")
        || msg.contains("dns error")
        || msg.contains("error sending request")
}

/// Extract the driver binary for `browser` from a ZIP archive, writing it to
/// `target_dir`.
pub(crate) fn extract_zip(
    bytes: &[u8],
    target_dir: &Path,
    browser: BrowserKind,
) -> Result<(), ManagerError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor).map_err(|e| ManagerError::Extract(e.to_string()))?;
    let exe = exe_name(browser);
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| ManagerError::Extract(e.to_string()))?;
        let entry_name = entry.name().to_string();
        let basename = Path::new(&entry_name).file_name().and_then(|s| s.to_str()).unwrap_or("");
        if basename == exe {
            let mut out = std::fs::File::create(target_dir.join(&exe)).map_err(ManagerError::Io)?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| ManagerError::Extract(e.to_string()))?;
            return Ok(());
        }
    }
    Err(ManagerError::Extract(format!(
        "{exe} not found inside {} archive",
        browser.cache_dir_name()
    )))
}

async fn download_geckodriver(
    client: &reqwest::Client,
    cfg: &DownloadConfig,
    version: &str,
    target_dir: &Path,
    emitter: &Emitter,
) -> Result<(), ManagerError> {
    let url = geckodriver_download_url(cfg, version)?;
    let bytes = fetch_bytes_with_retry(
        client,
        &url,
        cfg.download_timeout,
        BrowserKind::Firefox,
        version,
        emitter,
    )
    .await?;
    if cfg!(windows) {
        extract_zip(&bytes, target_dir, BrowserKind::Firefox)
    } else {
        extract_geckodriver_tar_gz(&bytes, target_dir)
    }
}

fn geckodriver_download_url(cfg: &DownloadConfig, version: &str) -> Result<Url, ManagerError> {
    let v = version.trim_start_matches('v');
    let asset = geckodriver_asset_name(v);
    cfg.mirror
        .geckodriver_downloads
        .join(&format!("v{v}/{asset}"))
        .map_err(|e| ManagerError::Parse(e.to_string()))
}

fn geckodriver_asset_name(version: &str) -> String {
    let suffix = if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            "win-aarch64.zip"
        } else if cfg!(target_pointer_width = "64") {
            "win64.zip"
        } else {
            "win32.zip"
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "macos-aarch64.tar.gz"
        } else {
            "macos.tar.gz"
        }
    } else if cfg!(target_arch = "aarch64") {
        "linux-aarch64.tar.gz"
    } else {
        "linux64.tar.gz"
    };
    format!("geckodriver-v{version}-{suffix}")
}

fn extract_geckodriver_tar_gz(bytes: &[u8], target_dir: &Path) -> Result<(), ManagerError> {
    let gz = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
    let mut archive = tar::Archive::new(gz);
    let exe = exe_name(BrowserKind::Firefox);
    for entry in archive.entries().map_err(|e| ManagerError::Extract(e.to_string()))? {
        let mut entry = entry.map_err(|e| ManagerError::Extract(e.to_string()))?;
        let path = entry.path().map_err(|e| ManagerError::Extract(e.to_string()))?;
        let basename = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if basename == exe {
            let out_path = target_dir.join(&exe);
            entry.unpack(&out_path).map_err(|e| ManagerError::Extract(e.to_string()))?;
            return Ok(());
        }
    }
    Err(ManagerError::Extract(format!("{exe} not found inside geckodriver archive")))
}

/// CfT platform string. Falls back to Linux 64-bit on unknown targets — best
/// effort.
fn chrome_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "mac-arm64"
        } else {
            "mac-x64"
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_pointer_width = "64") {
            "win64"
        } else {
            "win32"
        }
    } else {
        if cfg!(target_arch = "aarch64") {
            "linux-arm64"
        } else {
            "linux64"
        }
    }
}

/// msedgedriver platform string. The naming differs from Chrome's CfT scheme.
fn edge_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "mac64_m1"
        } else {
            "mac64"
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            "arm64"
        } else if cfg!(target_pointer_width = "64") {
            "win64"
        } else {
            "win32"
        }
    } else {
        "linux64"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn semver_sort() {
        assert_eq!(sort_semver("126.0.6478.10", "126.0.6478.126"), std::cmp::Ordering::Less);
        assert_eq!(sort_semver("127.0.0.0", "126.99.99.99"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn chrome_platform_known() {
        let p = chrome_platform();
        assert!(["mac-arm64", "mac-x64", "win64", "win32", "linux64"].contains(&p));
    }

    #[test]
    fn edge_platform_known() {
        let p = edge_platform();
        assert!(["mac64_m1", "mac64", "win64", "win32", "arm64", "linux64"].contains(&p));
    }

    #[test]
    fn decode_edge_text_utf16_bom() {
        // bytes: BOM + "126" in UTF-16 LE
        let bytes = [0xFF, 0xFE, 0x31, 0x00, 0x32, 0x00, 0x36, 0x00];
        assert_eq!(decode_edge_text(&bytes).trim(), "126");
    }

    #[test]
    fn decode_edge_text_utf8_fallback() {
        let bytes = b"126.0.6478.126\n";
        assert_eq!(decode_edge_text(bytes).trim(), "126.0.6478.126");
    }

    #[test]
    fn extract_zip_finds_chromedriver() {
        // Build an in-memory ZIP containing
        // `chromedriver-<platform>/chromedriver[.exe]` to mirror real CfT layout.
        let exe = exe_name(BrowserKind::Chrome);
        let inner_path = format!("chromedriver-linux64/{exe}");

        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zip.start_file::<_, ()>(&inner_path, Default::default()).unwrap();
            zip.write_all(b"#!/bin/sh\necho fake driver\n").unwrap();
            zip.finish().unwrap();
        }

        let dir = tempfile::tempdir().unwrap();
        extract_zip(&buf, dir.path(), BrowserKind::Chrome).unwrap();

        let extracted = dir.path().join(&exe);
        assert!(extracted.exists(), "extracted binary should exist at {extracted:?}");
        let content = std::fs::read(&extracted).unwrap();
        assert!(content.starts_with(b"#!/bin/sh"));
    }

    #[test]
    fn extract_zip_finds_geckodriver() {
        // Geckodriver Windows zip layout: binary at archive root.
        let exe = exe_name(BrowserKind::Firefox);
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zip.start_file::<_, ()>(&exe, Default::default()).unwrap();
            zip.write_all(b"fake geckodriver\n").unwrap();
            zip.finish().unwrap();
        }

        let dir = tempfile::tempdir().unwrap();
        // This is the regression test for the "extract_geckodriver_zip looked
        // for chromedriver" bug — exercising the unified extract_zip with the
        // Firefox kind must succeed.
        extract_zip(&buf, dir.path(), BrowserKind::Firefox).unwrap();
        assert!(dir.path().join(&exe).exists());
    }

    #[test]
    fn geckodriver_for_firefox_table() {
        // Latest geckodriver supports Firefox >=128.
        assert_eq!(geckodriver_for_firefox("150.0"), Some("0.36.0"));
        assert_eq!(geckodriver_for_firefox("128.0.1"), Some("0.36.0"));
        // Firefox 115-127: 0.36.0 needs >=128, so we drop to 0.35.0 (>=115, no max).
        assert_eq!(geckodriver_for_firefox("127.0"), Some("0.35.0"));
        assert_eq!(geckodriver_for_firefox("115.0"), Some("0.35.0"));
        // Firefox 102-114: 0.35/0.34 need >=115, so drop to 0.33.0 (102-120).
        assert_eq!(geckodriver_for_firefox("114.0"), Some("0.33.0"));
        assert_eq!(geckodriver_for_firefox("102.5"), Some("0.33.0"));
        // Firefox 91-101: 0.31.0 (91-120).
        assert_eq!(geckodriver_for_firefox("101.0"), Some("0.31.0"));
        assert_eq!(geckodriver_for_firefox("91.0"), Some("0.31.0"));
        // Firefox 78-90: 0.30.0.
        assert_eq!(geckodriver_for_firefox("80.0"), Some("0.30.0"));
        assert_eq!(geckodriver_for_firefox("78.0"), Some("0.30.0"));
        // Firefox 60-77: 0.29.1.
        assert_eq!(geckodriver_for_firefox("70.0"), Some("0.29.1"));
        assert_eq!(geckodriver_for_firefox("60.0"), Some("0.29.1"));
        // Firefox older than anything in the table (< 60): no entry covers it.
        assert_eq!(geckodriver_for_firefox("50.0"), None);
        // Garbage parses as 0 → no entry covers it.
        assert_eq!(geckodriver_for_firefox("nonsense"), None);
    }

    #[test]
    fn firefox_latest_is_table_head() {
        // Sanity: `Latest` for Firefox returns the highest entry, which is the
        // first element of the table.
        assert_eq!(firefox_latest_from_table(), "0.36.0");
    }

    #[test]
    fn transient_error_classification() {
        // 5xx → retry.
        assert!(is_transient(&ManagerError::Http("HTTP 502 Bad Gateway for ...".into())));
        assert!(is_transient(&ManagerError::Http("HTTP 503 Service Unavailable".into())));
        assert!(is_transient(&ManagerError::Http("HTTP 500 Internal Server Error".into())));
        // Network errors → retry.
        assert!(is_transient(&ManagerError::Http("error sending request".into())));
        assert!(is_transient(&ManagerError::Http("operation timed out".into())));
        // 4xx → don't retry.
        assert!(!is_transient(&ManagerError::Http("HTTP 404 Not Found".into())));
        assert!(!is_transient(&ManagerError::Http("HTTP 403 Forbidden".into())));
        // Non-Http error → don't retry.
        assert!(!is_transient(&ManagerError::Parse("bad version".into())));
    }

    #[test]
    fn extract_zip_missing_binary_errors() {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            zip.start_file::<_, ()>("README.txt", Default::default()).unwrap();
            zip.write_all(b"no driver here").unwrap();
            zip.finish().unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let err = extract_zip(&buf, dir.path(), BrowserKind::Chrome).unwrap_err();
        assert!(matches!(err, ManagerError::Extract(_)));
    }
}
