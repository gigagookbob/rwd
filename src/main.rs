mod analyzer;
mod cache;
mod cli;
mod config;
mod messages;
mod output;
mod parser;
mod redactor;
mod update;

use clap::Parser;
use cli::Commands;

// ANSI color codes — visible on both light and dark terminals.
const CYAN: &str = "\x1b[36m";
const BRIGHT_BLUE: &str = "\x1b[94m";
const YELLOW: &str = "\x1b[33m";
const MAGENTA: &str = "\x1b[35m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const RESET: &str = "\x1b[0m";

type CodexCollectedSession = (
    parser::codex::CodexSessionSummary,
    Vec<parser::codex::CodexEntry>,
);

#[tokio::main]
async fn main() {
    let args = cli::Cli::parse();

    // Show update notification only for synchronous commands.
    // Worker mode skips this to avoid blocking (no terminal).
    let skip_update = matches!(args.command, Commands::Update)
        || matches!(args.command, Commands::Today { worker: true, .. });
    if !skip_update {
        update::notify_if_update_available().await;
    }

    match args.command {
        Commands::Today {
            verbose,
            lang,
            date,
            background,
            no_cache,
            worker,
        } => {
            let target_date = match parse_date_flag(&date) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            if worker {
                if let Err(e) = run_worker(lang, target_date, no_cache).await {
                    let log_path = worker_log_path();
                    if let Some(parent) = log_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(&log_path, format!("{e}"));
                    send_notification(
                        crate::messages::background::NOTIFY_TITLE,
                        &crate::messages::background::notify_failure(&log_path.display()),
                    );
                    std::process::exit(1);
                }
            } else if background {
                if let Err(e) = spawn_worker(&lang, &date, no_cache) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            } else {
                if let Err(e) = run_today(verbose, lang, target_date, no_cache).await {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Init => {
            if let Err(e) = config::run_init() {
                eprintln!("{}", crate::messages::error::init_failed(&e));
                std::process::exit(1);
            }
        }
        Commands::Config { key, value } => {
            let result = match (key, value) {
                (Some(k), Some(v)) => config::run_config(&k, &v),
                (None, None) => config::run_config_interactive().await,
                _ => Err(crate::messages::config::USAGE.into()),
            };
            if let Err(e) = result {
                eprintln!("{}", crate::messages::error::config_failed(&e));
                std::process::exit(1);
            }
        }
        Commands::Update => {
            if let Err(e) = update::run_update().await {
                eprintln!("{}", crate::messages::error::update_failed(&e));
                std::process::exit(1);
            }
        }
        Commands::Summary {
            lang,
            date,
            no_cache,
        } => {
            let target_date = match parse_date_flag(&date) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            if let Err(e) = run_summary(lang, target_date, no_cache).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Commands::Slack {
            lang,
            date,
            no_cache,
        } => {
            let target_date = match parse_date_flag(&date) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };
            if let Err(e) = run_slack(lang, target_date, no_cache).await {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}

/// Parses `--date` flag into `NaiveDate`. Returns today if `None`.
fn parse_date_flag(date: &Option<String>) -> Result<chrono::NaiveDate, String> {
    match date {
        None => Ok(chrono::Local::now().date_naive()),
        Some(s) => chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| format!("Invalid date format: '{s}'. Expected YYYY-MM-DD.")),
    }
}

/// Returns the path to ~/.rwd/worker.lock
fn worker_lock_path() -> std::path::PathBuf {
    dirs::home_dir()
        .expect(crate::messages::error::HOME_DIR_NOT_FOUND)
        .join(".rwd")
        .join("worker.lock")
}

/// Returns the path to ~/.rwd/worker.log
fn worker_log_path() -> std::path::PathBuf {
    dirs::home_dir()
        .expect(crate::messages::error::HOME_DIR_NOT_FOUND)
        .join(".rwd")
        .join("worker.log")
}

/// Checks if a process with the given PID is alive (no unsafe, uses kill -0 command).
#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
        .unwrap_or(false)
}

/// Checks if a lock file exists with a live process.
fn is_worker_running() -> bool {
    let lock_path = worker_lock_path();
    if !lock_path.exists() {
        return false;
    }
    if let Ok(contents) = std::fs::read_to_string(&lock_path)
        && let Ok(pid) = contents.trim().parse::<u32>()
        && is_process_alive(pid)
    {
        return true;
    }
    // Stale lock — remove it
    let _ = std::fs::remove_file(&lock_path);
    false
}

/// Spawns a background worker process.
fn spawn_worker(
    lang_flag: &Option<String>,
    date_flag: &Option<String>,
    no_cache: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if is_worker_running() {
        println!("{}", crate::messages::background::ALREADY_RUNNING);
        return Ok(());
    }

    let exe = std::env::current_exe()?;
    let mut args = vec!["today".to_string(), "--worker".to_string()];
    if let Some(lang) = lang_flag {
        args.push("--lang".to_string());
        args.push(lang.clone());
    } else {
        // Resolve lang from config now (before detaching) to avoid stdin prompt in worker.
        // Fail synchronously if language cannot be resolved.
        let mut loaded_config = config::load_config_if_exists();
        let lang = resolve_lang(&None, &mut loaded_config)?;
        args.push("--lang".to_string());
        args.push(lang.to_string());
    }
    if let Some(date) = date_flag {
        args.push("--date".to_string());
        args.push(date.clone());
    }
    if no_cache {
        args.push("--no-cache".to_string());
    }

    let child = std::process::Command::new(exe)
        .args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    println!("  {}", crate::messages::background::starting(child.id()));
    println!("  {}", crate::messages::background::NOTIFIED_WHEN_DONE);

    // Show where results will be saved.
    let display_date =
        parse_date_flag(date_flag).unwrap_or_else(|_| chrono::Local::now().date_naive());
    if let Ok(vault_path) = output::load_vault_path() {
        let file_path = vault_path.join(format!("{display_date}.md"));
        println!(
            "  {}",
            crate::messages::background::results_path(&file_path.display())
        );
    }

    Ok(())
}

/// Runs as a background worker: lock, analyze, notify, unlock.
async fn run_worker(
    lang: Option<String>,
    target_date: chrono::NaiveDate,
    no_cache: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let lock_path = worker_lock_path();
    if let Some(parent) = lock_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&lock_path, std::process::id().to_string())?;

    let result = run_today(false, lang, target_date, no_cache).await;

    // Always clean up lock file
    let _ = std::fs::remove_file(&lock_path);

    match result {
        Ok(()) => {
            // Clean up previous error log on success
            let log_path = worker_log_path();
            if log_path.exists() {
                let _ = std::fs::remove_file(&log_path);
            }
            send_notification(
                crate::messages::background::NOTIFY_TITLE,
                crate::messages::background::NOTIFY_SUCCESS,
            );
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Sends an OS notification via notify-rust.
fn send_notification(title: &str, body: &str) {
    let mut notification = notify_rust::Notification::new();
    notification.summary(title).body(body);

    #[cfg(target_os = "macos")]
    notification.sound_name(crate::messages::background::NOTIFY_SOUND);

    notification.show().ok();
}

/// Resolves the language from: --lang flag > config > migration prompt.
fn resolve_lang(
    flag: &Option<String>,
    loaded_config: &mut Option<config::Config>,
) -> Result<config::Lang, Box<dyn std::error::Error>> {
    // 1. --lang flag takes priority
    if let Some(lang_str) = flag {
        return match lang_str.as_str() {
            "ko" => Ok(config::Lang::Ko),
            "en" => Ok(config::Lang::En),
            _ => Err(crate::messages::lang::unsupported(lang_str).into()),
        };
    }
    // 2. Config value
    if let Some(cfg) = loaded_config.as_ref()
        && let Some(lang) = &cfg.lang
    {
        return Ok(lang.clone());
    }
    // 3. Migration prompt — ask user and save to config
    eprint!("{}", crate::messages::lang::NOT_CONFIGURED);
    use std::io::Write;
    let _ = std::io::stderr().flush();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let lang = match input.trim() {
        "ko" => config::Lang::Ko,
        _ => config::Lang::En,
    };
    // Save to config if available
    if let Some(cfg) = loaded_config.as_mut() {
        cfg.lang = Some(lang.clone());
        if let Ok(config_file) = config::config_path() {
            let _ = config::save_config(cfg, &config_file);
        }
        eprintln!("{}", crate::messages::lang::saved(&lang.to_string()));
    }
    Ok(lang)
}

/// Parses session logs for the given date, runs LLM analysis, and prints insights.
async fn run_today(
    verbose: bool,
    lang_flag: Option<String>,
    target_date: chrono::NaiveDate,
    no_cache: bool,
) -> Result<(), parser::ParseError> {
    let mut loaded_config = config::load_config_if_exists();
    if loaded_config.is_none() {
        eprintln!("{}", crate::messages::error::NO_CONFIG);
        std::process::exit(1);
    }
    let redactor_enabled = loaded_config.as_ref().unwrap().is_redactor_enabled();
    let lang = resolve_lang(&lang_flag, &mut loaded_config)
        .map_err(|e| -> Box<dyn std::error::Error> { e })?;

    let today = target_date;

    // === Collect Claude Code logs ===
    let (claude_entries, claude_discovery) =
        collect_claude_entries_with_stats(today, loaded_config.as_ref());

    // === Collect Codex logs ===
    let (codex_sessions, codex_roots) = collect_codex_sessions(today, loaded_config.as_ref());

    let codex_entry_count: usize = codex_sessions
        .iter()
        .map(|(_, entries)| entries.len())
        .sum();

    if verbose {
        eprintln!(
            "{}",
            crate::messages::verbose::discover_stats(
                claude_discovery.project_count,
                claude_discovery.file_count,
                claude_discovery.total_entries,
                claude_entries.len() + codex_entry_count,
            )
        );
        if !claude_discovery.roots.is_empty() {
            eprintln!(
                "{}",
                crate::messages::verbose::used_roots(
                    "Claude",
                    &format_roots_for_display(&claude_discovery.roots)
                )
            );
        }
        if !codex_roots.is_empty() {
            eprintln!(
                "{}",
                crate::messages::verbose::used_roots(
                    "Codex",
                    &format_roots_for_display(&codex_roots)
                )
            );
        }
    }

    if claude_entries.is_empty() && codex_sessions.is_empty() {
        println!("No log entries found for today ({today}).");
        return Ok(());
    }

    let claude_count = claude_entries.len();
    let codex_session_count = codex_sessions.len();

    // === Logo banner ===
    print_logo_banner();

    // === Info box ===
    let summaries = if !claude_entries.is_empty() {
        Some(parser::summarize_entries(&claude_entries))
    } else {
        None
    };

    print_info_box(
        today,
        summaries.as_deref(),
        &claude_entries,
        &codex_sessions,
    );

    // === Cache check ===
    // Reuse previous analysis if the entry count is unchanged.
    if no_cache {
        println!("\n{}", crate::messages::status::CACHE_BYPASSED);
    } else if let Some(cached) = cache::load_cache(today)
        && cached.claude_entry_count == claude_count
        && cached.codex_session_count == codex_session_count
        && cached.codex_entry_count == codex_entry_count
    {
        println!("\n{}", crate::messages::status::CACHE_USED);
        let source_refs: Vec<(&str, &analyzer::AnalysisResult)> = cached
            .sources
            .iter()
            .map(|(name, result)| (name.as_str(), result))
            .collect();
        for (name, analysis) in &source_refs {
            print_insights(name, analysis);
        }
        save_combined_analysis(&source_refs, today, verbose);
        return Ok(());
    }

    // === LLM analysis ===
    let provider_label = analyzer::provider::load_provider()
        .map(|(p, _)| p.display_name().to_string())
        .unwrap_or_else(|_| "LLM".to_string());
    println!(
        "\n{MAGENTA}{}{RESET}",
        crate::messages::status::analyzing(&provider_label)
    );

    let mut sources: Vec<(String, analyzer::AnalysisResult)> = Vec::new();
    let mut total_redact = redactor::RedactResult::empty();

    // Claude analysis
    if !claude_entries.is_empty() {
        let (result, redact_result) =
            analyzer::analyze_entries(&claude_entries, redactor_enabled, verbose, &lang).await?;
        total_redact.merge(redact_result);
        sources.push(("Claude Code".to_string(), result));
    }

    // Codex analysis — runs after Claude spinner finishes; fast enough to skip a spinner.
    for (summary, entries) in &codex_sessions {
        let (result, redact_result) =
            analyzer::analyze_codex_entries(entries, &summary.session_id, redactor_enabled, &lang)
                .await?;
        total_redact.merge(redact_result);
        sources.push(("Codex".to_string(), result));
    }

    // Output and save results
    if total_redact.total_count > 0 {
        println!(
            "{}",
            crate::messages::status::redacted(
                total_redact.total_count,
                &total_redact.format_summary()
            )
        );
    }

    if !sources.is_empty() {
        let source_refs: Vec<(&str, &analyzer::AnalysisResult)> = sources
            .iter()
            .map(|(name, result)| (name.as_str(), result))
            .collect();
        for (name, analysis) in &source_refs {
            print_insights(name, analysis);
        }
        save_combined_analysis(&source_refs, today, verbose);

        // Save analysis results to cache.
        let cache_data = cache::TodayCache {
            date: today.to_string(),
            claude_entry_count: claude_count,
            codex_session_count,
            codex_entry_count,
            sources,
        };
        if let Err(e) = cache::save_cache(&cache_data, today) {
            eprintln!("{}", crate::messages::error::cache_save_failed(&e));
        } else if verbose {
            let cache_path = dirs::home_dir()
                .expect(crate::messages::error::HOME_DIR_NOT_FOUND)
                .join(".rwd")
                .join("cache")
                .join(format!("today-{today}.json"));
            if let Ok(meta) = std::fs::metadata(&cache_path) {
                eprintln!(
                    "{}",
                    crate::messages::verbose::cache_saved(
                        &cache_path.display(),
                        meta.len() as f64 / 1024.0,
                    )
                );
            }
        }

        println!("\n{GREEN}{}{RESET}", crate::messages::status::REWIND_DONE);
    }

    Ok(())
}

/// Generates a development progress summary for today.
///
/// 1. Loads today's cache (runs `run_today()` first if missing).
/// 2. Collects work_summary from all sessions and sends to LLM.
/// 3. Prints summary to terminal, appends to daily Markdown, copies to clipboard.
async fn run_summary(
    _lang_flag: Option<String>,
    target_date: chrono::NaiveDate,
    no_cache: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let today = target_date;

    if no_cache {
        run_today(false, None, today, true).await?;
    }

    let cached = match cache::load_cache(today) {
        Some(c) => c,
        None => {
            println!("{}", crate::messages::error::NO_CACHE);
            run_today(false, None, today, false).await?;
            match cache::load_cache(today) {
                Some(c) => c,
                None => {
                    eprintln!("{}", crate::messages::error::NO_CACHE_AFTER_ANALYSIS);
                    std::process::exit(1);
                }
            }
        }
    };

    // Concatenate all session work_summaries with source names for project-level grouping.
    let mut summaries_text = String::new();
    for (source_name, analysis) in &cached.sources {
        for session in &analysis.sessions {
            summaries_text.push_str(&format!(
                "[{source_name} / {}] {}\n",
                session.session_id, session.work_summary
            ));
        }
    }

    if summaries_text.is_empty() {
        println!("{}", crate::messages::error::NO_SESSIONS);
        return Ok(());
    }

    println!("{}", crate::messages::status::SUMMARY_GENERATING);
    let mut loaded_config = config::load_config_if_exists();
    let lang = resolve_lang(&_lang_flag, &mut loaded_config).unwrap_or(config::Lang::En);
    let summary = analyzer::analyze_summary(&summaries_text, &lang).await?;

    println!("\n{}", crate::messages::status::SUMMARY_HEADER);
    println!("{summary}");

    // Append summary section to daily Markdown file.
    append_summary_to_markdown(today, &summary);

    copy_to_clipboard(&summary);
    println!("\n{}", crate::messages::status::COPIED_TO_CLIPBOARD);

    Ok(())
}

/// Generates a Slack-ready message from today's analysis.
///
/// Similar to `run_summary()` but uses SLACK_PROMPT for Slack-friendly formatting.
/// Only outputs to terminal and copies to clipboard (no Obsidian save).
async fn run_slack(
    _lang_flag: Option<String>,
    target_date: chrono::NaiveDate,
    no_cache: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let today = target_date;

    if no_cache {
        run_today(false, None, today, true).await?;
    }

    let cached = match cache::load_cache(today) {
        Some(c) => c,
        None => {
            println!("{}", crate::messages::error::NO_CACHE);
            run_today(false, None, today, false).await?;
            match cache::load_cache(today) {
                Some(c) => c,
                None => {
                    eprintln!("{}", crate::messages::error::NO_CACHE_AFTER_ANALYSIS);
                    std::process::exit(1);
                }
            }
        }
    };

    // Warn if cache is stale (entry count mismatch).
    let mut loaded_config = config::load_config_if_exists();
    let claude_count = collect_claude_entries(today, loaded_config.as_ref()).len();
    let (codex_sessions, _) = collect_codex_sessions(today, loaded_config.as_ref());
    let codex_session_count = codex_sessions.len();
    let codex_entry_count: usize = codex_sessions
        .iter()
        .map(|(_, entries)| entries.len())
        .sum();
    if cached.claude_entry_count != claude_count
        || cached.codex_session_count != codex_session_count
        || cached.codex_entry_count != codex_entry_count
    {
        let cached_total = cached.claude_entry_count + cached.codex_entry_count;
        let current_total = claude_count + codex_entry_count;
        eprintln!(
            "{YELLOW}{}{RESET}",
            crate::messages::status::cache_stale(cached_total, current_total)
        );
        eprintln!("{}", crate::messages::status::CACHE_STALE_HINT);
        eprintln!();
    }

    // Collect all session work_summaries.
    let mut summaries_text = String::new();
    for (source_name, analysis) in &cached.sources {
        for session in &analysis.sessions {
            summaries_text.push_str(&format!(
                "[{source_name} / {}] {}\n",
                session.session_id, session.work_summary
            ));
        }
    }

    if summaries_text.is_empty() {
        println!("{}", crate::messages::error::NO_SESSIONS);
        return Ok(());
    }

    println!("{}", crate::messages::status::SLACK_GENERATING);
    let lang = resolve_lang(&_lang_flag, &mut loaded_config).unwrap_or(config::Lang::En);
    let slack_message = analyzer::analyze_slack(&summaries_text, &lang).await?;

    println!("\n{slack_message}");

    copy_to_clipboard(&slack_message);
    println!("\n{}", crate::messages::status::COPIED_TO_CLIPBOARD);

    Ok(())
}

/// Copies text to the system clipboard. macOS: pbcopy, Linux: xclip.
fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut cmd = if cfg!(target_os = "macos") {
        Command::new("pbcopy")
    } else {
        let mut c = Command::new("xclip");
        c.arg("-selection").arg("clipboard");
        c
    };

    if let Ok(mut child) = cmd.stdin(Stdio::piped()).spawn() {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

/// Overwrites or appends the progress section in the daily Markdown file.
///
/// If the section already exists, replaces it in-place. Otherwise appends at the end.
/// Section range: from the header to the next `## ` header (or EOF).
fn append_summary_to_markdown(date: chrono::NaiveDate, summary: &str) {
    let vault_path = match output::load_vault_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", crate::messages::error::vault_path_load_failed(&e));
            return;
        }
    };

    let file_path = vault_path.join(format!("{date}.md"));
    if !file_path.exists() {
        eprintln!(
            "{}",
            crate::messages::error::daily_markdown_not_found(&file_path.display())
        );
        return;
    }

    let existing = match std::fs::read_to_string(&file_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", crate::messages::error::file_read_failed(&e));
            return;
        }
    };

    let section_header = crate::messages::markdown::PROGRESS_SECTION_HEADER;
    let new_section = format!("{section_header}\n\n{summary}\n");

    // Replace existing section, or append at the end.
    let updated = if let Some(start) = existing.find(section_header) {
        // Find the end: start of the next `## ` header, or EOF.
        let after_header = start + section_header.len();
        let end = existing[after_header..]
            .find("\n## ")
            .map(|pos| after_header + pos + 1)
            .unwrap_or(existing.len());

        format!(
            "{}{}\n{}",
            &existing[..start],
            new_section,
            &existing[end..]
        )
    } else {
        format!("{}\n{}\n", existing.trim_end(), new_section)
    };

    match std::fs::write(&file_path, updated) {
        Ok(()) => println!(
            "{}",
            crate::messages::status::markdown_saved(&file_path.display())
        ),
        Err(e) => eprintln!("{}", crate::messages::error::file_save_failed(&e)),
    }
}

/// Stats from log discovery (for verbose output).
struct DiscoveryStats {
    project_count: usize,
    file_count: usize,
    total_entries: usize,
    roots: Vec<std::path::PathBuf>,
}

fn claude_root_overrides(config: Option<&config::Config>) -> Option<&[String]> {
    config
        .and_then(|cfg| cfg.input.as_ref())
        .and_then(|input| input.claude_roots.as_deref())
}

fn codex_root_overrides(config: Option<&config::Config>) -> Option<&[String]> {
    config
        .and_then(|cfg| cfg.input.as_ref())
        .and_then(|input| input.codex_roots.as_deref())
}

fn format_roots_for_display(roots: &[std::path::PathBuf]) -> String {
    roots
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Collects Claude Code log entries with discovery statistics.
fn collect_claude_entries_with_stats(
    today: chrono::NaiveDate,
    config: Option<&config::Config>,
) -> (Vec<parser::claude::LogEntry>, DiscoveryStats) {
    let roots = parser::discover_claude_log_roots(claude_root_overrides(config));
    let mut stats = DiscoveryStats {
        project_count: 0,
        file_count: 0,
        total_entries: 0,
        roots: roots.clone(),
    };

    if roots.is_empty() {
        return (Vec::new(), stats);
    }

    let mut all_entries = Vec::new();
    for root in roots {
        if let Ok(project_dirs) = parser::list_project_dirs_in_root(&root) {
            stats.project_count += project_dirs.len();
            for project_dir in project_dirs {
                if let Ok(session_files) = parser::list_session_files(&project_dir) {
                    stats.file_count += session_files.len();
                    for session_file in session_files {
                        if let Ok(entries) = parser::parse_jsonl_file(&session_file) {
                            stats.total_entries += entries.len();
                            let today_entries = parser::filter_entries_by_date(entries, today);
                            all_entries.extend(today_entries);
                        }
                    }
                }
            }
        }
    }
    let mut deduped = parser::dedupe_claude_entries(all_entries);
    deduped.sort_by(|left, right| {
        match (
            parser::claude::entry_timestamp(left),
            parser::claude::entry_timestamp(right),
        ) {
            (Some(l), Some(r)) => l.cmp(&r),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    (deduped, stats)
}

/// Collects Claude Code log entries. Returns empty Vec if the log directory is missing.
fn collect_claude_entries(
    today: chrono::NaiveDate,
    config: Option<&config::Config>,
) -> Vec<parser::claude::LogEntry> {
    collect_claude_entries_with_stats(today, config).0
}

/// Collects Codex session logs. Returns empty Vec if the sessions directory is missing.
fn collect_codex_sessions(
    today: chrono::NaiveDate,
    config: Option<&config::Config>,
) -> (Vec<CodexCollectedSession>, Vec<std::path::PathBuf>) {
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum SessionMergeKey {
        SessionId(String),
        RootAndFile {
            root: std::path::PathBuf,
            rollout_filename: String,
        },
    }

    let roots = parser::codex::discover_codex_session_roots(codex_root_overrides(config));
    if roots.is_empty() {
        return (Vec::new(), roots);
    }

    let mut merged_entries: std::collections::HashMap<
        SessionMergeKey,
        Vec<parser::codex::CodexEntry>,
    > = std::collections::HashMap::new();

    for root in &roots {
        let Ok(session_files) = parser::codex::list_session_files_for_local_date(root, today)
        else {
            continue;
        };

        for file in session_files {
            let Ok(entries) = parser::codex::parse_codex_jsonl_file(&file) else {
                continue;
            };
            let entries = parser::codex::filter_entries_by_local_date(entries, today);
            let summary = parser::codex::summarize_codex_entries(&entries);
            // Keep only sessions with actual conversation on the target date.
            if summary.user_count == 0 && summary.assistant_count == 0 {
                continue;
            }

            let merge_key = if !summary.session_id.is_empty() {
                SessionMergeKey::SessionId(summary.session_id)
            } else {
                let rollout_filename = file
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| file.to_string_lossy().to_string());
                SessionMergeKey::RootAndFile {
                    root: root.clone(),
                    rollout_filename,
                }
            };

            merged_entries.entry(merge_key).or_default().extend(entries);
        }
    }

    let mut sessions = Vec::new();
    for mut entries in merged_entries.into_values() {
        entries = parser::codex::dedupe_entries(entries);
        parser::codex::sort_entries_by_timestamp(&mut entries);
        let summary = parser::codex::summarize_codex_entries(&entries);
        if summary.user_count > 0 || summary.assistant_count > 0 {
            sessions.push((summary, entries));
        }
    }

    sessions.sort_by(|(_, left_entries), (_, right_entries)| {
        let left_first = left_entries
            .iter()
            .filter_map(parser::codex::entry_timestamp)
            .min();
        let right_first = right_entries
            .iter()
            .filter_map(parser::codex::entry_timestamp)
            .min();
        match (left_first, right_first) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    (sessions, roots)
}

/// Returns the earliest local timestamp from Claude entries.
fn claude_earliest_time(
    entries: &[parser::claude::LogEntry],
) -> Option<chrono::DateTime<chrono::Local>> {
    entries
        .iter()
        .filter_map(parser::claude::entry_timestamp)
        .min()
        .map(|ts| ts.with_timezone(&chrono::Local))
}

/// Returns the latest local timestamp from Claude entries.
fn claude_latest_time(
    entries: &[parser::claude::LogEntry],
) -> Option<chrono::DateTime<chrono::Local>> {
    entries
        .iter()
        .filter_map(parser::claude::entry_timestamp)
        .max()
        .map(|ts| ts.with_timezone(&chrono::Local))
}

/// Returns the earliest local timestamp from Codex sessions.
fn codex_earliest_time(
    sessions: &[CodexCollectedSession],
) -> Option<chrono::DateTime<chrono::Local>> {
    sessions
        .iter()
        .flat_map(|(_, entries)| entries.iter())
        .filter_map(|e| match e {
            parser::codex::CodexEntry::SessionMeta { timestamp, .. }
            | parser::codex::CodexEntry::UserMessage { timestamp, .. }
            | parser::codex::CodexEntry::AssistantMessage { timestamp, .. }
            | parser::codex::CodexEntry::FunctionCall { timestamp, .. } => Some(*timestamp),
            parser::codex::CodexEntry::Other => None,
        })
        .min()
        .map(|ts| ts.with_timezone(&chrono::Local))
}

/// Returns the latest local timestamp from Codex sessions.
fn codex_latest_time(
    sessions: &[CodexCollectedSession],
) -> Option<chrono::DateTime<chrono::Local>> {
    sessions
        .iter()
        .flat_map(|(_, entries)| entries.iter())
        .filter_map(|e| match e {
            parser::codex::CodexEntry::SessionMeta { timestamp, .. }
            | parser::codex::CodexEntry::UserMessage { timestamp, .. }
            | parser::codex::CodexEntry::AssistantMessage { timestamp, .. }
            | parser::codex::CodexEntry::FunctionCall { timestamp, .. } => Some(*timestamp),
            parser::codex::CodexEntry::Other => None,
        })
        .max()
        .map(|ts| ts.with_timezone(&chrono::Local))
}

/// Computes total token counts from Claude session summaries: (total_in, total_out).
fn claude_total_tokens(summaries: &[parser::claude::SessionSummary]) -> (u64, u64) {
    summaries.iter().fold((0, 0), |(acc_in, acc_out), s| {
        let total_in =
            s.total_input_tokens + s.total_cache_creation_tokens + s.total_cache_read_tokens;
        (acc_in + total_in, acc_out + s.total_output_tokens)
    })
}

/// Formats a time range as "HH:MM ~ HH:MM".
fn format_time_range(
    earliest: Option<chrono::DateTime<chrono::Local>>,
    latest: Option<chrono::DateTime<chrono::Local>>,
) -> String {
    match (earliest, latest) {
        (Some(start), Some(end)) => format!("{} ~ {}", start.format("%H:%M"), end.format("%H:%M")),
        (Some(start), None) => format!("{} ~ ?", start.format("%H:%M")),
        (None, Some(end)) => format!("? ~ {}", end.format("%H:%M")),
        (None, None) => "? ~ ?".to_string(),
    }
}

/// Formats a number with thousands separators.
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Combines analysis results from multiple sources and saves as Markdown.
fn save_combined_analysis(
    sources: &[(&str, &analyzer::AnalysisResult)],
    date: chrono::NaiveDate,
    verbose: bool,
) {
    let vault_path = match output::load_vault_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", crate::messages::error::vault_path_load_failed(&e));
            return;
        }
    };

    let markdown = output::render_combined_markdown(sources, date);

    match output::save_to_vault(&vault_path, date, &markdown) {
        Ok(saved) => {
            println!(
                "\n{}",
                crate::messages::status::markdown_saved(&saved.display())
            );
            if verbose && let Ok(meta) = std::fs::metadata(&saved) {
                eprintln!(
                    "{}",
                    crate::messages::verbose::markdown_file_size(
                        &saved.display(),
                        meta.len() as f64 / 1024.0,
                    )
                );
            }
        }
        Err(e) => eprintln!("{}", crate::messages::error::file_save_failed(&e)),
    }
}

/// Prints the RWD block ASCII logo banner.
fn print_logo_banner() {
    let version = env!("CARGO_PKG_VERSION");
    println!();
    println!("{CYAN}  ██████  ██     ██ ██████{RESET}");
    println!("{CYAN}  ██   ██ ██     ██ ██   ██{RESET}");
    println!(
        "{CYAN}  ██████  ██  █  ██ ██   ██{RESET}     {DIM}rewind your day  v{version}{RESET}"
    );
    println!("{CYAN}  ██   ██ ██ ███ ██ ██   ██{RESET}");
    println!("{CYAN}  ██   ██  ███ ███  ██████{RESET}");
}

/// Prints date and session summary as a Unicode box table.
/// Box width is dynamically sized to the longest content line.
fn print_info_box(
    date: chrono::NaiveDate,
    claude_summaries: Option<&[parser::claude::SessionSummary]>,
    claude_entries: &[parser::claude::LogEntry],
    codex_sessions: &[CodexCollectedSession],
) {
    // Build rows as (color_kind, text) pairs. "sep" means separator line.
    let date_str = format!("{date}");

    let mut rows: Vec<(&str, String)> = Vec::new();
    rows.push(("plain", date_str));

    // Claude Code section
    if let Some(summaries) = claude_summaries {
        rows.push(("sep", String::new()));
        rows.push(("blue", "Claude Code".to_string()));

        let earliest = claude_earliest_time(claude_entries);
        let latest = claude_latest_time(claude_entries);
        let time_range = format_time_range(earliest, latest);
        rows.push(("plain", time_range));

        let (total_in, total_out) = claude_total_tokens(summaries);
        rows.push((
            "plain",
            crate::messages::display::session_count_with_tokens(
                summaries.len(),
                &format_number(total_in),
                &format_number(total_out),
            ),
        ));
    }

    // Codex section
    rows.push(("sep", String::new()));
    rows.push(("yellow", "Codex".to_string()));
    if !codex_sessions.is_empty() {
        let earliest = codex_earliest_time(codex_sessions);
        let latest = codex_latest_time(codex_sessions);
        let time_range = format_time_range(earliest, latest);
        rows.push(("plain", time_range));
        rows.push((
            "plain",
            crate::messages::display::session_count(codex_sessions.len()),
        ));
    } else {
        rows.push(("plain", crate::messages::display::NO_SESSIONS.to_string()));
    }

    // Determine box width from the longest content line (CJK chars count as 2).
    let content_max = rows
        .iter()
        .filter(|(kind, _)| *kind != "sep")
        .map(|(_, text)| unicode_display_width(text))
        .max()
        .unwrap_or(20);
    let w = content_max + 4;

    let line = "─".repeat(w);
    println!("\n  ┌{line}┐");

    for (kind, text) in &rows {
        match *kind {
            "sep" => println!("  ├{line}┤"),
            "blue" => {
                let pad = w - 2 - unicode_display_width(text);
                println!("  │  {BRIGHT_BLUE}{text}{RESET}{:pad$}│", "");
            }
            "yellow" => {
                let pad = w - 2 - unicode_display_width(text);
                println!("  │  {YELLOW}{text}{RESET}{:pad$}│", "");
            }
            _ => {
                let pad = w - 2 - unicode_display_width(text);
                println!("  │  {text}{:pad$}│", "");
            }
        }
    }

    println!("  └{line}┘");
}

/// Returns the terminal width by querying `stty size` via /dev/tty.
fn terminal_width() -> usize {
    if let Ok(tty) = std::fs::File::open("/dev/tty")
        && let Ok(output) = std::process::Command::new("stty")
            .arg("size")
            .stdin(tty)
            .output()
    {
        let s = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = s.split_whitespace().collect();
        if let Some(cols) = parts.get(1).and_then(|c| c.parse::<usize>().ok())
            && cols > 0
        {
            return cols;
        }
    }
    80
}

/// Returns the display width of a string (ASCII = 1, CJK = 2).
fn unicode_display_width(s: &str) -> usize {
    s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum()
}

/// Prints analysis results in a full-box style per source.
fn print_insights(source_name: &str, analysis: &analyzer::AnalysisResult) {
    let term_w = terminal_width();
    let header_used = 5 + unicode_display_width(source_name) + 1;
    let line_len = if term_w > header_used {
        term_w - header_used
    } else {
        20
    };
    let line = "─".repeat(line_len);
    println!("\n{CYAN}  ┌─ {source_name} {line}{RESET}");

    for session in &analysis.sessions {
        // Show only the first 8 chars of session ID for readability
        let id_short = if session.session_id.len() >= 8 {
            &session.session_id[..8]
        } else {
            &session.session_id
        };
        println!("\n{BRIGHT_BLUE}  ▸ Session: {id_short}{RESET}");
        println!(
            "{}",
            crate::messages::display::summary_line(&session.work_summary)
        );

        if !session.decisions.is_empty() {
            println!(
                "\n  {YELLOW}{}{RESET}",
                crate::messages::display::DECISIONS_LABEL
            );
            for d in &session.decisions {
                println!("  • {}", d.what);
                println!("    {DIM}→ {}{RESET}", d.why);
            }
        }

        if !session.curiosities.is_empty() {
            println!(
                "\n  {YELLOW}{}{RESET}",
                crate::messages::display::CURIOSITIES_LABEL
            );
            for c in &session.curiosities {
                println!("  • {c}");
            }
        }

        if !session.corrections.is_empty() {
            println!(
                "\n  {YELLOW}{}{RESET}",
                crate::messages::display::CORRECTIONS_LABEL
            );
            for c in &session.corrections {
                println!("  {RED}\u{2717} {}{RESET}", c.model_said);
                println!("  {GREEN}\u{2713} {}{RESET}", c.user_corrected);
            }
        }
    }

    let bottom_line = "─".repeat(if term_w > 2 { term_w - 2 } else { 20 });
    println!("\n{CYAN}  └{bottom_line}{RESET}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn test_config(
        codex_roots: Option<Vec<String>>,
        claude_roots: Option<Vec<String>>,
    ) -> config::Config {
        config::Config {
            llm: config::LlmConfig {
                provider: "codex".to_string(),
                api_key: String::new(),
                codex_model: None,
                codex_reasoning_effort: None,
            },
            output: config::OutputConfig {
                path: "/tmp".to_string(),
            },
            redactor: None,
            lang: Some(config::Lang::En),
            input: Some(config::InputConfig {
                codex_roots,
                claude_roots,
            }),
        }
    }

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ))
    }

    #[test]
    fn test_collect_claude_entries_dedupes_across_roots() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 4, 11).expect("date");
        let base = unique_temp_dir("rwd_test_claude_multi_root");
        let root_a = base.join("claude-a");
        let root_b = base.join("claude-b");
        let project_a = root_a.join("project-a");
        let project_b = root_b.join("project-b");
        std::fs::create_dir_all(&project_a).expect("project a");
        std::fs::create_dir_all(&project_b).expect("project b");

        let line = r#"{"type":"user","sessionId":"claude-session-1","timestamp":"2026-04-11T12:00:00Z","uuid":"same-entry"}"#;
        let file_a = project_a.join("session-a.jsonl");
        let file_b = project_b.join("session-b.jsonl");
        std::fs::write(&file_a, format!("{line}\n")).expect("write a");
        std::fs::write(&file_b, format!("{line}\n")).expect("write b");

        let cfg = test_config(
            None,
            Some(vec![
                root_a.to_string_lossy().to_string(),
                root_b.to_string_lossy().to_string(),
            ]),
        );
        let (entries, stats) = collect_claude_entries_with_stats(date, Some(&cfg));

        assert!(stats.roots.starts_with(&[root_a.clone(), root_b.clone()]));
        assert_eq!(entries.len(), 1);

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_collect_codex_sessions_merges_by_session_id_across_roots() {
        let date = chrono::NaiveDate::from_ymd_opt(2099, 1, 1).expect("date");
        let base = unique_temp_dir("rwd_test_codex_multi_root");
        let root_a = base.join("codex-a");
        let root_b = base.join("codex-b");
        let day_dir_a = root_a.join("2099").join("01").join("01");
        let day_dir_b = root_b.join("2099").join("01").join("01");
        std::fs::create_dir_all(&day_dir_a).expect("day a");
        std::fs::create_dir_all(&day_dir_b).expect("day b");

        let mut file_a = std::fs::File::create(day_dir_a.join("rollout-a.jsonl")).expect("file a");
        writeln!(
            file_a,
            r#"{{"timestamp":"2099-01-01T12:00:00Z","type":"session_meta","payload":{{"id":"codex-s1","cwd":"/p","model_provider":"openai"}}}}"#
        )
        .expect("meta a");
        writeln!(
            file_a,
            r#"{{"timestamp":"2099-01-01T12:01:00Z","type":"event_msg","payload":{{"type":"user_message","message":"hello"}}}}"#
        )
        .expect("user a");

        let mut file_b = std::fs::File::create(day_dir_b.join("rollout-b.jsonl")).expect("file b");
        writeln!(
            file_b,
            r#"{{"timestamp":"2099-01-01T12:00:00Z","type":"session_meta","payload":{{"id":"codex-s1","cwd":"/p","model_provider":"openai"}}}}"#
        )
        .expect("meta b");
        writeln!(
            file_b,
            r#"{{"timestamp":"2099-01-01T12:01:00Z","type":"event_msg","payload":{{"type":"user_message","message":"hello"}}}}"#
        )
        .expect("user duplicate");
        writeln!(
            file_b,
            r#"{{"timestamp":"2099-01-01T12:02:00Z","type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"done"}}]}}}}"#
        )
        .expect("assistant b");

        let cfg = test_config(
            Some(vec![
                root_a.to_string_lossy().to_string(),
                root_b.to_string_lossy().to_string(),
            ]),
            None,
        );
        let (sessions, roots) = collect_codex_sessions(date, Some(&cfg));

        assert!(roots.starts_with(&[root_a.clone(), root_b.clone()]));
        assert_eq!(sessions.len(), 1);
        let (summary, _) = &sessions[0];
        assert_eq!(summary.session_id, "codex-s1");
        assert_eq!(summary.user_count, 1);
        assert_eq!(summary.assistant_count, 1);

        std::fs::remove_dir_all(&base).ok();
    }
}
