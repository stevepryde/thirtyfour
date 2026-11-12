//! Real-browser regression coverage for managed Chromium cleanup (issue #380).
#![cfg(feature = "manager-tests")]
use std::{path::PathBuf, time::Duration};
use thirtyfour::prelude::*;

async fn session() -> WebDriverResult<(WebDriver, PathBuf)> {
    let mut caps = DesiredCapabilities::chrome();
    caps.set_headless()?;
    caps.set_no_sandbox()?;
    let driver = WebDriver::managed(caps).await?;
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
    assert!(!profile.exists(), "profile remains after quit: {}", profile.display());
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
