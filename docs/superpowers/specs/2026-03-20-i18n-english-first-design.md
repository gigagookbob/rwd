# Design: i18n — English-First Internationalization

**Date:** 2026-03-20
**Branch:** `feat/i18n-english-first`
**Approach:** Single branch, category-based commits

## Background

rwd is transitioning from a Rust learning project to a production CLI tool for international audiences. All Korean content — source comments, CLI messages, LLM prompts, documentation, and GitHub metadata — will be converted to English. The developer (Korean) retains Korean explanations via private CLAUDE.md and a `--lang ko` option for LLM output.

## Scope

| Category | Count | Action |
|----------|-------|--------|
| Learning comments | ~800 lines | **Delete** |
| Design intent comments | ~400 lines | Translate to English |
| CLI messages | ~100 | English, centralized in `src/messages.rs` (sub-grouped) |
| LLM prompts | 3 (SYSTEM, SUMMARY, SLACK) | Externalize to `prompts/*.md`, EN/KO pairs |
| Test function names | ~56 | Rename to English |
| Documentation | 19 .md files | Translate to English |
| LEARNING_GUIDE.md | 1 | **Delete** |
| README.md | 1 | Rewrite in English |
| CLAUDE.md (repo) | 1 | Rewrite as English open-source guidelines |
| CLAUDE.md (private) | 1 | Create Korean personal preferences in `~/.claude/projects/` |
| Cargo.toml description | 1 | English |
| GitHub About / Releases | — | English |

## Design Decisions

### 1. Messages Module (`src/messages.rs`)

All user-facing strings are centralized as constants in a single module. No i18n crate is introduced — the module structure prepares for future i18n adoption by providing a single replacement point.

```rust
// src/messages.rs

/// Sub-grouped by domain for readability (~100 constants total).
pub mod init {
    pub const SELECT_PROVIDER: &str = "LLM provider (anthropic/openai) [default: anthropic]: ";
    pub const SELECT_LANG: &str = "Language (en/ko) [default: en]: ";
    // ...
}

pub mod error {
    pub const NO_CONFIG: &str = "No config found. Run `rwd init` first.";
    pub const UNSUPPORTED_PROVIDER: &str = "Unsupported provider: '{}'";
    // ...
}

pub mod status {
    pub const ANALYZING: &str = "Analyzing insights via {} API...";
    pub const REWIND_DONE: &str = "Today's daily rewind is ready!";
    // ...
}
```

Logic code references these via `messages::error::NO_CONFIG`.

**Format placeholder convention:** Constants containing `{}` are used with `format!()` at call sites (e.g., `format!(messages::status::ANALYZING, provider_name)`). No wrapper functions — direct `format!()` keeps it simple.

**Rationale:** A full i18n framework (rust-i18n, fluent) is overkill for ~100 messages with only 2 languages needed (and only for prompts). Centralizing constants with sub-grouping achieves the same structural benefit with zero dependencies.

### 2. Prompt Externalization (`prompts/*.md`)

LLM prompts are extracted from `src/analyzer/provider.rs` const strings into standalone Markdown files, embedded at compile time via `include_str!()`.

```
prompts/
├── system_en.md
├── system_ko.md
├── summary_en.md
├── summary_ko.md
├── slack_en.md
└── slack_ko.md
```

```rust
const SYSTEM_PROMPT_EN: &str = include_str!("../../prompts/system_en.md");
const SYSTEM_PROMPT_KO: &str = include_str!("../../prompts/system_ko.md");
// ... same pattern for SUMMARY_PROMPT and SLACK_PROMPT

fn get_prompt(base: &str, lang: &Lang) -> &'static str {
    // Selects the appropriate prompt variant by language.
    // Called before each API method — the resolved prompt is passed
    // as a parameter to `call_api_with_max_tokens(system_prompt, ...)`.
}
```

**Integration with API call chain:** The existing `call_api_with_max_tokens` already accepts a `system_prompt: &str` parameter. The `lang` value is resolved to a prompt string *before* calling API methods. `call_api`, `call_summary_api`, and `call_slack_api` each resolve their prompt via `get_prompt()` and pass it through. No signature changes needed on the underlying HTTP call methods.

**Rationale:**
- Single binary — no runtime file loading, no "file not found" errors
- `.md` files get syntax highlighting and preview in editors
- EN/KO versions are cleanly separated at file level
- Easy to review prompt changes in PRs

### 3. Language Configuration

#### Config struct

```rust
/// Supported languages for LLM output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Ko,
}

pub struct Config {
    pub llm: LlmConfig,
    pub output: OutputConfig,
    pub redactor: Option<RedactorConfig>,
    pub lang: Option<Lang>,  // None = migration needed
}
```

`Option<Lang>` enum provides compile-time safety — no invalid values like `"jp"` can slip in. `Option` (not `#[serde(default)]`) is intentional: when `None`, the CLI prompts the user to choose before proceeding. `Option<T>` fields in serde default to `None` when the key is absent, so existing config files without `lang` will parse correctly without any annotation. This avoids surprising existing Korean users with sudden English output.

#### Priority

1. `--lang` flag (one-time override)
2. `config.toml` `lang` value
3. If `None` → prompt user to choose (first run after update)

#### Setting points

- `rwd init` — required selection: `Language (en/ko) [default: en]:`
- `rwd config` interactive menu — new "lang" item
- `rwd config lang ko` — direct CLI change

#### Default value display convention

All prompts use `[default: X]` pattern consistently:

```
LLM provider (anthropic/openai) [default: anthropic]:
Markdown output path [default: /path/...]:
Language (en/ko) [default: en]:
```

### 4. Existing User Migration

When `lang` is `None` (config written before this update), the first command that needs LLM output will prompt:

```
Language not configured. Please select (en/ko) [default: en]: ko
✓ Saved: lang = "ko"
```

One-time prompt, saved to config, never asked again.

### 5. Source Code Comments

- **Delete:** Learning-oriented comments (Rust concept explanations, Book/tutorial references)
- **Translate:** Design intent comments ("why this approach", module/function purpose)
- **Rule:** Translated comments should be idiomatic English, not literal translations. Verbose comments are trimmed during translation.

### 6. Test Function Names

Rename from Korean `test_동작_조건_기대결과` to English `test_behavior_condition_expected`:

| Before | After |
|--------|-------|
| `test_config_path_rwd_디렉토리_포함` | `test_config_path_includes_rwd_dir` |
| `test_api_key_마스킹` | `test_api_key_masking` |
| `test_render_markdown_til_제목_설명_세션id_포함` | `test_render_markdown_til_includes_title_desc_session_id` |

### 7. Documentation

| File | Action |
|------|--------|
| README.md | Rewrite in English |
| docs/ARCHITECTURE.md | Translate |
| docs/CONVENTIONS.md | Translate |
| docs/MILESTONES.md | Translate (remove learning points, keep as milestone record) |
| docs/LEARNING_GUIDE.md | **Delete** |
| docs/superpowers/specs/*.md (8 existing) | Translate (this spec excluded — already English) |
| docs/superpowers/plans/*.md (8) | Translate |

### 8. CLAUDE.md Split

- **Repo `CLAUDE.md`** → English. Build/test rules, architecture references, coding conventions. No learning-specific rules.
- **`~/.claude/projects/.../CLAUDE.md`** → Korean. Personal preferences: "explain in Korean", "justify decisions before acting", etc.

### 9. GitHub Metadata

- Repository About description → English
- All existing Release notes → English
- Future releases → English

## Commit Strategy

Single branch `feat/i18n-english-first`, category-based commits:

| # | Commit | Scope |
|---|--------|-------|
| 1 | `feat: add messages module` | Create `src/messages.rs` with all CLI string constants |
| 2 | `refactor: replace Korean CLI messages with messages module` | Source files reference `messages::` constants |
| 3 | `refactor: remove learning comments, translate design comments` | All `.rs` files |
| 4 | `refactor: rename test functions to English` | All `#[test]` functions |
| 5 | `feat: add lang config and --lang flag` | `Config.lang`, migration prompt, CLI flag |
| 6 | `feat: externalize prompts to md files with EN/KO support` | `prompts/*.md`, `include_str!()`, prompt selection logic |
| 7 | `docs: translate all documentation to English` | README, docs/*.md, docs/superpowers/**/*.md |
| 8 | `chore: update CLAUDE.md and project metadata` | Repo CLAUDE.md, private CLAUDE.md, Cargo.toml |
| 9 | `chore: update GitHub repo description and releases` | GitHub API via gh CLI |

## Out of Scope

- Full i18n framework (rust-i18n, fluent) — deferred until 3+ languages needed
- CLI message localization — CLI stays English-only
- Automated translation pipeline
