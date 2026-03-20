# Redactor: Masking Sensitive Information Before LLM Transmission

> GitHub Issue: #26
> Date: 2026-03-17
> Version: v0.5.0

## Background

When running `rwd today`, session log text is sent directly to the LLM API. If the text contains sensitive information such as API keys, passwords, or internal IPs, it could be leaked to an external service.

## Decisions

| Item | Decision | Rationale |
|------|----------|-----------|
| Module | `src/redactor/` (mod.rs + patterns.rs) | Separation of concerns from parser/analyzer (SRP) |
| Pipeline position | Inside `analyzer/mod.rs`, after `build_prompt()` and before `call_api()` | The only path where prompt text goes to the LLM |
| Approach | Regex-based, `PatternKind` enum | Simplicity first, prepared for future Aho-Corasick replacement |
| API | `redact_text(&str) -> (String, RedactResult)` | Simple string transformation, infallible |
| Replacement format | `[REDACTED:TYPE]` | Preserves LLM context understanding + aids debugging |
| Built-in patterns | 8 | Covers major sensitive information types |
| Configuration | `config.toml` `[redactor] enabled = true` | Enabled by default, backward compatible |
| Terminal output | One-line masking count summary | Consistent with existing output style (no emojis) |
| Version | v0.5.0 | New feature module → minor bump |
| New dependency | `regex` crate | Standard regex library in the Rust ecosystem |

## Module Structure

```
src/redactor/
├── mod.rs       # Public API: redact_text(), RedactResult
└── patterns.rs  # Built-in pattern definitions (RedactorRule list, LazyLock initialization)
```

### Core Types

```rust
/// Pattern kind — FixedPrefix can be replaced with Aho-Corasick in the future.
/// Currently both kinds use Regex; kind serves as metadata only.
enum PatternKind {
    FixedPrefix,  // "sk-", "ghp_" etc. (fixed prefix based)
    Regex,        // "PASSWORD=..." etc. (complex patterns)
}

struct RedactorRule {
    name: &'static str,      // "API_KEY", "BEARER_TOKEN" etc.
    kind: PatternKind,
    pattern: Regex,           // Compiled regex
}

/// Masking result summary.
/// RedactResult::empty() creates an empty result (Default implementation).
struct RedactResult {
    pub total_count: usize,
    pub by_type: BTreeMap<String, usize>,  // Guarantees sorted output
}

impl RedactResult {
    /// Empty result for when redactor is disabled.
    pub fn empty() -> Self {
        Self { total_count: 0, by_type: BTreeMap::new() }
    }

    /// Merge multiple RedactResults (combine Claude + Codex results).
    pub fn merge(&mut self, other: RedactResult) {
        self.total_count += other.total_count;
        for (key, count) in other.by_type {
            *self.by_type.entry(key).or_insert(0) += count;
        }
    }
}
```

### Public API

```rust
/// Detects sensitive information in text and replaces it with [REDACTED:TYPE].
/// Patterns are initialized via LazyLock, so this function is infallible.
pub fn redact_text(text: &str) -> (String, RedactResult)
```

- Input: prompt text (return value of build_prompt / build_codex_prompt)
- Output: masked text + statistics
- No errors: patterns are compiled via `LazyLock` at program startup; failure means a programming error (panic)

## Pipeline Flow

```
Session logs (JSONL)
    ↓
parser (parsing)
    ↓ Vec<LogEntry>, Vec<(Summary, Vec<CodexEntry>)>
analyzer
    ├─ build_prompt() / build_codex_prompt()
    ├─ redactor::redact_text(&prompt)    ← newly added
    ├─ call_api(&redacted_prompt)
    └─ parse_response()
    ↓ (AnalysisResult, RedactResult)
main.rs: terminal summary output
    ↓
output (Markdown rendering + Vault save)
```

The only path where original text is sent externally is the LLM API call:
- Cache: stores AnalysisResult (processed insights), not original text
- Markdown: rendered from AnalysisResult, not original text

Therefore, masking the prompt text blocks all external leakage paths.

**Note: Existing API signature changes.** The `redactor_enabled: bool` parameter is added to `analyze_entries` and `analyze_codex_entries`, and the return type changes from `Result<AnalysisResult, _>` to `Result<(AnalysisResult, RedactResult), _>`. All call sites in `main.rs` and `run_summary` must be updated.

### analyzer/mod.rs Call Example

**Claude Code path:**

```rust
pub async fn analyze_entries(entries: &[LogEntry], redactor_enabled: bool)
    -> Result<(AnalysisResult, RedactResult), AnalyzerError>
{
    let prompt = prompt::build_prompt(entries)?;
    let (redacted_prompt, redact_result) = if redactor_enabled {
        redactor::redact_text(&prompt)
    } else {
        (prompt, RedactResult::empty())
    };
    let response = provider.call_api(&api_key, &redacted_prompt).await?;
    let result = insight::parse_response(&response)?;
    Ok((result, redact_result))
}
```

**Codex path:**

```rust
pub async fn analyze_codex_entries(entries: &[CodexEntry], session_id: &str, redactor_enabled: bool)
    -> Result<(AnalysisResult, RedactResult), AnalyzerError>
{
    let prompt = prompt::build_codex_prompt(entries, session_id)?;
    let (redacted_prompt, redact_result) = if redactor_enabled {
        redactor::redact_text(&prompt)
    } else {
        (prompt, RedactResult::empty())
    };
    let response = provider.call_api(&api_key, &redacted_prompt).await?;
    let result = insight::parse_response(&response)?;
    Ok((result, redact_result))
}
```

**Merging and displaying results in main.rs:**

```rust
// Merge RedactResult from each analyze call
let mut total_redact = RedactResult::empty();

let (claude_result, claude_redact) = analyze_entries(&entries, redactor_enabled).await?;
total_redact.merge(claude_redact);

for (summary, codex_entries) in &codex_sessions {
    let (codex_result, codex_redact) = analyze_codex_entries(&codex_entries, &id, redactor_enabled).await?;
    total_redact.merge(codex_redact);
}

// Display merged results
if total_redact.total_count > 0 {
    println!("Masked {} sensitive items ({})",
        total_redact.total_count,
        total_redact.format_summary());
}
```

**`format_summary` method** (`RedactResult` impl in `redactor/mod.rs`):

```rust
/// Generates a summary string like "API_KEY: 3, BEARER_TOKEN: 1"
pub fn format_summary(&self) -> String {
    self.by_type.iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join(", ")
}
```

## Built-in Detection Patterns

| Name | Kind | Pattern | Matches |
|------|------|---------|---------|
| `API_KEY` | FixedPrefix | `\bsk-[a-zA-Z0-9]{20,}\b` | OpenAI, Anthropic keys |
| `AWS_KEY` | FixedPrefix | `\bAKIA[0-9A-Z]{16}\b` | AWS Access Key ID |
| `GITHUB_TOKEN` | FixedPrefix | `\bgh[ps]_[a-zA-Z0-9]{36,}\b` | GitHub PAT |
| `SLACK_TOKEN` | FixedPrefix | `\bxox[bpsa]-[a-zA-Z0-9\-]+\b` | Slack Token (bot/user/session/app) |
| `BEARER_TOKEN` | Regex | `Bearer\s+[a-zA-Z0-9\-._~+/]+=*` | Authorization headers |
| `ENV_SECRET` | Regex | `(?i)(password\|secret\|api_key)\s*=\s*["'][^"']+["']` | Environment variable assignments (quoted values) |
| `PRIVATE_IP` | Regex | `\b(10\.\d+\.\d+\.\d+\|172\.(1[6-9]\|2\d\|3[01])\.\d+\.\d+\|192\.168\.\d+\.\d+)\b` | Private IP addresses |
| `PRIVATE_KEY` | Regex | `-----BEGIN[A-Z ]*PRIVATE KEY-----` | PEM private key header (v0.5.0 masks header only; key body requires multiline support) |

Changes (from review feedback):
- Added `\b` word boundaries to all FixedPrefix patterns (reduces false positives)
- `SLACK_TOKEN`: `xoxb-` → `xox[bpsa]-` (covers all Slack token types)
- `ENV_SECRET`: `\S+` → `["'][^"']+["']` (matches only quoted values, reduces false positives from code discussions)
- Added `PRIVATE_KEY` pattern (PEM key blocks)

### Known Limitations

- Multiline secrets (keys spanning multiple lines) are not supported in v0.5.0. Patterns match on a per-line basis.
- `ENV_SECRET` does not match unquoted assignments (`PASSWORD=mypass123`) to minimize false positives.

## config.toml Integration

```toml
[redactor]
enabled = true   # Default: true (active even when section is omitted)
```

- `[redactor]` section absent → `Option<RedactorConfig>` = `None` → default enabled (`enabled = true`)
- `enabled = false` → skip masking

Added as `redactor: Option<RedactorConfig>` to the existing config struct. `RedactorConfig` derives `Serialize + Deserialize`. Backward compatible.

## Terminal Output

```
=== rwd today (2026-03-17 14:30) ===

Claude Code
Total sessions: 3
Masked 5 sensitive items (API_KEY: 3, BEARER_TOKEN: 1, ENV_SECRET: 1)

Analyzing insights with Claude API...
```

- 0 items masked → line not displayed
- `redactor.enabled = false` → line not displayed
- `BTreeMap` ensures alphabetical sorting of type names
- No emojis, consistent with existing output style

## Follow-up Issues

- **Custom pattern support**: Allow user-defined regex patterns in config.toml
- **Aho-Corasick optimization**: Replace FixedPrefix patterns with Aho-Corasick for performance improvement
