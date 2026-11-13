//! Real-browser regression coverage for managed Chromium cleanup (issue #380).
#![cfg(feature = "manager-tests")]
use std::{path::PathBuf, time::Duration};
use thirtyfour::prelude::*;

async fn session() -> WebDriverResult<(WebDriver, PathBuf)> {
    session_with_graceful_browser(false).await
}

async fn session_with_graceful_browser(graceful: bool) -> WebDriverResult<(WebDriver, PathBuf)> {
    let mut caps = DesiredCapabilities::chrome();
    caps.set_headless()?;
    caps.set_no_sandbox()?;
    if graceful {
        caps.add_experimental_option("quitGracefully", true)?;
    }
    let builder = WebDriver::managed(caps);
    let driver = if std::env::var_os("THIRTYFOUR_CLEANUP_DIAGNOSTICS").is_some() {
        builder
            .driver_arg(thirtyfour::manager::BrowserKind::Chrome, "--verbose")
            .on_driver_log(|line| eprintln!("driver: {}", line.line))
            .await?
    } else {
        builder.await?
    };
    let profile = PathBuf::from(
        driver.capabilities().get("chrome").unwrap()["userDataDir"]
            .as_str()
            .expect("Chrome profile"),
    );
    Ok((driver, profile))
}

#[tokio::test]
async fn managed_quit_removes_chrome_profile() -> WebDriverResult<()> {
    let (driver, profile) = session().await?;
    assert!(profile.exists());
    driver.quit().await?;
    if profile.exists() {
        fn inventory(path: &std::path::Path, depth: usize) {
            if depth > 3 {
                return;
            }
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten().take(80) {
                    eprintln!(
                        "leftover: {} {:?}",
                        entry.path().display(),
                        entry.metadata().map(|m| m.len())
                    );
                    if entry.file_type().is_ok_and(|t| t.is_dir()) {
                        inventory(&entry.path(), depth + 1);
                    }
                }
            }
        }
        inventory(&profile, 0);
        tokio::time::sleep(Duration::from_secs(5)).await;
        panic!(
            "profile remained immediately after quit: {}; exists five seconds later: {}",
            profile.display(),
            profile.exists()
        );
    }
    Ok(())
}

/// Run in a dedicated process with MAC_CHROMIUM_TMPDIR (macOS) or TMP/TEMP
/// (Windows) redirected to an empty directory to inspect extension folders too.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "160 real-browser sessions; opt-in cleanup stress test"]
async fn managed_cleanup_stress() -> WebDriverResult<()> {
    let mut profiles = Vec::new();
    for round in 0..20 {
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..8 {
            tasks.spawn(async {
                let (driver, path) = session().await?;
                driver.quit().await?;
                Ok::<_, WebDriverError>(path)
            });
        }
        while let Some(result) = tasks.join_next().await {
            profiles.push(result.unwrap()?);
        }
        eprintln!(
            "round {}: {} / {} profiles remain",
            round + 1,
            profiles.iter().filter(|p| p.exists()).count(),
            profiles.len()
        );
    }
    tokio::time::sleep(Duration::from_secs(2)).await;
    let remaining: Vec<_> = profiles.iter().filter(|p| p.exists()).collect();
    assert!(
        remaining.is_empty(),
        "{} / {} profiles remain: {:?}",
        remaining.len(),
        profiles.len(),
        remaining
    );
    Ok(())
}

/// Gecko already cleans its profile during DELETE; retain its shutdown path.
#[tokio::test]
#[ignore = "requires a local Firefox installation"]
async fn managed_quit_removes_firefox_profile() -> WebDriverResult<()> {
    let mut caps = DesiredCapabilities::firefox();
    caps.set_headless()?;
    let driver = WebDriver::managed(caps).await?;
    let profile =
        PathBuf::from(driver.capabilities().get("moz:profile").unwrap().as_str().unwrap());
    assert!(profile.exists());
    driver.quit().await?;
    assert!(!profile.exists(), "Firefox profile remains: {}", profile.display());
    Ok(())
}

#[tokio::test]
async fn managed_graceful_browser_quit_removes_profile() -> WebDriverResult<()> {
    let (driver, profile) = session_with_graceful_browser(true).await?;
    assert!(profile.exists());
    driver.quit().await?;
    assert!(!profile.exists(), "profile remains with quitGracefully: {}", profile.display());
    Ok(())
}
