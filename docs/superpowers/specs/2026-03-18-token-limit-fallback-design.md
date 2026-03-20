# Design: Claude Code Analysis Token Limit Fallback

## Purpose

Handle LLM API token limits gracefully during Claude Code log analysis:
- 400 (context overflow): Automatically switch to per-session analysis
- 429 (TPM exceeded): Display a friendly error message with suggested solutions

## Current State

- `analyze_entries()` sends all Claude Code entries for the day to the LLM in one call
- Hitting OpenAI's TPM limit (30K/min) results in a 429 error and analysis failure
- Codex already uses per-session analysis (`analyze_codex_entries()` per session)

## Design

### Error-Specific Handling Strategy

#### 400 (Context Window Exceeded) → Per-Session Split Fallback

The fallback logic is handled inside `analyze_entries()`. It reuses the already-loaded `(provider, api_key)` to make per-session API calls.

```
analyze_entries(all entries)
  → build_prompt(all) → API call
  → Success: same as before
  → 400 (token-related) error:
    → Print "Switching to per-session analysis" notice
    → Extract session ID list with extract_session_ids()
    → For each session:
      → Filter original entries by session_id + clone → Vec<LogEntry>
      → build_prompt(&session_entries) → API call (reusing existing function)
      → Apply redaction
      → If token error occurs on individual session: skip that session + print warning
      → Show progress: "Analyzing session 3/8..."
    → Merge each AnalysisResult.sessions + RedactResult
```

#### 429 (TPM Exceeded) → Improved Error Message

429 cannot be resolved by splitting (splitting doesn't reduce the total tokens per minute). Instead, display a friendly error message:

```
API request rate (TPM) limit exceeded.
Solutions:
  • rwd config provider anthropic  (switch to Anthropic)
  • Upgrade your LLM provider plan  (increase TPM limit)
```

### Error Detection

Currently `anthropic.rs` and `openai.rs` return different error message formats:
- `anthropic.rs`: `"API request failed ({status}): {error_body}"`
- `openai.rs`: `"OpenAI API request failed ({status}): {error_body}"`

Detection via error message string parsing:
- Contains "429" → TPM error
- Contains "400" + ("token" or "context") → context error

**Risk:** Relies on error message format — if the format changes, fallback may not trigger. Will be migrated to structured errors (thiserror) during M5's error type improvements.

### Changed Files

#### 1. `src/analyzer/mod.rs` — `analyze_entries()` fallback logic

- Inspect error message on API call failure
- 400 token error → per-session split analysis + merge results/RedactResult
- 429 error → return friendly error message
- Other errors → propagate as before
- Add `is_context_limit_error()`, `is_rate_limit_error()` detection functions

#### 2. `src/analyzer/prompt.rs` — Add session ID extraction function

- Add `extract_session_ids(entries: &[LogEntry]) -> Vec<String>`
- For per-session splitting, filter + clone original entries by session_id into `Vec<LogEntry>`, then pass to existing `build_prompt()`
- No changes to `build_prompt()` signature

#### 3. `src/analyzer/insight.rs` — Add result merge function

- `merge_results(results: Vec<AnalysisResult>) -> AnalysisResult`
- Combine the `sessions` Vec from each result into one
- Module responsibility: "parsing and result composition"

### What Stays Unchanged

- `anthropic.rs` / `openai.rs` — no changes to error response format
- `build_prompt()` signature — unchanged
- Codex analysis — already per-session
- `provider.rs` — no per-provider token limit configuration
- Pre-estimation of token counts — unnecessary (try first, then fallback)

### Tests

- `is_context_limit_error()` / `is_rate_limit_error()` error detection tests
- `merge_results()` merge logic tests
- `extract_session_ids()` session ID extraction tests
