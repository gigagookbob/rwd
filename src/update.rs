// Version check and self-update via GitHub Releases.

use std::path::{Path, PathBuf};

const REPO: &str = "gigagookbob/rwd";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns true if `latest` is strictly newer than `current` using semver.
/// Falls back to string inequality if either version fails to parse.
fn is_newer(latest: &str, current: &str) -> bool {
    match (
        semver::Version::parse(latest),
        semver::Version::parse(current),
    ) {
        (Ok(l), Ok(c)) => l > c,
        _ => latest != current,
    }
}

fn is_local_dev_binary(current_exe: &Path) -> bool {
    // Avoid embedding build-machine absolute paths via `env!("CARGO_MANIFEST_DIR")`.
    // A local cargo binary path usually includes `.../target/{debug|release}/...`.
    let mut prev_was_target = false;
    for component in current_exe.components() {
        let name = component.as_os_str().to_string_lossy();
        if prev_was_target && (name == "debug" || name == "release") {
            return true;
        }
        prev_was_target = name == "target";
    }
    false
}

fn should_skip_update_notice(current_exe: &Path) -> bool {
    if std::env::var_os("RWD_FORCE_UPDATE_CHECK").is_some() {
        return false;
    }
    cfg!(debug_assertions)
        || is_local_dev_binary(current_exe)
        || std::env::var_os("RWD_DISABLE_UPDATE_CHECK").is_some()
}

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
    let Ok(current_exe) = std::env::current_exe() else {
        return;
    };
    if should_skip_update_notice(&current_exe) {
        return;
    }

    if let Some(cached) = crate::cache::load_update_check() {
        let now = chrono::Utc::now();
        let interval = chrono::Duration::hours(24);
        if is_newer(&cached.latest_version, CURRENT_VERSION) && now - cached.checked_at < interval {
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

/// Prints update notice if latest is strictly newer than current version.
fn print_update_notice(latest_version: &str) {
    if is_newer(latest_version, CURRENT_VERSION) {
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

    let current_exe = std::env::current_exe()?;

    if !is_newer(&latest, CURRENT_VERSION) {
        #[cfg(unix)]
        warn_duplicate_binaries_in_path(&current_exe);
        eprintln!(
            "{}",
            crate::messages::update::already_latest(CURRENT_VERSION)
        );
        return Ok(());
    }

    eprintln!(
        "{}",
        crate::messages::update::updating(CURRENT_VERSION, &latest)
    );

    // Determine platform-specific asset name
    let asset_name = detect_asset_name()?;
    let download_url =
        format!("https://github.com/{REPO}/releases/download/v{latest}/{asset_name}");

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
        let user_target = replace_binary_unix(&extracted, &current_exe)?;
        std::fs::remove_dir_all(&tmp_dir).ok();
        let active_target = user_target.clone().unwrap_or_else(|| current_exe.clone());
        if let Some(path) = user_target {
            eprintln!(
                "{}",
                crate::messages::update::user_bin_update_complete(&path.display(), &latest)
            );
            eprintln!("{}", crate::messages::update::USER_BIN_PATH_HINT);
        } else {
            eprintln!("{}", crate::messages::update::update_complete(&latest));
        }
        warn_duplicate_binaries_in_path(&active_target);
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

    let tmp_dir = new_binary.parent().ok_or("cannot resolve temp directory")?;
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
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(new_binary, std::fs::Permissions::from_mode(0o755))?;

    match stage_and_rename_unix(new_binary, target) {
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            if let Ok(Some(user_target)) = try_replace_in_user_bin(new_binary, target) {
                return Ok(Some(user_target));
            }

            eprintln!("{}", crate::messages::error::ADMIN_REQUIRED);
            // Privileged fallback: copy to sibling temp path, then atomic rename.
            let staged = target.with_extension("new");

            let copy_status = std::process::Command::new("sudo")
                .args([
                    "cp",
                    &new_binary.to_string_lossy(),
                    &staged.to_string_lossy(),
                ])
                .status()?;
            if !copy_status.success() {
                return Err(crate::messages::error::BINARY_REPLACE_FAILED.into());
            }

            let chmod_status = std::process::Command::new("sudo")
                .args(["chmod", "755", &staged.to_string_lossy()])
                .status()?;
            if !chmod_status.success() {
                let _ = std::process::Command::new("sudo")
                    .args(["rm", "-f", &staged.to_string_lossy()])
                    .status();
                return Err(crate::messages::error::BINARY_REPLACE_FAILED.into());
            }

            let move_status = std::process::Command::new("sudo")
                .args([
                    "mv",
                    "-f",
                    &staged.to_string_lossy(),
                    &target.to_string_lossy(),
                ])
                .status()?;
            if move_status.success() {
                return Ok(None);
            }
            Err(crate::messages::error::BINARY_REPLACE_FAILED.into())
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(unix)]
fn try_replace_in_user_bin(
    new_binary: &std::path::Path,
    target: &std::path::Path,
) -> Result<Option<PathBuf>, std::io::Error> {
    let home = match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home),
        None => return Ok(None),
    };
    let Some(user_target) = user_bin_target_for(target, &home) else {
        return Ok(None);
    };
    let Some(parent) = user_target.parent() else {
        return Ok(None);
    };

    std::fs::create_dir_all(parent)?;
    stage_and_rename_unix(new_binary, &user_target)?;
    Ok(Some(user_target))
}

#[cfg(unix)]
fn user_bin_target_for(target: &std::path::Path, home: &std::path::Path) -> Option<PathBuf> {
    let file_name = target.file_name()?;
    let user_target = home.join(".local").join("bin").join(file_name);
    if user_target == target {
        return None;
    }
    Some(user_target)
}

#[cfg(unix)]
fn warn_duplicate_binaries_in_path(active_binary: &Path) {
    use std::collections::HashSet;

    let Some(path_var) = std::env::var_os("PATH") else {
        return;
    };

    let mut seen = HashSet::new();
    let mut visible = Vec::new();

    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join("rwd");
        if !candidate.is_file() {
            continue;
        }
        let normalized = normalized_path(&candidate);
        if !seen.insert(normalized.clone()) {
            continue;
        }
        visible.push(candidate);
    }

    if visible.is_empty() {
        return;
    }

    let requested_active = normalized_path(active_binary);
    let active = if visible
        .iter()
        .any(|candidate| normalized_path(candidate) == requested_active)
    {
        active_binary.to_path_buf()
    } else {
        visible[0].clone()
    };
    let active_normalized = normalized_path(&active);
    let duplicates: Vec<PathBuf> = visible
        .into_iter()
        .filter(|candidate| normalized_path(candidate) != active_normalized)
        .collect();

    if duplicates.is_empty() {
        return;
    }

    eprintln!("{}", crate::messages::update::DUPLICATE_BINARIES_FOUND);
    eprintln!(
        "{}",
        crate::messages::update::active_binary(&active.display())
    );
    for duplicate in duplicates {
        let command = cleanup_command_for_duplicate(&duplicate);
        eprintln!(
            "{}",
            crate::messages::update::cleanup_duplicate(&duplicate.display(), &command)
        );
    }
}

#[cfg(unix)]
fn normalized_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(unix)]
fn cleanup_command_for_duplicate(path: &Path) -> String {
    if path == Path::new("/usr/local/bin/rwd") {
        return "sudo rm /usr/local/bin/rwd".to_string();
    }
    if path.to_string_lossy().ends_with("/.cargo/bin/rwd") {
        return "cargo uninstall rwd".to_string();
    }
    format!("rm {}", path.display())
}

#[cfg(unix)]
fn stage_and_rename_unix(
    new_binary: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), std::io::Error> {
    use std::io::ErrorKind;
    use std::os::unix::fs::PermissionsExt;

    let parent = target.parent().ok_or_else(|| {
        std::io::Error::new(ErrorKind::NotFound, "target has no parent directory")
    })?;
    let staged = parent.join(format!(".rwd-update-{}.tmp", std::process::id()));

    // Best-effort cleanup of a stale temp file from a previous failed attempt.
    let _ = std::fs::remove_file(&staged);
    std::fs::copy(new_binary, &staged)?;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;

    match std::fs::rename(&staged, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&staged);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn newer_version_returns_true() {
        assert!(is_newer("0.12.0", "0.11.4"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("0.11.5", "0.11.4"));
    }

    #[test]
    fn same_version_returns_false() {
        assert!(!is_newer("0.11.4", "0.11.4"));
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn older_version_returns_false() {
        assert!(!is_newer("0.11.3", "0.11.4"));
        assert!(!is_newer("0.5.0", "0.5.1"));
        assert!(!is_newer("0.10.0", "0.11.0"));
    }

    #[test]
    fn prerelease_is_older_than_release() {
        assert!(!is_newer("0.12.0-beta.1", "0.12.0"));
        assert!(is_newer("0.12.0", "0.12.0-beta.1"));
    }

    #[test]
    fn invalid_version_falls_back_to_string_compare() {
        assert!(is_newer("not-a-version", "0.11.4"));
        assert!(!is_newer("same", "same"));
    }

    #[test]
    fn local_dev_binary_detects_manifest_target_path() {
        let current = std::path::Path::new("/tmp/example/target/debug/rwd");
        assert!(super::is_local_dev_binary(current));
    }

    #[test]
    fn local_dev_binary_false_for_installed_path() {
        let current = std::path::Path::new("/usr/local/bin/rwd");
        assert!(!super::is_local_dev_binary(current));
    }

    #[cfg(unix)]
    #[test]
    fn stage_and_rename_unix_replaces_target_atomically() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir =
            std::env::temp_dir().join(format!("rwd_update_stage_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");

        let source = temp_dir.join("source.bin");
        let target = temp_dir.join("target.bin");

        std::fs::write(&source, b"new-content").expect("write source");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
            .expect("chmod source");
        std::fs::write(&target, b"old-content").expect("write target");

        super::stage_and_rename_unix(&source, &target).expect("replace target");

        let replaced = std::fs::read(&target).expect("read target");
        assert_eq!(replaced, b"new-content");

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn user_bin_target_for_system_binary_returns_local_bin_path() {
        let home = std::path::PathBuf::from("/Users/tester");
        let target = std::path::PathBuf::from("/usr/local/bin/rwd");
        let actual = super::user_bin_target_for(&target, &home).expect("target path");
        assert_eq!(
            actual,
            std::path::PathBuf::from("/Users/tester/.local/bin/rwd")
        );
    }

    #[cfg(unix)]
    #[test]
    fn user_bin_target_for_same_path_returns_none() {
        let home = std::path::PathBuf::from("/Users/tester");
        let target = std::path::PathBuf::from("/Users/tester/.local/bin/rwd");
        let actual = super::user_bin_target_for(&target, &home);
        assert!(actual.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_command_for_usr_local_uses_sudo_remove() {
        let path = std::path::PathBuf::from("/usr/local/bin/rwd");
        let command = super::cleanup_command_for_duplicate(&path);
        assert_eq!(command, "sudo rm /usr/local/bin/rwd");
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_command_for_cargo_bin_uses_cargo_uninstall() {
        let path = std::path::PathBuf::from("/Users/tester/.cargo/bin/rwd");
        let command = super::cleanup_command_for_duplicate(&path);
        assert_eq!(command, "cargo uninstall rwd");
    }
}
