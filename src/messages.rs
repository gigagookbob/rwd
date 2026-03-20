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
    pub const SELECT_PROVIDER: &str =
        "Select LLM provider (anthropic/openai) [anthropic]: ";
    pub const ENTER_API_KEY_ANTHROPIC: &str = "Enter Anthropic API key: ";
    pub const ENTER_API_KEY_OPENAI: &str = "Enter OpenAI API key: ";
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
        format!("Unsupported provider: {name}")
    }

    pub fn config_saved(path: &dyn std::fmt::Display) -> String {
        format!("Config saved: {path}")
    }
}

/// Messages for `rwd config` flow.
pub mod config {
    pub const NO_CONFIG: &str =
        "No config found. Run `rwd init` first.";
    pub const SELECT_SETTING: &str = "Select a setting to change";
    pub const LLM_PROVIDER: &str = "LLM Provider";
    pub const OUTPUT_PATH: &str = "Markdown output path";
    pub const REDACTOR: &str = "Sensitive data masking";
    pub const LANGUAGE: &str = "Language";
    pub const EXIT: &str = "Exit";
    pub const NAV_HINT: &str =
        "  \u{2191}\u{2193} Navigate \u{00B7} Enter Select \u{00B7} Esc Exit";
    pub const NO_CHANGE: &str = "  No change";
    pub const NEW_API_KEY: &str = "  New API key: ";
    pub const CONFIRM_API_KEY: &str = "Change API key?";
    pub const USAGE: &str =
        "Usage: `rwd config` (interactive) or `rwd config <key> <value>`";

    pub fn config_saved(path: &dyn std::fmt::Display) -> String {
        format!("Config saved. {path}")
    }

    pub fn changed(old: &str, new: &str) -> String {
        format!("  \u{2713} Changed {old} \u{2192} {new}")
    }

    pub fn output_path_changed(value: &str) -> String {
        format!("Output path changed: {value}")
    }

    pub fn provider_changed(value: &str) -> String {
        format!("LLM provider changed: {value}")
    }

    pub fn api_key_changed(masked: &str) -> String {
        format!("API key changed: {masked}")
    }

    pub fn unsupported_provider(name: &str) -> String {
        format!(
            "Unsupported provider: '{name}'. Available: anthropic, openai"
        )
    }

    pub fn unknown_key(key: &str) -> String {
        format!(
            "Unknown config key: '{key}'. Available: output-path, provider, api-key"
        )
    }
}

/// Error messages used across the application.
pub mod error {
    pub const NO_CONFIG: &str =
        "No config found. Run `rwd init` first.";
    pub const NO_CACHE: &str = "No cache found. Running today analysis first...";
    pub const NO_CACHE_AFTER_ANALYSIS: &str =
        "No cache found even after analysis.";
    pub const NO_SESSIONS: &str = "No sessions to summarize.";
    pub const HOME_DIR_NOT_FOUND: &str = "Home directory not found";
    pub const RELEASE_TAG_NOT_FOUND: &str =
        "Release tag not found";
    pub const ALL_SESSIONS_FAILED: &str =
        "All sessions failed analysis.";
    pub const NO_CONVERSATION_CLAUDE: &str =
        "No conversation found in log entry.";
    pub const NO_CONVERSATION_CODEX: &str =
        "No conversation found in Codex log.";

    pub fn init_failed(e: &dyn std::fmt::Display) -> String {
        format!("Initialization failed: {e}")
    }

    pub fn config_failed(e: &dyn std::fmt::Display) -> String {
        format!("Config change failed: {e}")
    }

    pub fn update_failed(e: &dyn std::fmt::Display) -> String {
        format!("Update failed: {e}")
    }

    pub fn unsupported_provider(name: &str) -> String {
        format!("Unsupported provider: {name}")
    }

    pub fn unsupported_platform(os: &str, arch: &str) -> String {
        format!("Unsupported platform: {os}-{arch}")
    }

    pub fn api_request_failed(status: &dyn std::fmt::Display, body: &str) -> String {
        format!("API request failed ({status}): {body}")
    }

    pub fn openai_api_request_failed(
        status: &dyn std::fmt::Display,
        body: &str,
    ) -> String {
        format!("OpenAI API request failed ({status}): {body}")
    }

    pub const API_NO_TEXT_BLOCK: &str = "No text block in API response";
    pub const OPENAI_EMPTY_CHOICES: &str =
        "OpenAI response has empty choices";

    /// Substring used by `analyzer/mod.rs` to detect JSON parse errors for retry logic.
    /// Must be a prefix of the message produced by `json_parse_failed()`.
    pub const JSON_PARSE_FAILED_MARKER: &str = "LLM response JSON parse failed";

    pub fn json_parse_failed(
        e: &dyn std::fmt::Display,
        preview: &str,
    ) -> String {
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

    pub const EXTRACT_FAILED: &str = "Extraction failed";
    pub const BINARY_NOT_FOUND: &str =
        "Update binary not found";
    pub const BINARY_REPLACE_FAILED: &str = "Binary replacement failed";
    pub const ADMIN_REQUIRED: &str = "Administrator privileges required.";
}

/// Status and progress messages.
pub mod status {
    pub const CACHE_USED: &str =
        "Using cached analysis. (no entry changes)";
    pub const REWIND_DONE: &str = "Today's daily rewind is ready!";
    pub const SUMMARY_GENERATING: &str = "Generating development progress summary...";
    pub const SUMMARY_HEADER: &str = "=== Development Progress ===";
    pub const SLACK_GENERATING: &str = "Generating Slack message...";
    pub const COPIED_TO_CLIPBOARD: &str = "Copied to clipboard.";
    pub const NO_SESSIONS_CODEX: &str = "No sessions";
    pub const PROBING_RATE_LIMITS: &str = "Checking API rate limits...";
    pub const ANALYZING_INSIGHT: &str = "Analyzing insights...";

    pub fn analyzing(provider: &str) -> String {
        format!("Analyzing insights via {provider} API...")
    }

    pub fn redacted(count: usize, summary: &str) -> String {
        format!("{count} sensitive item(s) masked ({summary})")
    }

    pub fn cache_stale(cached_total: usize, current_total: usize) -> String {
        format!(
            "\u{26A0} Cache is stale. (cached: {cached_total}, current: {current_total})"
        )
    }

    pub const CACHE_STALE_HINT: &str =
        "  Run `rwd today` first for up-to-date results.";

    pub fn markdown_saved(path: &dyn std::fmt::Display) -> String {
        format!("Markdown saved: {path}")
    }

    pub fn session_count(count: usize) -> String {
        format!("Sessions: {count}")
    }

    pub fn session_count_with_tokens(
        count: usize,
        total_in: &str,
        total_out: &str,
    ) -> String {
        format!("Sessions: {count}  in {total_in}  out {total_out}")
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

    pub fn step_reanalyzing(
        i: usize,
        total: usize,
        session_id: &str,
    ) -> String {
        format!("[{i}/{total}] Re-analyzing {session_id}...")
    }

    pub fn step_done(i: usize, total: usize) -> String {
        format!("\u{2713} [{i}/{total}] Done")
    }

    pub fn step_retry_success(i: usize, total: usize) -> String {
        format!("\u{2713} [{i}/{total}] Retry succeeded")
    }

    pub fn step_reanalysis_success(i: usize, total: usize) -> String {
        format!("\u{2713} [{i}/{total}] Re-analysis succeeded")
    }

    pub fn step_skip(
        i: usize,
        total: usize,
        session_id: &str,
        reason: &dyn std::fmt::Display,
    ) -> String {
        format!("\u{26A0} [{i}/{total}] {session_id} skipped: {reason}")
    }

    pub fn step_skip_retry(
        i: usize,
        total: usize,
        session_id: &str,
        reason: &dyn std::fmt::Display,
    ) -> String {
        format!("\u{26A0} [{i}/{total}] {session_id} skipped (retry failed): {reason}")
    }

    pub fn step_skip_reanalysis(
        i: usize,
        total: usize,
        session_id: &str,
        reason: &dyn std::fmt::Display,
    ) -> String {
        format!(
            "\u{26A0} [{i}/{total}] {session_id} skipped (re-analysis failed): {reason}"
        )
    }

    pub fn chunk_summarizing(i: usize, total: usize) -> String {
        format!("Summarizing chunk {i}/{total}...")
    }

    pub fn chunk_done(i: usize, total: usize) -> String {
        format!("    \u{2713} Chunk {i}/{total} done")
    }

    pub fn plan_single_shot(estimated_tokens: u64) -> String {
        format!("\u{2713} Analyzing full log in one pass (est. {estimated_tokens} tokens)")
    }

    pub fn plan_multi_step(steps: usize, total_tokens: u64) -> String {
        format!(
            "\u{2713} Analyzing {steps} sessions sequentially (est. {total_tokens} tokens)"
        )
    }

    pub fn plan_step_direct(
        session_id: &str,
        tokens: u64,
    ) -> String {
        format!("  \u{2022} {session_id}: {tokens} tokens \u{2192} direct analysis")
    }

    pub fn plan_step_summarize(
        session_id: &str,
        tokens: u64,
        chunks: usize,
    ) -> String {
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
}

/// Labels for `rwd today --verbose` output sections.
pub mod display {
    pub const SUMMARY_LABEL: &str = "Summary";
    pub const DECISIONS_LABEL: &str = "Key Decisions";
    pub const CURIOSITIES_LABEL: &str = "Questions & Curiosities";
    pub const CORRECTIONS_LABEL: &str = "Model Corrections";

    pub fn summary_line(work_summary: &str) -> String {
        format!("  Summary: {}", work_summary)
    }

    pub fn session_count_with_tokens(
        count: usize,
        total_in: &str,
        total_out: &str,
    ) -> String {
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
    pub const VERIFY_SKIPPED_CLIENT: &str =
        "  API key verification skipped (HTTP client creation failed)";
    pub const VERIFY_SKIPPED_NETWORK: &str =
        "  API key verification skipped (network error)";

    pub fn key_invalid(status: u16) -> String {
        format!(
            "  \u{26A0} API key is invalid ({status}). Please check your key."
        )
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

    pub fn update_complete(version: &str) -> String {
        format!("rwd v{version} update complete!")
    }

    pub fn new_version_available(latest: &str, current: &str) -> String {
        format!("New version available: v{latest} (current: v{current})")
    }

    pub const UPDATE_HINT: &str = "Update: rwd update";
}

/// Language configuration messages (placeholder for future i18n).
pub mod lang {
    pub const SELECT: &str = "Select language";
    pub const NOT_CONFIGURED: &str = "Language not configured";

    pub fn saved(lang: &str) -> String {
        format!("Language saved: {lang}")
    }

    pub fn unsupported(lang: &str) -> String {
        format!("Unsupported language: {lang}")
    }
}
