//! Opt-in, blank-page-only comparison. No profile files are deleted by this test.
#![cfg(feature = "manager-tests")]
#[path = "support/cleanup_diagnostics.rs"]
mod diagnostics;
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use thirtyfour::{
    manager::{BrowserKind, Status, WebDriverManager},
    prelude::*,
};

fn caps() -> Capabilities {
    let mut caps = DesiredCapabilities::chrome();
    caps.set_headless().unwrap();
    caps.set_no_sandbox().unwrap();
    // Explicit in BOTH arms, including macOS/Linux and manual driver overrides.
    caps.add_experimental_option("quitGracefully", true).unwrap();
    caps.into()
}

async fn binary() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("THIRTYFOUR_CHROMEDRIVER") {
        return Ok(path.into());
    }
    let binary = Arc::new(Mutex::new(None));
    let captured = binary.clone();
    let manager = WebDriverManager::builder()
        .on_status(move |status| {
            if let Status::DriverProcessSpawned {
                binary,
                ..
            } = status
            {
                *captured.lock().unwrap() = Some(binary.clone());
            }
        })
        .build();
    let driver = manager.launch(caps()).await?;
    let profile = PathBuf::from(
        driver.capabilities().get("chrome").context("chrome capabilities")?["userDataDir"]
            .as_str()
            .context("bootstrap profile")?,
    );
    diagnostics::save(
        "bootstrap",
        &json!({"profile": profile, "capabilities": driver.capabilities()}),
    );
    driver.quit().await?;
    if profile.exists() {
        diagnostics::snapshot("bootstrap-residue", &profile).await;
    }
    let path = binary.lock().unwrap().clone().context("resolved ChromeDriver path")?;
    Ok(path)
}

async fn session(
    arm: &'static str,
    round: usize,
    slot: usize,
    binary: PathBuf,
    root: PathBuf,
) -> Result<PathBuf> {
    let name = format!("{arm}-{round:02}-{slot}");
    let start = Instant::now();
    let mut record = json!({"arm": arm, "round": round, "slot": slot, "started_ms": diagnostics::now_ms(), "requested": caps(), "binary": binary});
    let log = root.join(format!("{name}-driver.log"));
    diagnostics::save(&name, &record);
    let profile;
    if arm == "managed" {
        let event_name = format!("{name}-process");
        let manager = WebDriverManager::builder()
            .driver_binary(BrowserKind::Chrome, &binary)
            .driver_arg(BrowserKind::Chrome, "--verbose")
            .driver_arg(BrowserKind::Chrome, format!("--log-path={}", log.display()))
            .on_status(move |status| {
                if matches!(
                    status,
                    Status::DriverProcessSpawned { .. } | Status::DriverShutdown { .. }
                ) {
                    // Distinct names preserve both spawn and shutdown evidence.
                    let phase = if matches!(status, Status::DriverProcessSpawned { .. }) {
                        "spawn"
                    } else {
                        "exit"
                    };
                    diagnostics::save(
                        &format!("{event_name}-{phase}"),
                        &json!({"at_ms": diagnostics::now_ms(), "status": format!("{status:?}")}),
                    );
                }
            })
            .build();
        let driver = manager.launch(caps()).await?;
        profile = PathBuf::from(
            driver.capabilities().get("chrome").context("chrome capabilities")?["userDataDir"]
                .as_str()
                .context("profile")?,
        );
        record["profile"] = json!(profile);
        record["returned"] = json!(driver.capabilities());
        record["created_ms"] = json!(start.elapsed().as_millis());
        diagnostics::save(&name, &record); // Save before quit, including when quit fails.
        let quit = driver.quit().await;
        record["quit_ms"] = json!(start.elapsed().as_millis());
        record["quit_result"] = json!(format!("{quit:?}"));
        diagnostics::save(&name, &record);
        quit?;
    } else {
        let mut command = tokio::process::Command::new(&binary);
        command
            .args(["--port=0".into(), "--verbose".into(), format!("--log-path={}", log.display())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x0800_0000);
        let mut child = command.spawn()?;
        record["driver_pid"] = json!(child.id());
        let client =
            reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(60)).build()?;
        // Let ChromeDriver choose its listening port; reserving then releasing a
        // port races other concurrent children. Read the actual startup report.
        let port = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Some(status) = child.try_wait()? {
                    bail!("raw driver exited before readiness: {status}");
                }
                let text = std::fs::read_to_string(&log).unwrap_or_default();
                if let Some(port) = text.lines().find_map(|line| {
                    line.split_once("ChromeDriver was started successfully on port ")
                        .and_then(|(_, port)| port.trim_end_matches('.').parse::<u16>().ok())
                }) {
                    break Ok::<_, anyhow::Error>(port);
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .context("raw driver readiness timeout")??;
        let url = format!("http://127.0.0.1:{port}");
        client.get(format!("{url}/status")).send().await?.error_for_status()?;
        let response = client.post(format!("{url}/session"))
            .json(&json!({"capabilities": {"alwaysMatch": caps(), "firstMatch": [{}]}, "desiredCapabilities": caps()}))
            .send().await?;
        let status = response.status();
        let body: Value = response.json().await?;
        ensure!(status.is_success(), "raw create failed: {body}");
        let id = body["value"]["sessionId"].as_str().context("session id")?;
        profile = PathBuf::from(
            body["value"]["capabilities"]["chrome"]["userDataDir"]
                .as_str()
                .context("raw profile")?,
        );
        record["profile"] = json!(profile);
        record["returned"] = body["value"]["capabilities"].clone();
        record["created_ms"] = json!(start.elapsed().as_millis());
        diagnostics::save(&name, &record);
        let response = client.delete(format!("{url}/session/{id}")).send().await?;
        record["delete_status"] = json!(response.status().as_u16());
        record["delete_body"] = response.json().await?;
        record["delete_ms"] = json!(start.elapsed().as_millis());
        diagnostics::save(&name, &record);
        ensure!(record["delete_status"] == 200, "raw DELETE failed: {record}");
        let response = client.get(format!("{url}/session/{id}/url")).send().await?;
        record["barrier_status"] = json!(response.status().as_u16());
        record["barrier_body"] = response.json().await?;
        record["barrier_ms"] = json!(start.elapsed().as_millis());
        diagnostics::save(&name, &record);
        ensure!(
            record["barrier_status"] == 404
                && record["barrier_body"]["value"]["error"] == "invalid session id",
            "raw barrier not confirmed: {record}"
        );
        // Observe while ChromeDriver is still alive: residue here predates /shutdown.
        if profile.exists() {
            diagnostics::snapshot(&format!("{name}-before-shutdown"), &profile).await;
        }
        let shutdown = tokio::time::timeout(Duration::from_secs(10), async {
            let response = client.get(format!("{url}/shutdown")).send().await;
            record["shutdown_response"] = json!(format!("{:?}", response.map(|r| r.status())));
            child.wait().await
        })
        .await;
        record["exit"] = json!(format!("{shutdown:?}"));
        record["quit_ms"] = json!(start.elapsed().as_millis());
        diagnostics::save(&name, &record);
        if shutdown.is_err() {
            child.kill().await?;
            bail!("raw driver shutdown timed out: {name}");
        }
        shutdown??;
    }
    if profile.exists() {
        diagnostics::snapshot(&format!("{name}-after-quit"), &profile).await;
    }
    Ok(profile)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "320 real Chrome sessions plus bootstrap; diagnostic comparison"]
async fn matched_cleanup_comparison() -> Result<()> {
    let root = PathBuf::from(
        std::env::var_os("THIRTYFOUR_CLEANUP_ARTIFACTS")
            .context("set THIRTYFOUR_CLEANUP_ARTIFACTS to a dedicated absolute directory")?,
    );
    ensure!(root.is_absolute(), "artifact directory must be absolute");
    std::fs::create_dir_all(&root)?;
    let binary = binary().await?;
    let version = tokio::process::Command::new(&binary).arg("--version").output().await?;
    diagnostics::save(
        "environment",
        &json!({"os": std::env::consts::OS, "binary": binary, "driver_version": String::from_utf8_lossy(&version.stdout), "requested": caps()}),
    );
    let rounds: usize =
        std::env::var("THIRTYFOUR_CLEANUP_ROUNDS").unwrap_or_else(|_| "20".into()).parse()?;
    let mut profiles = Vec::new();
    let mut errors = Vec::new();
    for round in 1..=rounds {
        // Alternate order to reduce warm-cache / elapsed-time bias.
        for arm in if round % 2 == 1 {
            ["raw", "managed"]
        } else {
            ["managed", "raw"]
        } {
            let mut tasks = tokio::task::JoinSet::new();
            for slot in 0..8 {
                let binary = binary.clone();
                let root = root.clone();
                tasks.spawn(async move {
                    (
                        format!("{arm}-{round:02}-{slot}"),
                        session(arm, round, slot, binary, root).await,
                    )
                });
            }
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok((_, Ok(path))) => profiles.push((arm, path)),
                    Ok((name, Err(error))) => errors.push(format!("{name}: {error:#}")),
                    Err(error) => errors.push(format!("task: {error}")),
                }
            }
            diagnostics::save(
                "progress",
                &json!({"round": round, "arm": arm, "profiles": profiles, "errors": errors}),
            );
            eprintln!(
                "round {round} {arm}: {} / {} recorded profiles remain; {} errors",
                profiles.iter().filter(|(_, p)| p.exists()).count(),
                profiles.len(),
                errors.len()
            );
        }
    }
    tokio::time::sleep(Duration::from_secs(2)).await;
    let remaining: Vec<_> = profiles.iter().filter(|(_, p)| p.exists()).collect();
    diagnostics::save(
        "summary",
        &json!({"profiles": profiles, "remaining": remaining, "errors": errors}),
    );
    for (index, (_, profile)) in remaining.iter().enumerate() {
        diagnostics::snapshot(&format!("final-{index}"), profile).await;
    }
    if !remaining.is_empty() {
        tokio::time::sleep(Duration::from_secs(30)).await;
        for (index, (_, profile)) in remaining.iter().enumerate() {
            diagnostics::snapshot(&format!("delayed-{index}"), profile).await;
        }
    }
    ensure!(errors.is_empty(), "session errors: {errors:?}");
    ensure!(
        remaining.is_empty(),
        "{} / {} profiles remain; see artifacts",
        remaining.len(),
        profiles.len()
    );
    Ok(())
}
