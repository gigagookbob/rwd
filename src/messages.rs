// src/messages.rs
//
// Centralized user-facing string constants and parameterized message functions.
// Sub-grouped by domain for readability.
//
// Design:
// - Non-parameterized messages: `&str` constants
// - Parameterized messages: functions returning `String`
//   (Rust's format!() requires a string literal as first arg, so constants
//    with placeholders cannot be used with format!() directly)

/// Messages for `rwd init` flow.
pub mod init {
    pub const SELECT_PROVIDER: &str = "Select LLM provider (anthropic/openai/codex) [anthropic]: ";
    pub const ENTER_API_KEY_ANTHROPIC: &str = "Enter Anthropic API key: ";
    pub const ENTER_API_KEY_OPENAI: &str = "Enter OpenAI API key: ";
    pub const CODEX_LOGIN_AUTH: &str =
        "Codex provider uses `codex login` session authentication (API key is not used).";
    pub const API_KEY_EMPTY: &str = "API key is empty.";

    pub fn api_key_input_failed(e: &dyn std::fmt::Display) -> String {
        format!("API key input failed: {e}")
    }

    pub fn api_key_set(masked: &str) -> String {
        format!("API key set: {masked}")
    }

    pub fn output_path_prompt(default: &dyn std::fmt::Display) -> String {
        format!("Markdown output path [{default}]: ")
    }

    pub fn output_path_set(path: &dyn std::fmt::Display) -> String {
        format!("Output path: {path}")
    }

    pub fn unsupported_provider(name: &str) -> String {
        format!("Unsupported provider: {name}. Available: anthropic, openai, codex")
    }

    pub fn config_saved(path: &dyn std::fmt::Display) -> String {
        format!("Config saved: {path}")
    }
}

/// Messages for `rwd config` flow.
pub mod config {
    pub const NO_CONFIG: &str = "No config found. Run `rwd init` first.";
    pub const SELECT_SETTING: &str = "Select a setting to change";
    pub const LLM_PROVIDER: &str = "LLM Provider";
    pub const OUTPUT_PATH: &str = "Markdown output path";
    pub const REDACTOR: &str = "Sensitive data masking";
    pub const LANGUAGE: &str = "Language";
    pub const EXIT: &str = "Exit";
    pub const NAV_HINT: &str =
        "  \u{2191}\u{2193} Navigate \u{00B7} Enter Select \u{00B7} Esc Exit";
    pub const NO_CHANGE: &str = "  No change";
    pub const USAGE: &str = "Usage: `rwd config` (interactive) or `rwd config <key> <value>` (keys: output-path, provider, api-key, openai-api-key, anthropic-api-key, codex-model, codex-reasoning, claude-include-automated)";

    pub fn config_saved(path: &dyn std::fmt::Display) -> String {
        format!("Config saved. {path}")
    }

    pub fn changed(old: &str, new: &str) -> String {
        format!("  \u{2713} Changed {old} \u{2192} {new}")
    }

    pub fn output_path_changed(value: &str) -> String {
        format!("Output path changed: {value}")
    }

    pub fn provider_changed(value: &str, auth_method: &str) -> String {
        format!("LLM provider changed: {value} (auth: {auth_method})")
    }

    pub fn api_key_changed(masked: &str) -> String {
        format!("API key changed: {masked}")
    }

    pub fn new_provider_api_key(provider: &str) -> String {
        format!("  New {provider} API key: ")
    }

    pub fn confirm_provider_api_key(provider: &str) -> String {
        format!("Change {provider} API key?")
    }

    pub fn provider_api_key_changed(provider: &str, masked: &str) -> String {
        format!("{provider} API key changed: {masked}")
    }

    pub fn unsupported_provider(name: &str) -> String {
        format!("Unsupported provider: '{name}'. Available: anthropic, openai, codex")
    }

    pub fn unknown_key(key: &str) -> String {
        format!(
            "Unknown config key: '{key}'. Available: output-path, provider, api-key, openai-api-key, anthropic-api-key, codex-model, codex-reasoning, claude-include-automated"
        )
    }

    pub fn codex_model_changed(value: &str) -> String {
        format!("Codex model changed: {value}")
    }

    pub fn codex_reasoning_changed(value: &str) -> String {
        format!("Codex reasoning effort changed: {value}")
    }

    pub fn claude_include_automated_changed(enabled: bool) -> String {
        if enabled {
            "Claude automated sessions will be included in `rwd today`.".to_string()
        } else {
            "Claude automated sessions will be excluded from `rwd today`.".to_string()
        }
    }

    pub fn provider_now_uses(provider: &str, auth_method: &str) -> String {
        format!("  Provider `{provider}` now uses: {auth_method}")
    }

    pub fn switched_to_codex_keeps_api_key(api_key_state: &str) -> String {
        format!(
            "  Switched to Codex login auth. Stored API key is kept ({api_key_state}) and unused."
        )
    }

    pub fn provider_requires_api_key(provider: &str) -> String {
        format!(
            "  `{provider}` uses API key auth. Provider key is not set — set one with `rwd config api-key <key>`."
        )
    }

    pub fn api_key_unused_for_provider(provider: &str) -> String {
        format!(
            "Current provider `{provider}` does not use API key auth. Use `rwd config provider openai|anthropic` first, or set `openai-api-key` / `anthropic-api-key` directly."
        )
    }

    pub fn unsupported_reasoning_effort(value: &str) -> String {
        format!(
            "Unsupported codex reasoning effort: '{value}'. Available: low, medium, high, xhigh, default"
        )
    }
}

/// Messages for `rwd reset`.
pub mod reset {
    pub const CONFIRM: &str = "This will remove rwd config/cache files. Continue?";
    pub const CANCELLED: &str = "Reset cancelled.";
    pub const NOTHING_TO_RESET: &str = "Nothing to reset.";
    pub const DRY_RUN_HEADER: &str = "Reset dry-run (no files removed):";
    pub const REMOVED_HEADER: &str = "Reset completed. Removed:";
    pub const NEXT_STEP: &str = "Next: run `rwd init` to set up configuration again.";

    pub fn item(path: &dyn std::fmt::Display) -> String {
        format!(" - {path}")
    }
}

/// Messages for `rwd auth status`.
pub mod auth {
    pub fn provider(value: &str) -> String {
        format!("Provider: {value}")
    }

    pub fn auth_method(value: &str) -> String {
        format!("Auth method: {value}")
    }

    pub fn provider_api_key(provider: &str, state: &str, detail: &str) -> String {
        format!("{provider} API key: {state} ({detail})")
    }

    pub fn codex_login_status(state: &str) -> String {
        format!("Codex login: {state}")
    }

    pub fn provider_missing_api_key(provider: &str, hint: &str) -> String {
        format!("{provider} API key status: missing ({hint})")
    }
}

/// Error messages used across the application.
pub mod error {
    fn sanitize_api_error_body(body: &str) -> String {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return "<empty body>".to_string();
        }

        let (redacted, _) = crate::redactor::redact_text(trimmed);
        const MAX_BODY_CHARS: usize = 240;
        let mut out: String = redacted.chars().take(MAX_BODY_CHARS).collect();
        if redacted.chars().count() > MAX_BODY_CHARS {
            out.push_str("...(truncated)");
        }
        out
    }

    pub const NO_CONFIG: &str = "No config found. Run `rwd init` first.";
    pub const NO_CACHE: &str = "No cache found. Running today analysis first...";
    pub const NO_CACHE_AFTER_ANALYSIS: &str = "No cache found even after analysis.";
    pub const NO_SESSIONS: &str = "No sessions to summarize.";
    pub const HOME_DIR_NOT_FOUND: &str = "Home directory not found";
    pub const RELEASE_TAG_NOT_FOUND: &str = "Release tag not found";
    pub const ALL_SESSIONS_FAILED: &str = "All sessions failed analysis.";
    pub const NO_CONVERSATION_CLAUDE: &str = "No conversation found in log entry.";
    pub const NO_CONVERSATION_CODEX: &str = "No conversation found in Codex log.";

    pub fn init_failed(e: &dyn std::fmt::Display) -> String {
        format!("Initialization failed: {e}")
    }

    pub fn config_failed(e: &dyn std::fmt::Display) -> String {
        format!("Config change failed: {e}")
    }

    pub fn update_failed(e: &dyn std::fmt::Display) -> String {
        format!("Update failed: {e}")
    }

    pub fn unsupported_platform(os: &str, arch: &str) -> String {
        format!("Unsupported platform: {os}-{arch}")
    }

    pub fn unsupported_provider_in_config(provider: &str) -> String {
        format!("Unsupported provider in config: '{provider}'. Available: anthropic, openai, codex")
    }

    pub fn api_request_failed(status: &dyn std::fmt::Display, body: &str) -> String {
        let safe_body = sanitize_api_error_body(body);
        format!("API request failed ({status}): {safe_body}")
    }

    pub fn openai_api_request_failed(status: &dyn std::fmt::Display, body: &str) -> String {
        let safe_body = sanitize_api_error_body(body);
        format!("OpenAI API request failed ({status}): {safe_body}")
    }

    pub const API_NO_TEXT_BLOCK: &str = "No text block in API response";
    pub const OPENAI_EMPTY_CHOICES: &str = "OpenAI response has empty choices";

    /// Substring used by `analyzer/mod.rs` to detect JSON parse errors for retry logic.
    /// Must be a prefix of the message produced by `json_parse_failed()`.
    pub const JSON_PARSE_FAILED_MARKER: &str = "LLM response JSON parse failed";

    pub fn json_parse_failed(e: &dyn std::fmt::Display, preview: &str) -> String {
        format!(
            "LLM response JSON parse failed: {e}\nResponse preview (first 200 chars): {preview}"
        )
    }

    pub fn cache_save_failed(e: &dyn std::fmt::Display) -> String {
        format!("Cache save failed: {e}")
    }

    pub fn vault_path_load_failed(e: &dyn std::fmt::Display) -> String {
        format!("Vault path load failed: {e}")
    }

    pub fn daily_markdown_not_found(path: &dyn std::fmt::Display) -> String {
        format!("Daily Markdown file not found: {path}")
    }

    pub fn file_read_failed(e: &dyn std::fmt::Display) -> String {
        format!("File read failed: {e}")
    }

    pub fn file_save_failed(e: &dyn std::fmt::Display) -> String {
        format!("File save failed: {e}")
    }

    pub fn download_failed(status: u16) -> String {
        format!(
            "Download failed (HTTP {status}). Release assets may not be ready yet — try again shortly."
        )
    }
    pub const EXTRACT_FAILED: &str = "Extraction failed";
    pub const BINARY_NOT_FOUND: &str = "Update binary not found";
    #[cfg(unix)]
    pub const BINARY_REPLACE_FAILED: &str = "Binary replacement failed";
    #[cfg(unix)]
    pub const ADMIN_REQUIRED: &str = "Administrator privileges required.";
}

/// Status and progress messages.
pub mod status {
    pub const CACHE_USED: &str = "Using cached analysis. (no entry changes)";
    pub const CACHE_BYPASSED: &str = "Ignoring cache. (--no-cache)";
    pub const REWIND_DONE: &str = "Today's daily rewind is ready!";
    pub const SUMMARY_GENERATING: &str = "Generating development progress summary...";
    pub const SUMMARY_HEADER: &str = "=== Development Progress ===";
    pub const SLACK_GENERATING: &str = "Generating Slack message...";
    pub const COPIED_TO_CLIPBOARD: &str = "Copied to clipboard.";
    pub const PROBING_RATE_LIMITS: &str = "Checking API rate limits...";

    pub fn analyzing(provider: &str) -> String {
        format!("Analyzing insights via {provider}...")
    }

    pub fn redacted(count: usize, summary: &str) -> String {
        format!("{count} sensitive item(s) masked ({summary})")
    }

    pub fn cache_stale(cached_total: usize, current_total: usize) -> String {
        format!("\u{26A0} Cache is stale. (cached: {cached_total}, current: {current_total})")
    }

    pub const CACHE_STALE_TIMEZONE: &str =
        "\u{26A0} Cache is stale. (timezone/day boundary changed)";

    pub const CACHE_STALE_HINT: &str = "  Run `rwd today` first for up-to-date results.";

    pub fn markdown_saved(path: &dyn std::fmt::Display) -> String {
        format!("Markdown saved: {path}")
    }

    pub fn countdown_waiting(remaining: u64) -> String {
        format!("Waiting for next request... ({remaining}s)")
    }

    pub fn step_analyzing(i: usize, total: usize, session_id: &str) -> String {
        format!("[{i}/{total}] Analyzing {session_id}...")
    }

    pub fn step_retrying(i: usize, total: usize, session_id: &str) -> String {
        format!("[{i}/{total}] Retrying {session_id}...")
    }

    pub fn step_reanalyzing(i: usize, total: usize, session_id: &str) -> String {
        format!("[{i}/{total}] Re-analyzing {session_id}...")
    }

    pub fn step_done(i: usize, total: usize) -> String {
        format!("\u{2713} [{i}/{total}] Done")
    }

    pub fn chunk_summarizing(i: usize, total: usize) -> String {
        format!("Summarizing chunk {i}/{total}...")
    }

    pub fn chunk_done(i: usize, total: usize) -> String {
        format!("    \u{2713} Chunk {i}/{total} done")
    }

    pub fn plan_multi_step(steps: usize, total_tokens: u64) -> String {
        format!("\u{2713} Analyzing {steps} sessions (est. {total_tokens} tokens)")
    }

    pub fn plan_step_direct(session_id: &str, tokens: u64) -> String {
        format!("  \u{2022} {session_id}: {tokens} tokens \u{2192} direct analysis")
    }

    pub fn plan_step_summarize(session_id: &str, tokens: u64, chunks: usize) -> String {
        format!(
            "  \u{2022} {session_id}: {tokens} tokens \u{2192} summarize then analyze ({chunks} chunks)"
        )
    }

    pub fn rate_limit_ok(itpm: u64, otpm: u64, rpm: u64) -> String {
        format!("\u{2713} ITPM: {itpm} | OTPM: {otpm} | RPM: {rpm}")
    }

    pub fn rate_limit_fallback(itpm: u64, otpm: u64, rpm: u64) -> String {
        format!(
            "\u{26A0} Rate limit check failed, proceeding with defaults. \
             (ITPM: {itpm} | OTPM: {otpm} | RPM: {rpm})"
        )
    }

    pub fn rate_limit_probe_skipped(provider: &str, itpm: u64, otpm: u64, rpm: u64) -> String {
        format!(
            "\u{2139} Rate limit check is not supported for {provider}; using defaults. \
             (ITPM: {itpm} | OTPM: {otpm} | RPM: {rpm})"
        )
    }
}

/// Labels for `rwd today --verbose` output sections.
pub mod display {
    pub const DECISIONS_LABEL: &str = "Key Decisions";
    pub const CURIOSITIES_LABEL: &str = "Questions & Curiosities";
    pub const CORRECTIONS_LABEL: &str = "Model Corrections";

    pub fn summary_line(work_summary: &str) -> String {
        format!("  Summary: {}", work_summary)
    }

    pub fn session_count_with_tokens(count: usize, total_in: &str, total_out: &str) -> String {
        format!("Sessions: {count}  in {total_in}  out {total_out}")
    }

    pub fn session_count(count: usize) -> String {
        format!("Sessions: {count}")
    }

    pub const NO_SESSIONS: &str = "No sessions";
}

/// Markdown section headers used in `src/output/markdown.rs`.
/// English translations of the current Korean headers — Task 2 will
/// replace the Korean literals with these constants.
pub mod markdown {
    pub const WORK_SUMMARY_HEADER: &str = "### Work Summary";
    pub const DECISIONS_HEADER: &str = "### Key Decisions";
    pub const CURIOSITIES_HEADER: &str = "### Questions & Curiosities";
    pub const CORRECTIONS_HEADER: &str = "### Model Errors & Corrections";
    pub const CORRECTION_MODEL: &str = "**Model**";
    pub const CORRECTION_FIX: &str = "**Fix**";
    /// Section header for development progress in daily Markdown files.
    pub const PROGRESS_SECTION_HEADER: &str = "## Development Progress";
}

/// API key verification messages.
pub mod verify {
    pub const VERIFYING_KEY: &str = "  Verifying API key...";
    pub const KEY_VERIFIED: &str = "  \u{2713} API key verified";
    pub const VERIFYING_CODEX_LOGIN: &str = "  Verifying Codex login...";
    pub const CODEX_LOGIN_VERIFIED: &str = "  \u{2713} Codex login verified";
    pub const VERIFY_SKIPPED_CLIENT: &str =
        "  API key verification skipped (HTTP client creation failed)";
    pub const VERIFY_SKIPPED_NETWORK: &str = "  API key verification skipped (network error)";
    pub const CODEX_LOGIN_CHECK_FAILED: &str =
        "  Codex login check failed (command execution error)";
    pub const CODEX_NOT_LOGGED_IN: &str = "  \u{26A0} Codex login not found. Run `codex login`.";

    pub fn key_invalid(status: u16) -> String {
        format!("  \u{26A0} API key is invalid ({status}). Please check your key.")
    }
}

/// Messages for `rwd update` flow.
pub mod update {
    pub fn already_latest(version: &str) -> String {
        format!("Already on the latest version: v{version}")
    }

    pub fn updating(current: &str, latest: &str) -> String {
        format!("Updating v{current} \u{2192} v{latest}...")
    }

    pub fn downloading(url: &str) -> String {
        format!("Downloading: {url}")
    }

    #[cfg(unix)]
    pub fn update_complete(version: &str) -> String {
        format!("rwd v{version} update complete!")
    }

    #[cfg(unix)]
    pub fn user_bin_update_complete(path: &dyn std::fmt::Display, version: &str) -> String {
        format!("rwd v{version} update complete! Installed to {path}")
    }

    #[cfg(unix)]
    pub const USER_BIN_PATH_HINT: &str =
        "Tip: ensure ~/.local/bin comes before system directories in PATH.";

    #[cfg(unix)]
    pub const DUPLICATE_BINARIES_FOUND: &str =
        "Multiple `rwd` binaries are visible in PATH. Keep one to avoid confusion.";

    #[cfg(unix)]
    pub fn active_binary(path: &dyn std::fmt::Display) -> String {
        format!("Active binary: {path}")
    }

    #[cfg(unix)]
    pub fn cleanup_duplicate(path: &dyn std::fmt::Display, command: &str) -> String {
        format!("Remove duplicate {path} with: {command}")
    }

    pub fn new_version_available(latest: &str, current: &str) -> String {
        format!("New version available: v{latest} (current: v{current})")
    }

    pub const UPDATE_HINT: &str = "Update: rwd update";
}

/// Language configuration messages (placeholder for future i18n).
pub mod lang {
    pub const SELECT: &str = "Language (en/ko) [default: en]: ";
    pub const NOT_CONFIGURED: &str =
        "Language not configured. Please select (en/ko) [default: en]: ";

    pub fn saved(lang: &str) -> String {
        format!("Language saved: {lang}")
    }

    pub fn unsupported(lang: &str) -> String {
        format!("Unsupported language: {lang}")
    }
}

/// Messages for background execution mode.
pub mod background {
    pub const ALREADY_RUNNING: &str = "Analysis is already running.";
    pub const NOTIFIED_WHEN_DONE: &str = "You'll be notified when it's done!";
    pub const NOTIFY_TITLE: &str = "rwd";
    pub const NOTIFY_SUCCESS: &str = "Your daily rewind is ready!";
    #[cfg(target_os = "macos")]
    pub const NOTIFY_SOUND: &str = "Blow";

    pub fn starting(pid: u32) -> String {
        format!("Starting analysis in background...  \x1b[2m(pid: {pid})\x1b[0m")
    }

    pub fn results_path(path: &dyn std::fmt::Display) -> String {
        format!("Results will be saved to: {path}")
    }

    pub fn notify_failure(log_path: &dyn std::fmt::Display) -> String {
        format!("Analysis failed. Check {log_path}")
    }
}

/// Messages for verbose (-v) diagnostic output.
pub mod verbose {
    pub fn discover_stats(projects: usize, files: usize, total: usize, today: usize) -> String {
        format!(
            "[discover] {projects} projects, {files} log files scanned \u{2192} {today} entries today (of {total} total)"
        )
    }

    pub fn used_roots(source: &str, roots: &str) -> String {
        format!("[discover] {source} roots: {roots}")
    }

    pub fn claude_session_mix(
        interactive: usize,
        automated: usize,
        include_automated: bool,
    ) -> String {
        if include_automated {
            format!(
                "[discover] Claude sessions: {interactive} interactive + {automated} automated (included)"
            )
        } else {
            format!(
                "[discover] Claude sessions: {interactive} interactive + {automated} automated (excluded)"
            )
        }
    }

    pub fn step_done_detail(
        i: usize,
        total: usize,
        session_id: &str,
        secs: f64,
        input: u64,
        output: u64,
    ) -> String {
        format!(
            "\u{2713} [{i}/{total}] {short} done in {secs:.1}s (input: {input} / output: {output})",
            short = if session_id.len() >= 8 {
                &session_id[..8]
            } else {
                session_id
            },
        )
    }

    pub fn cache_saved(path: &dyn std::fmt::Display, size_kb: f64) -> String {
        format!("Cache saved: {path} ({size_kb:.1} KB)")
    }

    pub fn markdown_file_size(path: &dyn std::fmt::Display, size_kb: f64) -> String {
        format!("Markdown size: {path} ({size_kb:.1} KB)")
    }
}

#[cfg(test)]
mod tests {
    use super::error;

    #[test]
    fn test_api_request_failed_masks_sensitive_body() {
        let body = r#"{"error":"invalid key sk-abcdefghijklmnopqrstuvwxyz1234"}"#;
        let msg = error::api_request_failed(&401, body);
        assert!(!msg.contains("sk-abcdefghijklmnopqrstuvwxyz1234"));
        assert!(msg.contains("[REDACTED:API_KEY]"));
    }

    #[test]
    fn test_api_request_failed_truncates_long_body() {
        let long_body = "x".repeat(400);
        let msg = error::openai_api_request_failed(&500, &long_body);
        assert!(msg.contains("...(truncated)"));
    }
}
