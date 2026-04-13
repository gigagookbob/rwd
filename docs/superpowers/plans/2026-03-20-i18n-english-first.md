# i18n English-First Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert all Korean content to English, externalize prompts to `.md` files with EN/KO support, and add `--lang` config/flag for LLM output language.

**Architecture:** Messages centralized in `src/messages.rs` (sub-grouped by domain), prompts externalized to `prompts/*.md` with `include_str!()`, language config via `Lang` enum in `Config` struct with migration prompt for existing users.

**Tech Stack:** Rust 2024 Edition, clap (CLI), serde/toml (config), include_str! (prompt embedding)

**Spec:** `docs/superpowers/specs/2026-03-20-i18n-english-first-design.md`

---

## File Structure

### New Files
- `src/messages.rs` — All user-facing string constants and parameterized message functions, sub-grouped by domain
- `prompts/system_en.md` — English system prompt for session analysis
- `prompts/system_ko.md` — Korean system prompt (current SYSTEM_PROMPT)
- `prompts/summary_en.md` — English summary prompt
- `prompts/summary_ko.md` — Korean summary prompt (current SUMMARY_PROMPT)
- `prompts/slack_en.md` — English slack prompt
- `prompts/slack_ko.md` — Korean slack prompt (current SLACK_PROMPT)
- `prompts/chunk_summarize_en.md` — English chunk summarization prompt
- `prompts/chunk_summarize_ko.md` — Korean chunk summarization prompt (current CHUNK_SUMMARIZE_PROMPT)

### Modified Files
- `src/main.rs` — Replace Korean strings with `messages::` references, add `--lang` flag handling
- `src/cli.rs` — Add `--lang` flag to subcommands, translate help text
- `src/config.rs` — Add `Lang` enum, `lang` field, migration prompt, interactive menu item
- `src/analyzer/provider.rs` — Replace const prompts with `include_str!()`, add `lang` parameter to `call_api`/`call_summary_api`/`call_slack_api`
- `src/analyzer/mod.rs` — Thread `lang` through orchestration functions
- `src/analyzer/insight.rs` — Translate comments, rename test functions
- `src/analyzer/openai.rs` — Translate comments
- `src/analyzer/anthropic.rs` — Translate comments
- `src/analyzer/planner.rs` — Translate comments, rename test functions
- `src/analyzer/prompt.rs` — Translate comments, rename test functions
- `src/analyzer/summarizer.rs` — Translate comments
- `src/cache.rs` — Translate comments, rename test functions
- `src/output/markdown.rs` — Translate comments, rename test functions
- `src/output/mod.rs` — Translate comments
- `src/parser/claude.rs` — Translate comments, rename test functions
- `src/parser/codex.rs` — Translate comments, rename test functions
- `src/parser/mod.rs` — Translate comments
- `src/redactor/mod.rs` — Translate comments, rename test functions
- `src/redactor/patterns.rs` — Translate comments
- `src/update.rs` — Translate comments

### Documentation (Translate/Rewrite)
- `README.md`
- `CLAUDE.md`
- `Cargo.toml` (description field)
- `docs/ARCHITECTURE.md`
- `docs/CONVENTIONS.md`
- `docs/MILESTONES.md`
- `docs/superpowers/specs/*.md` (8 existing Korean specs)
- `docs/superpowers/plans/*.md` (7 existing Korean plans)

### Delete
- `docs/LEARNING_GUIDE.md`

### Create (Private, not in repo)
- `~/.claude/projects/-Users-example-workspace-repos-company-example-rwd/CLAUDE.md` — Korean personal preferences

---

## Task 1: Create messages module

**Files:**
- Create: `src/messages.rs`
- Modify: `src/main.rs` (add `mod messages;`)

Audit all `println!`, `eprintln!`, `eprint!`, `format!` calls across all source files and centralize them as constants. Sub-group by domain.

- [ ] **Step 1: Audit all user-facing strings**

Read every `.rs` file and collect all Korean string literals in `println!`, `eprintln!`, `eprint!`, `format!`, and error string arguments. Categorize them by domain.

- [ ] **Step 2: Create `src/messages.rs`**

```rust
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

pub mod init {
    pub const SELECT_PROVIDER: &str = "LLM provider (anthropic/openai) [default: anthropic]: ";
    pub const ENTER_API_KEY_ANTHROPIC: &str = "Anthropic API key: ";
    pub const ENTER_API_KEY_OPENAI: &str = "OpenAI API key: ";
    pub const API_KEY_EMPTY: &str = "API key is empty.";

    pub fn output_path_prompt(default: &str) -> String {
        format!("Markdown output path [default: {}]: ", default)
    }
    pub fn api_key_set(masked: &str) -> String {
        format!("API key set: {}", masked)
    }
    pub fn output_path_set(path: &str) -> String {
        format!("Output path: {}", path)
    }
    pub fn config_saved(path: &str) -> String {
        format!("Config saved: {}", path)
    }
}

pub mod config {
    pub const NO_CONFIG: &str = "No config found. Run `rwd init` first.";
    pub const SELECT_SETTING: &str = "Select a setting to change";
    pub const LLM_PROVIDER: &str = "LLM provider";
    pub const OUTPUT_PATH: &str = "Markdown output path";
    pub const REDACTOR: &str = "Sensitive data masking";
    pub const LANGUAGE: &str = "Language";
    pub const EXIT: &str = "Exit";
    pub const NAV_HINT: &str = "  ↑↓ Navigate · Enter Select · Esc Exit";
    pub const NO_CHANGE: &str = "  No change";
    pub const CHANGED: &str = "  ✓ Changed";
    pub const CONFIRM_API_KEY: &str = "Change API key?";
    pub const NEW_API_KEY: &str = "  New API key: ";
    pub const CONFIG_SAVED: &str = "Config saved.";
    pub const USAGE: &str = "Usage: `rwd config` (interactive) or `rwd config <key> <value>`";

    pub fn unsupported_provider(name: &str) -> String {
        format!("Unsupported provider: '{}'. Available: anthropic, openai", name)
    }
    pub fn unknown_key(key: &str) -> String {
        format!("Unknown config key: '{}'. Available: output-path, provider, api-key, lang", key)
    }
}

pub mod error {
    pub const NO_CONFIG: &str = "No config found. Run `rwd init` first.";
    pub const NO_CACHE: &str = "No cache found. Running today analysis first...";
    pub const NO_CACHE_AFTER_ANALYSIS: &str = "Cache not found after analysis.";
    pub const NO_SESSIONS: &str = "No sessions to summarize.";
    pub const HOME_DIR_NOT_FOUND: &str = "Could not find home directory";
    pub const JSON_PARSE_FAILED: &str = "JSON parse failed";

    pub fn init_failed(e: &dyn std::fmt::Display) -> String {
        format!("Init failed: {}", e)
    }
    pub fn config_failed(e: &dyn std::fmt::Display) -> String {
        format!("Config change failed: {}", e)
    }
    pub fn update_failed(e: &dyn std::fmt::Display) -> String {
        format!("Update failed: {}", e)
    }
    pub fn unsupported_provider(name: &str) -> String {
        format!("Unsupported provider: {}", name)
    }
}

pub mod status {
    pub const CACHE_USED: &str = "Using cached analysis. (no entry changes)";
    pub const REWIND_DONE: &str = "Today's daily rewind is ready!";
    pub const SUMMARY_GENERATING: &str = "Generating development progress summary...";
    pub const SUMMARY_HEADER: &str = "=== Development Progress ===";
    pub const SLACK_GENERATING: &str = "Generating Slack message...";
    pub const COPIED_TO_CLIPBOARD: &str = "Copied to clipboard.";
    pub const CACHE_STALE_HINT: &str = "  Run `rwd today` first for latest results.\n";

    pub fn analyzing(provider: &str) -> String {
        format!("Analyzing insights via {} API...", provider)
    }
    pub fn redacted(count: usize, summary: &str) -> String {
        format!("Sensitive data: {} items masked ({})", count, summary)
    }
    pub fn cache_stale(cached: usize, current: usize) -> String {
        format!("⚠ Cache is outdated. (cache: {} entries, current: {} entries)", cached, current)
    }
}

pub mod verify {
    pub const VERIFYING_KEY: &str = "  Verifying API key...";
    pub const KEY_VERIFIED: &str = "  ✓ API key verified";
    pub const VERIFY_SKIPPED_CLIENT: &str = "  API key verification skipped (HTTP client error)";
    pub const VERIFY_SKIPPED_NETWORK: &str = "  API key verification skipped (network error)";

    pub fn key_invalid(status: u16) -> String {
        format!("  ⚠ API key is invalid ({}). Please check your key.", status)
    }
}

pub mod update {
    // Update-related messages — to be filled during Step 1 audit
}

pub mod lang {
    pub const SELECT: &str = "Language (en/ko) [default: en]: ";
    pub const NOT_CONFIGURED: &str = "Language not configured. Please select (en/ko) [default: en]: ";

    pub fn saved(lang: &str) -> String {
        format!("✓ Saved: lang = \"{}\"", lang)
    }
    pub fn unsupported(lang: &str) -> String {
        format!("Unsupported language: '{}'. Available: en, ko", lang)
    }
}
```

Adjust constants based on the actual audit in Step 1. The above is a starting template.

- [ ] **Step 3: Register module in `src/main.rs`**

Add `mod messages;` to the module declarations at the top of `src/main.rs`.

- [ ] **Step 4: Verify build**

Run: `cargo build`
Expected: Compiles successfully (module exists but unused warnings are OK)

- [ ] **Step 5: Commit**

```bash
git add src/messages.rs src/main.rs
git commit -m "feat: add messages module with centralized CLI string constants"
```

---

## Task 2: Replace Korean CLI messages with messages module

**Files:**
- Modify: `src/main.rs` — Replace all Korean string literals
- Modify: `src/config.rs` — Replace all Korean string literals
- Modify: `src/update.rs` — Replace Korean string literals
- Modify: `src/analyzer/mod.rs` — Replace Korean string literals
- Modify: `src/analyzer/provider.rs` — Replace error message in `load_provider()`
- Modify: `src/redactor/mod.rs` — Replace Korean string literals

- [ ] **Step 1: Replace messages in `src/main.rs`**

Replace all Korean strings in `println!`, `eprintln!`, `eprint!`, error strings with `messages::` references. Example transformations:

```rust
// Non-parameterized — use constant directly:
// Before:
eprintln!("설정 파일이 없습니다. 먼저 `rwd init`을 실행해 주세요.");
// After:
eprintln!("{}", messages::error::NO_CONFIG);

// Parameterized — call function:
// Before:
eprintln!("{provider_label} API로 인사이트 분석 중...");
// After:
eprintln!("{}", messages::status::analyzing(provider_label));
```

**Important:** Rust's `format!()` macro requires a string literal as its first argument — `&str` constants cannot be passed to `format!()`. This is why parameterized messages use functions returning `String` instead of constants with `{}` placeholders.

- [ ] **Step 2: Replace messages in `src/config.rs`**

Replace all Korean strings in `run_init()`, `run_config()`, `run_config_interactive()`, `verify_api_key()`.

- [ ] **Step 3: Replace messages in remaining files**

Replace Korean strings in `src/update.rs`, `src/analyzer/mod.rs`, `src/analyzer/provider.rs`, `src/redactor/mod.rs`.

- [ ] **Step 4: Verify build and tests**

Run: `cargo build && cargo clippy && cargo test`
Expected: All pass. No Korean string literals remain in source (except prompts, which are handled in Task 6).

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "refactor: replace Korean CLI messages with messages module references"
```

---

## Task 3: Remove learning comments, translate design comments

**Files:**
- Modify: All 20 `.rs` files in `src/`

- [ ] **Step 1: Process `src/main.rs`**

Delete learning comments (Rust Book references, concept explanations like "tokio는 Rust의 비동기 런타임으로..."). Translate design intent comments to idiomatic English. Trim verbose comments.

- [ ] **Step 2: Process `src/cli.rs`**

Delete learning comments. Translate `///` doc comments to English (e.g., `/// 오늘의 세션 로그를 분석합니다` → `/// Analyze today's session logs`).

- [ ] **Step 3: Process `src/config.rs`**

Delete learning comments (serde explanations, stdin().read_line() explanations). Translate design comments.

- [ ] **Step 4: Process `src/analyzer/*.rs`**

Process all 8 files: `mod.rs`, `provider.rs`, `insight.rs`, `openai.rs`, `anthropic.rs`, `planner.rs`, `prompt.rs`, `summarizer.rs`. Delete learning comments, translate design comments.

**Important:** In `src/analyzer/insight.rs`, when translating the error message `"JSON 파싱 실패"` to English (e.g., `messages::error::JSON_PARSE_FAILED`), also update the matching string check in `src/analyzer/mod.rs` (line ~316: `err_msg.contains("JSON 파싱 실패")`) to use the same constant. Both must change in sync.

- [ ] **Step 5: Process remaining files**

Process: `src/cache.rs`, `src/output/mod.rs`, `src/output/markdown.rs`, `src/parser/mod.rs`, `src/parser/claude.rs`, `src/parser/codex.rs`, `src/redactor/mod.rs`, `src/redactor/patterns.rs`, `src/update.rs`.

- [ ] **Step 6: Verify build and tests**

Run: `cargo build && cargo clippy && cargo test`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add src/
git commit -m "refactor: remove learning comments, translate design comments to English"
```

---

## Task 4: Rename test functions to English

**Files:**
- Modify: `src/config.rs` (6 tests)
- Modify: `src/cache.rs` (3 tests)
- Modify: `src/redactor/mod.rs` (19 tests)
- Modify: `src/analyzer/insight.rs` (7+ tests)
- Modify: `src/analyzer/planner.rs` (4 tests)
- Modify: `src/analyzer/prompt.rs` (4 tests)
- Modify: `src/output/markdown.rs` (7 tests)
- Modify: `src/parser/claude.rs` (3 tests)
- Modify: `src/parser/codex.rs` (1 test)

- [ ] **Step 1: Rename tests in `src/config.rs`**

```rust
// Before → After
test_config_path_rwd_디렉토리_포함 → test_config_path_includes_rwd_dir
test_save_and_load_config_왕복_확인 → test_save_and_load_config_roundtrip
test_detect_obsidian_vault_obsidian폴더_있으면_경로반환 → test_detect_vault_returns_path_when_obsidian_dir_exists
test_detect_obsidian_vault_없으면_None → test_detect_vault_returns_none_when_missing
test_config_redactor_없으면_none → test_config_redactor_none_when_missing
test_config_redactor_있으면_파싱 → test_config_redactor_parses_when_present
```

Also translate any Korean strings inside test bodies (e.g., `.expect("경로 생성 성공")` → `.expect("path creation should succeed")`).

- [ ] **Step 2: Rename tests in `src/redactor/mod.rs`**

```rust
test_api_key_마스킹 → test_api_key_masking
test_aws_key_마스킹 → test_aws_key_masking
test_github_token_마스킹 → test_github_token_masking
test_slack_token_마스킹 → test_slack_token_masking
test_bearer_token_마스킹 → test_bearer_token_masking
test_env_secret_따옴표감싼_값만_매칭 → test_env_secret_matches_quoted_values_only
test_env_secret_따옴표없으면_미매칭 → test_env_secret_no_match_without_quotes
test_env_secret_작은따옴표_매칭 → test_env_secret_matches_single_quotes
test_private_ip_마스킹 → test_private_ip_masking
test_private_key_헤더_마스킹 → test_private_key_header_masking
test_민감정보_없으면_원본_유지 → test_no_sensitive_data_preserves_original
test_여러_패턴_동시_매칭 → test_multiple_patterns_matched
test_같은_패턴_여러번_매칭 → test_same_pattern_matched_multiple_times
test_empty_결과_기본값 → test_empty_result_defaults
test_merge_두_결과_합산 → test_merge_two_results
test_format_summary_알파벳순 → test_format_summary_alphabetical_order
test_api_key_짧으면_미매칭 → test_api_key_short_no_match
test_public_ip_미매칭 → test_public_ip_no_match
test_현실적_프롬프트_통합_마스킹 → test_realistic_prompt_combined_masking
```

- [ ] **Step 3: Rename tests in remaining files**

Apply same pattern to `src/cache.rs`, `src/analyzer/insight.rs`, `src/analyzer/planner.rs`, `src/analyzer/prompt.rs`, `src/output/markdown.rs`, `src/parser/claude.rs`, `src/parser/codex.rs`.

- [ ] **Step 4: Verify all tests pass**

Run: `cargo test`
Expected: All ~98 tests pass with new English names.

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "refactor: rename test functions from Korean to English"
```

---

## Task 5: Add Lang enum, config field, and --lang CLI flag

**Files:**
- Modify: `src/config.rs` — Add `Lang` enum, `lang: Option<Lang>` field, init/config integration
- Modify: `src/cli.rs` — Add `--lang` flag to Today, Summary, Slack subcommands
- Modify: `src/main.rs` — Resolve lang from flag → config → migration prompt
- Test: `src/config.rs` (new tests)

- [ ] **Step 1: Write tests for Lang enum and config**

Add to `src/config.rs` tests:

```rust
#[test]
fn test_config_lang_none_when_missing() {
    let toml_str = r#"
[llm]
provider = "anthropic"
api_key = "sk-test"

[output]
path = "/tmp/vault"
"#;
    let config: Config = toml::from_str(toml_str).expect("parse should succeed");
    assert!(config.lang.is_none());
}

#[test]
fn test_config_lang_parses_when_present() {
    // lang is a top-level field in Config, NOT nested under [output]
    let toml_str = r#"
[llm]
provider = "anthropic"
api_key = "sk-test"

[output]
path = "/tmp/vault"

lang = "ko"
"#;
    let config: Config = toml::from_str(toml_str).expect("parse should succeed");
    assert!(matches!(config.lang, Some(Lang::Ko)));
}

#[test]
fn test_lang_serializes_lowercase() {
    let lang = Lang::Ko;
    let serialized = toml::to_string(&lang).expect("serialize should succeed");
    assert!(serialized.contains("ko"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_config_lang`
Expected: FAIL — `Lang` enum doesn't exist yet.

- [ ] **Step 3: Implement Lang enum and Config field**

In `src/config.rs`:

```rust
/// Supported languages for LLM output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Ko,
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lang::En => write!(f, "en"),
            Lang::Ko => write!(f, "ko"),
        }
    }
}
```

Add `pub lang: Option<Lang>` to `Config` struct.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test test_config_lang`
Expected: All 3 new tests PASS.

- [ ] **Step 5: Add lang to `rwd init`**

In `run_init()`, after output path prompt, add language selection:

```rust
eprint!("{}", messages::lang::SELECT);
let mut lang_input = String::new();
std::io::stdin().read_line(&mut lang_input)?;
let lang = match lang_input.trim() {
    "ko" => Lang::Ko,
    _ => Lang::En,
};
```

Set `lang: Some(lang)` in the Config struct.

- [ ] **Step 6: Add lang to `rwd config` interactive menu**

Add a new menu item between "redactor" and "Exit" in `run_config_interactive()`. Use `Select` with `["en", "ko"]` options.

- [ ] **Step 7: Add lang to `rwd config <key> <value>`**

In `run_config()`, add a `"lang"` match arm:

```rust
"lang" => {
    let lang = match value {
        "ko" => Lang::Ko,
        "en" => Lang::En,
        _ => return Err(format!("Unsupported language: '{}'. Available: en, ko", value).into()),
    };
    config.lang = Some(lang);
    eprintln!("Language changed: {value}");
}
```

- [ ] **Step 8: Add `--lang` flag to CLI**

In `src/cli.rs`, add to Today, Summary, and Slack variants:

```rust
/// Override output language (en/ko)
#[arg(long)]
lang: Option<String>,
```

- [ ] **Step 9: Add lang resolution in `src/main.rs`**

Create a helper function to resolve lang from flag → config → migration prompt:

```rust
fn resolve_lang(flag: &Option<String>, config: &mut Config) -> Result<Lang, Box<dyn std::error::Error>> {
    // 1. --lang flag takes priority
    if let Some(lang_str) = flag {
        return match lang_str.as_str() {
            "ko" => Ok(Lang::Ko),
            "en" => Ok(Lang::En),
            _ => Err(format!("Unsupported language: '{}'. Available: en, ko", lang_str).into()),
        };
    }
    // 2. Config value
    if let Some(lang) = &config.lang {
        return Ok(lang.clone());
    }
    // 3. Migration prompt
    eprint!("{}", messages::lang::NOT_CONFIGURED);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let lang = match input.trim() {
        "ko" => Lang::Ko,
        _ => Lang::En,
    };
    // Save to config
    config.lang = Some(lang.clone());
    let config_file = config::config_path()?;
    config::save_config(config, &config_file)?;
    eprintln!("{}", messages::lang::saved(&lang.to_string()));
    Ok(lang)
}
```

Thread the resolved `Lang` value through `run_today()`, `run_summary()`, `run_slack()`.

**Note on `Commands::Today` destructuring:** The existing `Commands::Today { verbose }` pattern in `main.rs` must become `Commands::Today { verbose, lang }` (same for Summary and Slack).

- [ ] **Step 10: Add tests for resolve_lang**

Test the flag-override and config-value paths (migration prompt path is interactive and difficult to unit test):

```rust
#[test]
fn test_resolve_lang_flag_overrides_config() {
    let flag = Some("ko".to_string());
    let mut config = make_test_config();
    config.lang = Some(Lang::En);
    let result = resolve_lang_from_flag_or_config(&flag, &config);
    assert!(matches!(result, Ok(Lang::Ko)));
}

#[test]
fn test_resolve_lang_uses_config_when_no_flag() {
    let flag = None;
    let mut config = make_test_config();
    config.lang = Some(Lang::Ko);
    let result = resolve_lang_from_flag_or_config(&flag, &config);
    assert!(matches!(result, Ok(Lang::Ko)));
}
```

Note: Extract the non-interactive parts of `resolve_lang` into a testable `resolve_lang_from_flag_or_config` function, keeping the migration prompt in the main `resolve_lang` wrapper.

- [ ] **Step 11: Verify build and all tests**

Run: `cargo build && cargo clippy && cargo test`
Expected: All pass.

- [ ] **Step 12: Commit**

```bash
git add src/cli.rs src/config.rs src/main.rs src/messages.rs
git commit -m "feat: add Lang enum, config field, --lang flag, and migration prompt"
```

---

## Task 6: Externalize prompts to .md files with EN/KO support

**Files:**
- Create: `prompts/system_en.md`, `prompts/system_ko.md`
- Create: `prompts/summary_en.md`, `prompts/summary_ko.md`
- Create: `prompts/slack_en.md`, `prompts/slack_ko.md`
- Create: `prompts/chunk_summarize_en.md`, `prompts/chunk_summarize_ko.md`
- Modify: `src/analyzer/provider.rs` — Replace const strings with `include_str!()`, add lang-aware prompt selection
- Modify: `src/analyzer/summarizer.rs` — Replace `CHUNK_SUMMARIZE_PROMPT` with `include_str!()`
- Modify: `src/analyzer/mod.rs` — Thread `lang` parameter through all orchestration functions
- Modify: `src/main.rs` — Pass `lang` to analyzer functions

- [ ] **Step 1: Create Korean prompt files (preserve current content)**

Extract current prompts into `.md` files:
- `prompts/system_ko.md` — exact content of current `SYSTEM_PROMPT` from `provider.rs`
- `prompts/summary_ko.md` — exact content of current `SUMMARY_PROMPT` from `provider.rs`
- `prompts/slack_ko.md` — exact content of current `SLACK_PROMPT` from `provider.rs`
- `prompts/chunk_summarize_ko.md` — exact content of current `CHUNK_SUMMARIZE_PROMPT` from `summarizer.rs`

- [ ] **Step 2: Create English prompt files**

Write English versions of each prompt:
- `prompts/system_en.md` — Same structure but `ALL text values MUST be in English`
- `prompts/summary_en.md` — Same rules but `ALL text MUST be in English`
- `prompts/slack_en.md` — English version: `[Today's Work Update]` header, "~was completed" style, translate dev-term simplification rules to English equivalents
- `prompts/chunk_summarize_en.md` — English version of chunk summarization prompt

- [ ] **Step 3: Replace const strings with `include_str!()` in `provider.rs`**

```rust
const SYSTEM_PROMPT_EN: &str = include_str!("../../prompts/system_en.md");
const SYSTEM_PROMPT_KO: &str = include_str!("../../prompts/system_ko.md");
const SUMMARY_PROMPT_EN: &str = include_str!("../../prompts/summary_en.md");
const SUMMARY_PROMPT_KO: &str = include_str!("../../prompts/summary_ko.md");
const SLACK_PROMPT_EN: &str = include_str!("../../prompts/slack_en.md");
const SLACK_PROMPT_KO: &str = include_str!("../../prompts/slack_ko.md");

use crate::config::Lang;

fn get_system_prompt(lang: &Lang) -> &'static str {
    match lang {
        Lang::Ko => SYSTEM_PROMPT_KO,
        Lang::En => SYSTEM_PROMPT_EN,
    }
}

fn get_summary_prompt(lang: &Lang) -> &'static str {
    match lang {
        Lang::Ko => SUMMARY_PROMPT_KO,
        Lang::En => SUMMARY_PROMPT_EN,
    }
}

fn get_slack_prompt(lang: &Lang) -> &'static str {
    match lang {
        Lang::Ko => SLACK_PROMPT_KO,
        Lang::En => SLACK_PROMPT_EN,
    }
}
```

- [ ] **Step 4: Update `call_api`, `call_summary_api`, `call_slack_api` to accept `lang`**

```rust
pub async fn call_api(
    &self,
    api_key: &str,
    conversation_text: &str,
    max_tokens: u32,
    lang: &Lang,
) -> Result<String, super::AnalyzerError> {
    let prompt = get_system_prompt(lang);
    match self {
        LlmProvider::Anthropic => {
            super::anthropic::call_anthropic_api(api_key, prompt, conversation_text, max_tokens).await
        }
        LlmProvider::OpenAi => {
            super::openai::call_openai_api(api_key, prompt, conversation_text, max_tokens).await
        }
    }
}
```

Same pattern for `call_summary_api` and `call_slack_api`.

- [ ] **Step 4b: Update `src/analyzer/summarizer.rs`**

Replace `CHUNK_SUMMARIZE_PROMPT` const with `include_str!()` and add lang selection, same pattern as above.

- [ ] **Step 4c: Add tests for prompt selection**

```rust
#[test]
fn test_get_system_prompt_en_ko_differ() {
    let en = get_system_prompt(&Lang::En);
    let ko = get_system_prompt(&Lang::Ko);
    assert_ne!(en, ko);
    assert!(en.contains("English"));
    assert!(ko.contains("한국어"));
}
```

Same for summary and slack prompts.

- [ ] **Step 5: Thread `lang` through `src/analyzer/mod.rs`**

Update ALL orchestration functions to accept and pass `lang: &Lang`:
- `analyze_entries()` — calls `provider.call_api()`
- `analyze_codex_entries()` — calls `provider.call_api()`
- `analyze_summary()` — calls `provider.call_summary_api()`
- `analyze_slack()` — calls `provider.call_slack_api()`
- `execute_direct_step()` — private, calls `provider.call_api()`
- `execute_summarize_step()` — private, calls `provider.call_api_with_max_tokens()`
- `execute_plan()` — orchestrator, passes `lang` to step functions

**Note:** `load_provider()` does NOT need changes — it returns `(LlmProvider, String)` and `lang` is passed separately.

- [ ] **Step 6: Thread `lang` through `src/main.rs`**

Update `run_today()`, `run_summary()`, `run_slack()` to resolve `lang` and pass it to analyzer functions.

- [ ] **Step 7: Verify build and tests**

Run: `cargo build && cargo clippy && cargo test`
Expected: All pass.

- [ ] **Step 8: Commit**

```bash
git add prompts/ src/analyzer/ src/main.rs
git commit -m "feat: externalize prompts to md files with EN/KO support"
```

---

## Task 7: Translate all documentation to English

**Files:**
- Rewrite: `README.md`
- Translate: `docs/ARCHITECTURE.md`
- Translate: `docs/CONVENTIONS.md`
- Translate: `docs/MILESTONES.md` (remove learning points)
- Delete: `docs/LEARNING_GUIDE.md`
- Translate: `docs/superpowers/specs/*.md` (8 existing Korean specs)
- Translate: `docs/superpowers/plans/*.md` (7 existing Korean plans)

- [ ] **Step 1: Rewrite `README.md` in English**

Full English rewrite covering: project description, installation, usage (`rwd today`, `rwd summary`, `rwd slack`, `rwd config`), configuration, macOS quarantine note.

- [ ] **Step 2: Translate `docs/ARCHITECTURE.md`**

Translate architecture overview, core flow diagrams, module descriptions to English.

- [ ] **Step 3: Translate `docs/CONVENTIONS.md`**

Translate coding conventions to English. Remove learning-specific rules.

- [ ] **Step 4: Translate `docs/MILESTONES.md`**

Translate milestones to English. Remove "학습 포인트" sections, keep as historical milestone record.

- [ ] **Step 5: Delete `docs/LEARNING_GUIDE.md`**

```bash
git rm docs/LEARNING_GUIDE.md
```

- [ ] **Step 6: Translate `docs/superpowers/specs/*.md` (8 files)**

Translate each existing Korean spec to English. Preserve structure and formatting.

- [ ] **Step 7: Translate `docs/superpowers/plans/*.md` (7 files)**

Translate each existing Korean plan to English. Preserve structure and formatting.

- [ ] **Step 8: Verify no broken links**

Check that all internal doc references (`[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)` etc.) still work.

- [ ] **Step 9: Commit**

```bash
git add README.md docs/
git commit -m "docs: translate all documentation to English"
```

---

## Task 8: Update CLAUDE.md and project metadata

**Files:**
- Rewrite: `CLAUDE.md` (repo root)
- Modify: `Cargo.toml` (description)
- Create: `~/.claude/projects/-Users-example-workspace-repos-company-example-rwd/CLAUDE.md` (private, Korean)

- [ ] **Step 1: Rewrite repo `CLAUDE.md` in English**

Remove learning-specific rules ("한 번에 100줄 이상 금지", "개념 설명 필수", "unsafe 금지" etc.). Write general open-source project guidelines:

```markdown
# AGENTS.md — rwd (rewind)

CLI tool that analyzes AI coding session logs and extracts daily development insights, saving them as Markdown to an Obsidian vault.

## Technical Constraints

- Language: Rust (2024 Edition), Stable 1.94.0+
- Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Milestones: [docs/MILESTONES.md](docs/MILESTONES.md)
- Conventions: [docs/CONVENTIONS.md](docs/CONVENTIONS.md)

## MUST DO

- Validate with `cargo build`, `cargo clippy`, `cargo test` after changes
- Use `Result` and `?` operator for error handling (no `unwrap()` in non-test code)
- Use Context7 MCP to reference crate APIs before writing code
- Follow conventions in [docs/CONVENTIONS.md](docs/CONVENTIONS.md)

## MUST NOT DO

- No `unsafe` blocks
- No deprecated APIs
```

- [ ] **Step 2: Update `Cargo.toml` description**

```toml
description = "CLI tool that analyzes AI coding session logs and extracts daily development insights"
```

- [ ] **Step 3: Create private Korean CLAUDE.md**

Write to `~/.claude/projects/-Users-example-workspace-repos-company-example-rwd/CLAUDE.md`:

```markdown
# Personal Preferences

- 설명은 항상 한국어로 해줄 것
- 설계 결정 시 대안과 선택 이유를 먼저 제시하고, 납득한 후에 진행할 것
- 새로운 Rust 개념 사용 시, 해당 개념이 무엇이고 왜 필요한지 한국어로 설명할 것
```

Note: This file does not currently exist. Create it new. The directory already exists (contains `memory/` subfolder).

- [ ] **Step 4: Commit repo changes only**

```bash
git add CLAUDE.md Cargo.toml
git commit -m "chore: rewrite CLAUDE.md in English, update Cargo.toml description"
```

(Private CLAUDE.md is not in the repo.)

---

## Task 9: Update GitHub repo description and releases

**Files:** None (GitHub API only)

- [ ] **Step 1: Update repo description**

```bash
gh repo edit --description "CLI tool that analyzes AI coding session logs and extracts daily development insights"
```

- [ ] **Step 2: Update existing release notes**

List releases and update each one's body to English:

```bash
gh release list
# For each release:
gh release edit <tag> --notes "..."
```

- [ ] **Step 3: Verify on GitHub**

```bash
gh repo view
gh release list
```

- [ ] **Step 4: Commit** (no code changes, just record)

No git commit needed — changes are on GitHub only.

---

## Final Verification

After all 9 tasks are complete:

- [ ] **Full build and test**

```bash
cargo build && cargo clippy && cargo test
```

- [ ] **Grep for remaining Korean**

Search for any remaining Korean characters in source code (excluding `prompts/*_ko.md`):

```bash
grep -rn '[가-힣]' src/ --include='*.rs'
```

Expected: No results.

- [ ] **Test `--lang` flag**

```bash
rwd today --lang ko   # Should produce Korean insights
rwd today --lang en   # Should produce English insights
rwd today             # Should use config setting
```

- [ ] **Version bump and tag** (if desired)

```bash
# Update version in Cargo.toml, commit, tag, push
```
