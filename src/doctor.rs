use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckLevel {
    Ok,
    Warn,
    Fail,
}

struct CheckResult {
    name: &'static str,
    level: CheckLevel,
    lines: Vec<String>,
}

pub fn run_doctor() -> Result<(), Box<dyn std::error::Error>> {
    let (path_check, active_binary) = check_binary_path()?;
    let update_check = check_update_write_access(&active_binary)?;
    let config_check = check_config_file()?;
    let roots_check = check_log_roots();

    let results = vec![path_check, update_check, config_check, roots_check];
    print_report(&results);
    Ok(())
}

fn check_binary_path() -> Result<(CheckResult, PathBuf), Box<dyn std::error::Error>> {
    let binaries = visible_rwd_binaries();
    if binaries.is_empty() {
        let result = CheckResult {
            name: "Binary discovery",
            level: CheckLevel::Fail,
            lines: vec!["No `rwd` binary found in PATH.".to_string()],
        };
        return Ok((result, std::env::current_exe()?));
    }

    let current_exe = std::env::current_exe()?;
    let current_norm = normalized_path(&current_exe);
    let active = if binaries
        .iter()
        .any(|candidate| normalized_path(candidate) == current_norm)
    {
        current_exe
    } else {
        binaries[0].clone()
    };
    let active_norm = normalized_path(&active);

    let mut lines = vec![format!("Active binary: {}", active.display())];
    let mut level = CheckLevel::Ok;

    for candidate in &binaries {
        if normalized_path(candidate) == active_norm {
            continue;
        }
        level = CheckLevel::Warn;
        let cleanup = cleanup_command_for_duplicate(candidate);
        lines.push(format!(
            "Duplicate found: {} (cleanup: {cleanup})",
            candidate.display()
        ));
    }

    Ok((
        CheckResult {
            name: "Binary discovery",
            level,
            lines,
        },
        active,
    ))
}

fn visible_rwd_binaries() -> Vec<PathBuf> {
    use std::collections::HashSet;
    let Some(path_var) = std::env::var_os("PATH") else {
        return Vec::new();
    };

    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary_name());
        if !candidate.is_file() {
            continue;
        }
        let canonical = normalized_path(&candidate);
        if seen.insert(canonical) {
            found.push(candidate);
        }
    }
    found
}

fn binary_name() -> &'static str {
    if cfg!(windows) { "rwd.exe" } else { "rwd" }
}

fn normalized_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn cleanup_command_for_duplicate(path: &Path) -> String {
    #[cfg(unix)]
    {
        if path == Path::new("/usr/local/bin/rwd") {
            return "sudo rm /usr/local/bin/rwd".to_string();
        }
    }
    if path.to_string_lossy().ends_with(".cargo/bin/rwd")
        || path.to_string_lossy().ends_with(".cargo\\bin\\rwd.exe")
    {
        return "cargo uninstall rwd".to_string();
    }
    if cfg!(windows) {
        format!("del {}", path.display())
    } else {
        format!("rm {}", path.display())
    }
}

fn check_update_write_access(
    active_binary: &Path,
) -> Result<CheckResult, Box<dyn std::error::Error>> {
    let Some(parent) = active_binary.parent() else {
        return Ok(CheckResult {
            name: "Update permissions",
            level: CheckLevel::Fail,
            lines: vec!["Cannot resolve the active binary directory.".to_string()],
        });
    };

    let writable = can_write_to_dir(parent)?;
    let (level, message) = if writable {
        (
            CheckLevel::Ok,
            format!("Writable install directory: {}", parent.display()),
        )
    } else {
        (
            CheckLevel::Warn,
            format!("Directory is not writable: {}", parent.display()),
        )
    };
    Ok(CheckResult {
        name: "Update permissions",
        level,
        lines: vec![message],
    })
}

fn can_write_to_dir(dir: &Path) -> Result<bool, std::io::Error> {
    use std::io::ErrorKind;
    let probe = dir.join(format!(".rwd-doctor-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            std::fs::remove_file(&probe).ok();
            Ok(true)
        }
        Err(e) if e.kind() == ErrorKind::PermissionDenied => Ok(false),
        Err(e) => Err(e),
    }
}

fn check_config_file() -> Result<CheckResult, Box<dyn std::error::Error>> {
    let path = crate::config::config_path()?;
    if !path.exists() {
        return Ok(CheckResult {
            name: "Config file",
            level: CheckLevel::Warn,
            lines: vec![format!(
                "Missing config at {} (run `rwd init`).",
                path.display()
            )],
        });
    }

    match crate::config::load_config(&path) {
        Ok(config) => Ok(CheckResult {
            name: "Config file",
            level: CheckLevel::Ok,
            lines: vec![format!(
                "Config loaded: {} (provider: {})",
                path.display(),
                config.llm.provider
            )],
        }),
        Err(e) => Ok(CheckResult {
            name: "Config file",
            level: CheckLevel::Fail,
            lines: vec![format!("Config parse failed at {}: {e}", path.display())],
        }),
    }
}

fn check_log_roots() -> CheckResult {
    let cfg = crate::config::load_config_if_exists();
    let claude_overrides = cfg
        .as_ref()
        .and_then(|c| c.input.as_ref())
        .and_then(|i| i.claude_roots.as_deref());
    let codex_overrides = cfg
        .as_ref()
        .and_then(|c| c.input.as_ref())
        .and_then(|i| i.codex_roots.as_deref());

    let claude_roots = crate::parser::claude::discover_claude_log_roots(claude_overrides);
    let codex_roots = crate::parser::codex::discover_codex_session_roots(codex_overrides);
    if claude_roots.is_empty() && codex_roots.is_empty() {
        return CheckResult {
            name: "Session roots",
            level: CheckLevel::Warn,
            lines: vec!["No Claude/Codex log roots were discovered.".to_string()],
        };
    }

    CheckResult {
        name: "Session roots",
        level: CheckLevel::Ok,
        lines: vec![
            format!("Claude Code roots: {}", claude_roots.len()),
            format!("Codex roots: {}", codex_roots.len()),
        ],
    }
}

fn print_report(results: &[CheckResult]) {
    println!("=== rwd doctor ===");
    let mut ok = 0usize;
    let mut warn = 0usize;
    let mut fail = 0usize;

    for result in results {
        let marker = match result.level {
            CheckLevel::Ok => {
                ok += 1;
                "OK"
            }
            CheckLevel::Warn => {
                warn += 1;
                "WARN"
            }
            CheckLevel::Fail => {
                fail += 1;
                "FAIL"
            }
        };
        println!("[{marker}] {}", result.name);
        for line in &result.lines {
            println!("  - {line}");
        }
    }

    println!("Summary: {ok} ok, {warn} warning, {fail} fail");
}
