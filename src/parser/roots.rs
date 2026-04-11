use std::collections::HashSet;
use std::path::PathBuf;

/// Detects whether the current Linux runtime is WSL.
pub fn is_wsl_environment() -> bool {
    if std::env::var_os("WSL_DISTRO_NAME").is_some() {
        return true;
    }

    std::fs::read_to_string("/proc/version")
        .map(|s| s.to_ascii_lowercase().contains("microsoft"))
        .unwrap_or(false)
}

/// Returns candidate Windows home directories visible from WSL.
pub fn wsl_windows_home_candidates() -> Vec<PathBuf> {
    let mut homes: Vec<PathBuf> = Vec::new();

    if let Some(userprofile) = std::env::var_os("USERPROFILE") {
        homes.push(PathBuf::from(userprofile));
    }

    if let Ok(entries) = std::fs::read_dir("/mnt/c/Users") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                homes.push(path);
            }
        }
    }

    dedupe_existing_paths(homes)
}

/// Keeps only existing directories, preserving the first-seen order.
/// Paths are deduped by canonical path when possible.
pub fn dedupe_existing_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut deduped = Vec::new();

    for path in paths {
        if !path.is_dir() {
            continue;
        }
        let normalized = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if seen.insert(normalized) {
            deduped.push(path);
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedupe_existing_paths_keeps_first_order() {
        let base = std::env::temp_dir().join(format!(
            "rwd_test_parser_roots_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let first = base.join("first");
        let second = base.join("second");
        std::fs::create_dir_all(&first).expect("first dir");
        std::fs::create_dir_all(&second).expect("second dir");

        let missing = base.join("missing");
        let deduped =
            dedupe_existing_paths(vec![first.clone(), second.clone(), first.clone(), missing]);

        assert_eq!(deduped, vec![first, second]);

        std::fs::remove_dir_all(&base).ok();
    }
}
