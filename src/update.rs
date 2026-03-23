// Version check and self-update via GitHub Releases.

use std::path::PathBuf;

const REPO: &str = "gigagookbob/rwd";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Fetches the latest release tag from GitHub API.
pub async fn check_latest_version() -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "rwd")
        .send()
        .await?
        .json()
        .await?;

    let tag = resp
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or(crate::messages::error::RELEASE_TAG_NOT_FOUND)?;

    // "v0.1.0" → "0.1.0"
    Ok(tag.trim_start_matches('v').to_string())
}

/// Prints update notice if a newer version exists.
/// Cache strategy: 24h TTL when update available, always recheck when current.
pub async fn notify_if_update_available() {
    if let Some(cached) = crate::cache::load_update_check() {
        let now = chrono::Utc::now();
        let interval = chrono::Duration::hours(24);
        if cached.latest_version != CURRENT_VERSION
            && now - cached.checked_at < interval
        {
            print_update_notice(&cached.latest_version);
            return;
        }
    }

    // Cache miss or expired — call GitHub API
    if let Ok(latest) = check_latest_version().await {
        // Save to cache (silently ignore failures)
        let cache = crate::cache::UpdateCheckCache {
            checked_at: chrono::Utc::now(),
            latest_version: latest.clone(),
        };
        let _ = crate::cache::save_update_check(&cache);

        print_update_notice(&latest);
    }
}

/// Prints update notice if latest differs from current version.
fn print_update_notice(latest_version: &str) {
    if latest_version != CURRENT_VERSION {
        eprintln!(
            "{}",
            crate::messages::update::new_version_available(latest_version, CURRENT_VERSION)
        );
        eprintln!("{}\n", crate::messages::update::UPDATE_HINT);
    }
}

/// Performs self-update: fetch latest binary from GitHub and replace current executable.
pub async fn run_update() -> Result<(), Box<dyn std::error::Error>> {
    // Clean up leftover from previous Windows update
    cleanup_old_binary();

    let latest = check_latest_version().await?;

    if latest == CURRENT_VERSION {
        eprintln!("{}", crate::messages::update::already_latest(CURRENT_VERSION));
        return Ok(());
    }

    eprintln!("{}", crate::messages::update::updating(CURRENT_VERSION, &latest));

    // Determine platform-specific asset name
    let asset_name = detect_asset_name()?;
    let download_url = format!(
        "https://github.com/{REPO}/releases/download/v{latest}/{asset_name}"
    );

    // Download binary
    eprintln!("{}", crate::messages::update::downloading(&download_url));
    let client = reqwest::Client::new();
    let resp = client
        .get(&download_url)
        .header("User-Agent", "rwd")
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(crate::messages::error::download_failed(resp.status().as_u16()).into());
    }
    let bytes = resp.bytes().await?;

    // Save to temp and extract
    let tmp_dir = std::env::temp_dir().join("rwd_update");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir)?;

    let archive_path = tmp_dir.join(&asset_name);
    std::fs::write(&archive_path, &bytes)?;

    // Extract archive
    extract_archive(&archive_path, &tmp_dir)?;

    // Find extracted binary
    let extracted = find_binary_in_dir(&tmp_dir)?;

    // Replace current executable
    let current_exe = std::env::current_exe()?;
    replace_binary(&extracted, &current_exe)?;

    // Cleanup
    std::fs::remove_dir_all(&tmp_dir).ok();

    // Update cache so next run won't show update notice
    let cache = crate::cache::UpdateCheckCache {
        checked_at: chrono::Utc::now(),
        latest_version: latest.clone(),
    };
    let _ = crate::cache::save_update_check(&cache);

    eprintln!("{}", crate::messages::update::update_complete(&latest));
    Ok(())
}

/// Returns platform-specific asset filename.
fn detect_asset_name() -> Result<String, Box<dyn std::error::Error>> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    let name = match (os, arch) {
        ("macos", "aarch64") => "rwd-aarch64-apple-darwin.tar.gz",
        ("macos", "x86_64") => "rwd-x86_64-apple-darwin.tar.gz",
        ("linux", "x86_64") => "rwd-x86_64-unknown-linux-gnu.tar.gz",
        ("windows", "x86_64") => "rwd-x86_64-pc-windows-msvc.zip",
        _ => return Err(crate::messages::error::unsupported_platform(os, arch).into()),
    };
    Ok(name.to_string())
}

/// Extracts the downloaded archive (tar.gz on Unix, zip on Windows).
fn extract_archive(
    archive: &std::path::Path,
    dest: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive_str = archive.to_string_lossy();

    if archive_str.ends_with(".zip") {
        let status = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    archive_str,
                    dest.to_string_lossy()
                ),
            ])
            .status()?;
        if !status.success() {
            return Err(crate::messages::error::EXTRACT_FAILED.into());
        }
    } else {
        let status = std::process::Command::new("tar")
            .args(["-xzf", &archive_str, "-C", &dest.to_string_lossy()])
            .status()?;
        if !status.success() {
            return Err(crate::messages::error::EXTRACT_FAILED.into());
        }
    }
    Ok(())
}

/// Finds the rwd binary in a directory.
fn find_binary_in_dir(dir: &std::path::Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let binary_name = if cfg!(windows) { "rwd.exe" } else { "rwd" };
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == binary_name && path.is_file() {
            return Ok(path);
        }
    }
    Err(crate::messages::error::BINARY_NOT_FOUND.into())
}

/// Cleans up leftover `.old` binary from a previous Windows update.
fn cleanup_old_binary() {
    let Ok(current_exe) = std::env::current_exe() else {
        return;
    };
    let old_path = current_exe.with_extension("exe.old");
    if old_path.exists() {
        std::fs::remove_file(&old_path).ok();
    }
}

/// Replaces the current binary with a new one.
// `return` inside #[cfg(windows)] block is required — clippy can't see across cfg boundaries.
#[allow(clippy::needless_return)]
fn replace_binary(
    new_binary: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Set executable permission
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(new_binary, std::fs::Permissions::from_mode(0o755))?;
    }

    // On Windows, rename the running exe first (Windows allows rename but not overwrite).
    #[cfg(windows)]
    {
        let old_path = target.with_extension("exe.old");
        // Remove leftover from previous update
        if old_path.exists() {
            std::fs::remove_file(&old_path).ok();
        }
        std::fs::rename(target, &old_path)?;
        return match std::fs::copy(new_binary, target) {
            Ok(_) => Ok(()),
            Err(e) => {
                // Rollback: restore original binary
                std::fs::rename(&old_path, target).ok();
                Err(e.into())
            }
        };
    }

    // Unix: direct copy, with sudo fallback for system-wide installs
    #[cfg(unix)]
    {
        match std::fs::copy(new_binary, target) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("{}", crate::messages::error::ADMIN_REQUIRED);
                let status = std::process::Command::new("sudo")
                    .args(["cp", &new_binary.to_string_lossy(), &target.to_string_lossy()])
                    .status()?;
                if status.success() {
                    return Ok(());
                }
                Err(crate::messages::error::BINARY_REPLACE_FAILED.into())
            }
            Err(e) => Err(e.into()),
        }
    }
}
