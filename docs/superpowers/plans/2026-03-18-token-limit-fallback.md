# Token Limit Fallback Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When LLM token limit errors occur during Claude Code analysis, automatically fall back to per-session analysis for 400 errors, and display a friendly error message for 429 errors.

**Architecture:** When API call fails inside `analyze_entries()`, inspect the error message — 400 (context overflow) triggers per-session split analysis, 429 (TPM) returns a guidance message. Per-session analysis reuses existing `build_prompt()` and merges results.

**Tech Stack:** Rust, serde_json

**Spec:** `docs/superpowers/specs/2026-03-18-token-limit-fallback-design.md`

---

## Chunk 1: Error Detection + Result Merging + Session ID Extraction + Fallback Logic

### File Structure

| File | Change | Role |
|------|--------|------|
| `src/analyzer/mod.rs` | Modify | `analyze_entries()` fallback logic, error detection functions |
| `src/analyzer/prompt.rs` | Modify | Add `extract_session_ids()` |
| `src/analyzer/insight.rs` | Modify | Add `merge_results()` |

---

### Task 1: Error detection function tests and implementation

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: Write error detection tests**

Add a test module at the end of `src/analyzer/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_context_limit_error_true_for_400_with_token() {
        let err = "API request failed (400 Bad Request): {\"error\":{\"message\":\"This model's maximum context length is 128000 tokens\"}}";
        assert!(is_context_limit_error(err));
    }

    #[test]
    fn test_is_context_limit_error_true_for_400_with_context() {
        let err = "OpenAI API request failed (400 Bad Request): {\"error\":{\"code\":\"context_length_exceeded\"}}";
        assert!(is_context_limit_error(err));
    }

    #[test]
    fn test_is_context_limit_error_false_for_429() {
        let err = "OpenAI API request failed (429 Too Many Requests): rate limit";
        assert!(!is_context_limit_error(err));
    }

    #[test]
    fn test_is_context_limit_error_false_for_general_error() {
        let err = "API request failed (500 Internal Server Error): server error";
        assert!(!is_context_limit_error(err));
    }

    #[test]
    fn test_is_rate_limit_error_true_for_429() {
        let err = "OpenAI API request failed (429 Too Many Requests): {\"error\":{\"message\":\"Rate limit exceeded\"}}";
        assert!(is_rate_limit_error(err));
    }

    #[test]
    fn test_is_rate_limit_error_false_for_400() {
        let err = "API request failed (400 Bad Request): token limit";
        assert!(!is_rate_limit_error(err));
    }
}
```

- [ ] **Step 2: Verify test failure**

Run: `cargo test -p rwd test_is_context_limit -- --nocapture`
Expected: compile error — functions don't exist

- [ ] **Step 3: Implement error detection functions**

Add below the `analyze_codex_entries` function in `src/analyzer/mod.rs`:

```rust
/// Determines if an API error is a context window overflow (400).
/// Checks if the error message contains "400" and ("token" or "context").
/// Note: relies on error message format — will be migrated to structured error types in M5.
fn is_context_limit_error(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    lower.contains("400") && (lower.contains("token") || lower.contains("context"))
}

/// Determines if an API error is a TPM/RPM limit exceeded (429).
fn is_rate_limit_error(err_msg: &str) -> bool {
    err_msg.contains("429")
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p rwd test_is_context_limit -- --nocapture && cargo test -p rwd test_is_rate_limit -- --nocapture`
Expected: all 6 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/analyzer/mod.rs
git commit -m "feat: add token limit error detection functions (#36)"
```

---

### Task 2: merge_results test and implementation

**Files:**
- Modify: `src/analyzer/insight.rs`

- [ ] **Step 1: Write merge_results tests**

Add to the `mod tests` block in `src/analyzer/insight.rs`:

```rust
    #[test]
    fn test_merge_results_combines_multiple_results() {
        let r1 = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "s1".to_string(),
                work_summary: "work1".to_string(),
                decisions: vec![],
                curiosities: vec![],
                corrections: vec![],
                til: vec![],
            }],
        };
        let r2 = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "s2".to_string(),
                work_summary: "work2".to_string(),
                decisions: vec![],
                curiosities: vec![],
                corrections: vec![],
                til: vec![],
            }],
        };
        let merged = merge_results(vec![r1, r2]);
        assert_eq!(merged.sessions.len(), 2);
        assert_eq!(merged.sessions[0].session_id, "s1");
        assert_eq!(merged.sessions[1].session_id, "s2");
    }

    #[test]
    fn test_merge_results_empty_vec_returns_empty_result() {
        let merged = merge_results(vec![]);
        assert!(merged.sessions.is_empty());
    }
```

- [ ] **Step 2: Verify test failure**

Run: `cargo test -p rwd test_merge_results -- --nocapture`
Expected: compile error — function doesn't exist

- [ ] **Step 3: Implement merge_results**

Add below the `parse_response` function in `src/analyzer/insight.rs`:

```rust
/// Merges multiple AnalysisResults into one.
/// Combines the sessions Vec from each result in order.
/// Used to compose a single result from per-session fallback analysis.
pub fn merge_results(results: Vec<AnalysisResult>) -> AnalysisResult {
    let sessions = results
        .into_iter()
        .flat_map(|r| r.sessions)
        .collect();
    AnalysisResult { sessions }
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p rwd test_merge_results -- --nocapture`
Expected: both tests PASS

- [ ] **Step 5: Update module comment**

Change line 1 comment of `src/analyzer/insight.rs`:

```rust
// Module for parsing LLM API responses into structured insight types and merging split analysis results.
```

- [ ] **Step 6: Commit**

```bash
git add src/analyzer/insight.rs
git commit -m "feat: add merge_results function for result combining (#36)"
```

---

### Task 3: Add Clone derive to LogEntry (preparation)

**Files:**
- Modify: `src/parser/claude.rs`

- [ ] **Step 1: Add Clone derive**

Add `Clone` to the following types in `src/parser/claude.rs` (alongside existing `Debug, Deserialize`):

- `LogEntry` (enum, line 29)
- `UserEntry` (line 45)
- `AssistantEntry` (line 58)
- `AssistantMessage` (around line 68)
- `ContentBlock` (enum)
- `Usage` (struct)
- `ProgressEntry`
- `SystemEntry`
- `FileHistorySnapshotEntry`

`serde_json::Value`, `String`, `DateTime<Utc>` and other base types already implement `Clone`, so just adding the derive is sufficient.

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: compilation success

- [ ] **Step 3: Commit**

```bash
git add src/parser/claude.rs
git commit -m "chore: add Clone derive to LogEntry related types (#36)"
```

---

### Task 4: extract_session_ids test and implementation (formerly Task 3)

**Files:**
- Modify: `src/analyzer/prompt.rs`

- [ ] **Step 1: Write extract_session_ids tests**

Add to the `mod tests` block in `src/analyzer/prompt.rs`:

```rust
    #[test]
    fn test_extract_session_ids_deduplicates_preserving_order() {
        let entries = vec![
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:00:00Z","uuid":"u1","message":{"role":"user","content":"first"}}"#,
            ).unwrap(),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s2","timestamp":"2026-03-11T11:00:00Z","uuid":"u2","message":{"role":"user","content":"second"}}"#,
            ).unwrap(),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T12:00:00Z","uuid":"u3","message":{"role":"user","content":"third"}}"#,
            ).unwrap(),
        ];
        let ids = extract_session_ids(&entries);
        assert_eq!(ids, vec!["s1".to_string(), "s2".to_string()]);
    }

    #[test]
    fn test_extract_session_ids_empty_entries_returns_empty() {
        let entries: Vec<LogEntry> = vec![];
        let ids = extract_session_ids(&entries);
        assert!(ids.is_empty());
    }
```

- [ ] **Step 2: Verify test failure**

Run: `cargo test -p rwd test_extract_session_ids -- --nocapture`
Expected: compile error — function doesn't exist

- [ ] **Step 3: Implement extract_session_ids**

Add below the `build_codex_prompt` function in `src/analyzer/prompt.rs`:

```rust
/// Extracts unique session IDs from a LogEntry slice.
/// Preserves insertion order while removing duplicates.
/// Used for splitting entries by session during fallback.
pub fn extract_session_ids(entries: &[LogEntry]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in entries {
        // User/Assistant/Progress have session_id: String
        // System has session_id: Option<String>
        // FileHistorySnapshot has no session_id field
        let id = match entry {
            LogEntry::User(e) => Some(e.session_id.as_str()),
            LogEntry::Assistant(e) => Some(e.session_id.as_str()),
            LogEntry::Progress(e) => Some(e.session_id.as_str()),
            LogEntry::System(e) => e.session_id.as_deref(),
            LogEntry::FileHistorySnapshot(_) | LogEntry::Other(_) => None,
        };
        if let Some(session_id) = id {
            if seen.insert(session_id.to_string()) {
                ids.push(session_id.to_string());
            }
        }
    }
    ids
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p rwd test_extract_session_ids -- --nocapture`
Expected: both tests PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy`
Expected: 0 warnings

- [ ] **Step 6: Commit**

```bash
git add src/analyzer/prompt.rs
git commit -m "feat: add extract_session_ids function (#36)"
```

---

### Task 5: Implement analyze_entries fallback logic

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: Add fallback logic to analyze_entries**

Replace `analyze_entries()` in `src/analyzer/mod.rs` with:

```rust
pub async fn analyze_entries(
    entries: &[LogEntry],
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;
    let prompt_text = prompt::build_prompt(entries)?;
    let (final_prompt, redact_result) = if redactor_enabled {
        crate::redactor::redact_text(&prompt_text)
    } else {
        (prompt_text, RedactResult::empty())
    };

    match provider.call_api(&api_key, &final_prompt).await {
        Ok(raw_response) => {
            let result = insight::parse_response(&raw_response)?;
            Ok((result, redact_result))
        }
        Err(e) => {
            let err_msg = e.to_string();

            // 429 TPM limit → friendly error message
            if is_rate_limit_error(&err_msg) {
                return Err(
                    "API request rate (TPM) limit exceeded.\n\
                     Solutions:\n  \
                     • rwd config provider anthropic  (switch to Anthropic)\n  \
                     • Upgrade your LLM provider plan  (increase TPM limit)"
                        .into(),
                );
            }

            // 400 context limit → per-session split fallback
            if is_context_limit_error(&err_msg) {
                eprintln!("Prompt exceeds token limit, switching to per-session analysis...");
                return analyze_entries_by_session(
                    entries,
                    &provider,
                    &api_key,
                    redactor_enabled,
                )
                .await;
            }

            // Other errors → propagate as-is
            Err(e)
        }
    }
}
```

- [ ] **Step 2: Implement analyze_entries_by_session function**

Add below the `analyze_entries` function:

```rust
/// Splits entries by session for individual analysis and merges results.
/// Called as a fallback when a 400 context overflow error occurs.
async fn analyze_entries_by_session(
    entries: &[LogEntry],
    provider: &provider::LlmProvider,
    api_key: &str,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let session_ids = prompt::extract_session_ids(entries);
    let total = session_ids.len();
    let mut results = Vec::new();
    let mut total_redact = RedactResult::empty();

    for (i, session_id) in session_ids.iter().enumerate() {
        eprintln!("  Analyzing session {}/{total}... ({session_id})", i + 1);

        // Filter entries for this session into a new Vec.
        // clone is needed because build_prompt() takes &[LogEntry], requiring an owned Vec.
        let session_entries: Vec<LogEntry> = entries
            .iter()
            .filter(|e| entry_session_id(e) == Some(session_id.as_str()))
            .cloned()
            .collect();

        if session_entries.is_empty() {
            continue;
        }

        let prompt_text = match prompt::build_prompt(&session_entries) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("  Session {session_id} prompt generation failed: {e}");
                continue;
            }
        };

        let (final_prompt, redact_result) = if redactor_enabled {
            crate::redactor::redact_text(&prompt_text)
        } else {
            (prompt_text, RedactResult::empty())
        };
        total_redact.merge(redact_result);

        match provider.call_api(api_key, &final_prompt).await {
            Ok(raw_response) => {
                match insight::parse_response(&raw_response) {
                    Ok(result) => results.push(result),
                    Err(e) => eprintln!("  Session {session_id} response parsing failed: {e}"),
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                if is_context_limit_error(&err_msg) || is_rate_limit_error(&err_msg) {
                    eprintln!("  Session {session_id} skipped (token limit exceeded)");
                } else {
                    eprintln!("  Session {session_id} analysis failed: {err_msg}");
                }
            }
        }
    }

    if results.is_empty() {
        return Err("All session analyses failed.".into());
    }

    Ok((insight::merge_results(results), total_redact))
}

/// Extracts the session_id from a LogEntry.
/// SystemEntry has Option<String>, FileHistorySnapshotEntry has no session_id.
fn entry_session_id(entry: &LogEntry) -> Option<&str> {
    match entry {
        LogEntry::User(e) => Some(&e.session_id),
        LogEntry::Assistant(e) => Some(&e.session_id),
        LogEntry::Progress(e) => Some(&e.session_id),
        LogEntry::System(e) => e.session_id.as_deref(),
        LogEntry::FileHistorySnapshot(_) | LogEntry::Other(_) => None,
    }
}
```

- [ ] **Step 4: Verify build**

Run: `cargo build`
Expected: compilation success

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: all tests PASS

- [ ] **Step 6: Run clippy**

Run: `cargo clippy`
Expected: 0 warnings

- [ ] **Step 7: Commit**

```bash
git add src/analyzer/mod.rs src/parser/claude.rs
git commit -m "feat: implement analyze_entries token limit fallback (#36)"
```
