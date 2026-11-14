//! Observation only: never remove, rename, or open files for writing in a profile.
use serde_json::{Value, json};
use std::{io::Read, path::Path, time::SystemTime};

pub fn now_ms() -> u128 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis()
}

pub fn save(name: &str, value: &Value) {
    if let Some(root) = std::env::var_os("THIRTYFOUR_CLEANUP_ARTIFACTS") {
        std::fs::create_dir_all(&root).expect("diagnostic directory");
        std::fs::write(
            Path::new(&root).join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .expect("write diagnostic artifact");
    }
}

pub fn inventory(profile: &Path) -> Value {
    fn walk(root: &Path, path: &Path, depth: usize, rows: &mut Vec<Value>, truncated: &mut bool) {
        if depth > 6 || rows.len() >= 256 {
            *truncated = true;
            return;
        }
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) => {
                rows.push(json!({"path": path.strip_prefix(root).unwrap_or(path), "error": error.to_string()}));
                return;
            }
        };
        for entry in entries {
            if rows.len() >= 256 {
                *truncated = true;
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    rows.push(json!({"error": error.to_string()}));
                    continue;
                }
            };
            let path = entry.path();
            // symlink_metadata and no recursion through symlinks/reparse points.
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(error) => {
                    rows.push(json!({"path": path.strip_prefix(root).unwrap(), "error": error.to_string()}));
                    continue;
                }
            };
            let mut row = json!({"path": path.strip_prefix(root).unwrap(), "bytes": metadata.len(), "directory": metadata.is_dir(), "symlink": metadata.is_symlink()});
            #[cfg(windows)]
            let reparse = {
                use std::os::windows::fs::MetadataExt;
                metadata.file_attributes() & 0x400 != 0
            };
            #[cfg(not(windows))]
            let reparse = metadata.is_symlink();
            if metadata.is_file() && !reparse && entry.file_name() == "chrome_debug.log" {
                let mut bytes = Vec::new();
                match std::fs::File::open(&path).and_then(|f| f.take(16384).read_to_end(&mut bytes))
                {
                    Ok(_) => {
                        row["log_prefix"] = json!(String::from_utf8_lossy(&bytes));
                        row["log_truncated"] = json!(metadata.len() > 16384);
                    }
                    Err(error) => row["log_error"] = json!(error.to_string()),
                }
            }
            rows.push(row);
            if metadata.is_dir() && !reparse {
                walk(root, &path, depth + 1, rows, truncated);
            }
        }
    }
    let mut rows = Vec::new();
    let mut truncated = false;
    let exists = profile.try_exists();
    if matches!(exists, Ok(true)) {
        walk(profile, profile, 0, &mut rows, &mut truncated);
    }
    json!({"at_ms": now_ms(), "profile": profile, "exists": format!("{exists:?}"), "entries": rows, "truncated": truncated})
}

pub async fn snapshot(name: &str, profile: &Path) {
    let report = inventory(profile);
    eprintln!("cleanup snapshot {name}: {report}");
    save(name, &report);
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("THIRTYFOUR_CLEANUP_ARTIFACTS") {
        let mut command = tokio::process::Command::new("pwsh");
        command
            .args(["-NoProfile", "-File", "../ci/cleanup-handles.ps1", "-Profile"])
            .arg(profile)
            .arg("-OutputPath")
            .arg(Path::new(&root).join(format!("{name}-handles.txt")))
            .kill_on_drop(true);
        match tokio::time::timeout(std::time::Duration::from_secs(20), command.status()).await {
            Ok(Ok(status)) => eprintln!("handle collector: {status}"),
            other => eprintln!("handle collector unavailable: {other:?}"),
        }
    }
}
