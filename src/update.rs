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
    // Clean up leftover .old binary from previous rename-based updates
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

    let current_exe = std::env::current_exe()?;

    // Update cache before potential process exit (Windows deferred path)
    let cache = crate::cache::UpdateCheckCache {
        checked_at: chrono::Utc::now(),
        latest_version: latest.clone(),
    };
    let _ = crate::cache::save_update_check(&cache);

    // On Windows, spawn a helper script and exit so the file lock is released.
    #[cfg(windows)]
    {
        schedule_deferred_replace(&extracted, &current_exe, &latest)?;
        eprintln!("{}", crate::messages::update::update_deferred(&latest));
        std::process::exit(0);
    }

    // On Unix, replace in-place.
    #[cfg(unix)]
    {
        replace_binary_unix(&extracted, &current_exe)?;
        std::fs::remove_dir_all(&tmp_dir).ok();
        eprintln!("{}", crate::messages::update::update_complete(&latest));
    }

    #[allow(unreachable_code)]
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

/// Cleans up leftover `.old` binary from a previous rename-based update.
fn cleanup_old_binary() {
    let Ok(current_exe) = std::env::current_exe() else {
        return;
    };
    let old_path = current_exe.with_extension("exe.old");
    if old_path.exists() {
        std::fs::remove_file(&old_path).ok();
    }
}

/// Windows: writes a batch script that waits for the file lock to release,
/// then copies the new binary. The script runs after rwd exits.
#[cfg(windows)]
fn schedule_deferred_replace(
    new_binary: &std::path::Path,
    target: &std::path::Path,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::windows::process::CommandExt;

    let tmp_dir = new_binary
        .parent()
        .ok_or("cannot resolve temp directory")?;
    let script_path = std::env::temp_dir().join("rwd_update.cmd");

    let script = format!(
        "@echo off\r\n\
         set retries=0\r\n\
         :retry\r\n\
         timeout /t 1 /nobreak >nul\r\n\
         copy /y \"{source}\" \"{target}\" >nul 2>&1\r\n\
         if errorlevel 1 (\r\n\
             set /a retries+=1\r\n\
             if %retries% geq 30 (\r\n\
                 echo rwd update failed: could not replace binary after 30s.\r\n\
                 goto cleanup\r\n\
             )\r\n\
             goto retry\r\n\
         )\r\n\
         echo rwd v{version} update complete!\r\n\
         :cleanup\r\n\
         rmdir /s /q \"{tmp_dir}\" >nul 2>&1\r\n\
         del \"%~f0\" >nul 2>&1\r\n",
        source = new_binary.display(),
        target = target.display(),
        version = version,
        tmp_dir = tmp_dir.display(),
    );
    std::fs::write(&script_path, &script)?;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args(["/C", &script_path.to_string_lossy()])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;

    Ok(())
}

/// Unix: replaces the current binary in-place, with sudo fallback.
#[cfg(unix)]
fn replace_binary_unix(
    new_binary: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(new_binary, std::fs::Permissions::from_mode(0o755))?;

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
