# Codex Session Parser Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse Codex CLI session logs and integrate them into the same insight analysis pipeline as Claude Code.

**Architecture:** Add a `parser::codex` module in parallel with `parser::claude`. Since Codex JSONL has a nested payload structure, two-stage parsing (loose parse → structured enum conversion) is used. The analyzer branches via a separate `analyze_codex_entries()` function, and output is organized into per-agent sections within the same date file.

**Tech Stack:** Rust, serde, serde_json, chrono

---

## File Structure

| File | Change | Role |
|------|--------|------|
| `src/parser/codex.rs` | Create | Codex JSONL parsing, entry types, summary |
| `src/parser/mod.rs` | Modify | Add `pub mod codex;` |
| `src/analyzer/prompt.rs` | Modify | Add `build_codex_prompt()` |
| `src/analyzer/mod.rs` | Modify | Add `analyze_codex_entries()` |
| `src/output/markdown.rs` | Modify | Multi-source section rendering |
| `src/output/mod.rs` | Modify | Add `render_combined_markdown` re-export |
| `src/main.rs` | Modify | Codex collection/analysis integration, remove existing `save_analysis` |

**Behavior change:** Even if the Claude directory (`~/.claude/projects/`) doesn't exist, the program no longer aborts with an error — it proceeds with an empty result. This supports Codex-only users.

---

## Chunk 1: parser::codex Module

### Task 1: Codex entry type definition and basic parsing

**Files:**
- Create: `src/parser/codex.rs`
- Modify: `src/parser/mod.rs`

- [ ] **Step 1: Add codex module declaration to `parser/mod.rs`**

```rust
// src/parser/mod.rs — append to existing code
pub mod codex;
```

- [ ] **Step 2: Define Codex entry types — test first**

Create `src/parser/codex.rs`. Write the test first.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_session_meta_entry() {
        let json = r#"{"timestamp":"2026-03-11T03:28:22.017Z","type":"session_meta","payload":{"id":"abc-123","timestamp":"2026-03-11T03:28:04.550Z","cwd":"/Users/test/project","model_provider":"openai","cli_version":"0.114.0","source":"cli"}}"#;
        let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
        assert_eq!(raw.entry_type, "session_meta");

        let entry = CodexEntry::from_raw(&raw);
        assert!(matches!(entry, CodexEntry::SessionMeta { .. }));
        if let CodexEntry::SessionMeta { session_id, cwd, .. } = &entry {
            assert_eq!(session_id, "abc-123");
            assert_eq!(cwd, "/Users/test/project");
        }
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p rwd parser::codex::tests::test_parse_session_meta_entry`
Expected: FAIL — CodexRawEntry, CodexEntry not defined

- [ ] **Step 4: Implement minimal types — session_meta only**

```rust
// Codex CLI session log (.jsonl) parser
//
// Unlike Claude Code, Codex JSONL has a nested payload structure.
// Each line: {"timestamp": "...", "type": "...", "payload": {...}}
// Two-stage parsing: CodexRawEntry (loose) → CodexEntry (structured).

#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use std::io::BufRead;
use std::path::{Path, PathBuf};

// === Stage 1: Loose parsing struct ===

/// Loosely parses a single JSONL line.
/// Receives payload as serde_json::Value for type-specific conversion in stage 2.
#[derive(Debug, Deserialize)]
pub struct CodexRawEntry {
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "type")]
    pub entry_type: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

// === Stage 2: Structured entries ===

/// Enum of meaningful entries extracted from Codex logs.
/// Corresponds to Claude Code's LogEntry but reflects Codex-specific structure.
#[derive(Debug)]
pub enum CodexEntry {
    SessionMeta {
        timestamp: DateTime<Utc>,
        session_id: String,
        cwd: String,
        model_provider: String,
    },
    UserMessage {
        timestamp: DateTime<Utc>,
        text: String,
    },
    AssistantMessage {
        timestamp: DateTime<Utc>,
        text: String,
    },
    FunctionCall {
        timestamp: DateTime<Utc>,
        name: String,
    },
    Other,
}

impl CodexEntry {
    /// Converts a CodexRawEntry into a structured CodexEntry.
    /// Inspects the payload structure to select the appropriate variant.
    pub fn from_raw(raw: &CodexRawEntry) -> Self {
        match raw.entry_type.as_str() {
            "session_meta" => {
                let id = raw.payload.get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let cwd = raw.payload.get("cwd")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let model_provider = raw.payload.get("model_provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                CodexEntry::SessionMeta {
                    timestamp: raw.timestamp,
                    session_id: id,
                    cwd,
                    model_provider,
                }
            }
            _ => CodexEntry::Other,
        }
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p rwd parser::codex::tests::test_parse_session_meta_entry`
Expected: PASS

- [ ] **Step 6: user_message parsing test**

```rust
#[test]
fn test_parse_user_message_from_event_msg() {
    let json = r#"{"timestamp":"2026-03-11T03:28:22.019Z","type":"event_msg","payload":{"type":"user_message","message":"Create an AGENTS.md file"}}"#;
    let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
    let entry = CodexEntry::from_raw(&raw);
    assert!(matches!(entry, CodexEntry::UserMessage { .. }));
    if let CodexEntry::UserMessage { text, .. } = &entry {
        assert!(text.contains("AGENTS.md"));
    }
}
```

- [ ] **Step 7: Add event_msg → UserMessage branch to from_raw**

Add to the `from_raw()` match:

```rust
"event_msg" => {
    let msg_type = raw.payload.get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match msg_type {
        "user_message" => {
            let text = raw.payload.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            CodexEntry::UserMessage {
                timestamp: raw.timestamp,
                text,
            }
        }
        _ => CodexEntry::Other,
    }
}
```

- [ ] **Step 8: Run test to verify it passes**

Run: `cargo test -p rwd parser::codex::tests::test_parse_user_message`
Expected: PASS

- [ ] **Step 9: assistant message parsing test**

```rust
#[test]
fn test_parse_assistant_message_from_response_item() {
    let json = r#"{"timestamp":"2026-03-11T03:28:29.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Checked the README."}]}}"#;
    let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
    let entry = CodexEntry::from_raw(&raw);
    assert!(matches!(entry, CodexEntry::AssistantMessage { .. }));
    if let CodexEntry::AssistantMessage { text, .. } = &entry {
        assert!(text.contains("README"));
    }
}
```

- [ ] **Step 10: Add response_item → AssistantMessage branch to from_raw**

```rust
"response_item" => {
    let item_type = raw.payload.get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let role = raw.payload.get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match (item_type, role) {
        ("message", "assistant") => {
            let text = extract_codex_output_text(&raw.payload);
            if text.is_empty() {
                CodexEntry::Other
            } else {
                CodexEntry::AssistantMessage {
                    timestamp: raw.timestamp,
                    text,
                }
            }
        }
        ("function_call", _) => {
            let name = raw.payload.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            CodexEntry::FunctionCall {
                timestamp: raw.timestamp,
                name,
            }
        }
        _ => CodexEntry::Other,
    }
}
```

Helper function:

```rust
/// Extracts output_text from a response_item payload's content array.
fn extract_codex_output_text(payload: &serde_json::Value) -> String {
    let Some(content) = payload.get("content").and_then(|v| v.as_array()) else {
        return String::new();
    };
    content.iter()
        .filter_map(|block| {
            if block.get("type")?.as_str()? == "output_text" {
                block.get("text")?.as_str()
            } else {
                None
            }
        })
        .collect::<Vec<&str>>()
        .join("\n")
}
```

- [ ] **Step 11: function_call parsing test**

```rust
#[test]
fn test_parse_function_call_from_response_item() {
    let json = r#"{"timestamp":"2026-03-11T03:28:29.000Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":"call_123"}}"#;
    let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
    let entry = CodexEntry::from_raw(&raw);
    assert!(matches!(entry, CodexEntry::FunctionCall { .. }));
    if let CodexEntry::FunctionCall { name, .. } = &entry {
        assert_eq!(name, "exec_command");
    }
}
```

- [ ] **Step 12: unknown entry resilience test**

```rust
#[test]
fn test_parse_unknown_entry_returns_other() {
    let json = r#"{"timestamp":"2026-03-11T03:28:22.000Z","type":"turn_context","payload":{}}"#;
    let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
    let entry = CodexEntry::from_raw(&raw);
    assert!(matches!(entry, CodexEntry::Other));
}
```

- [ ] **Step 13: Run all codex tests**

Run: `cargo test -p rwd parser::codex`
Expected: ALL PASS

- [ ] **Step 14: cargo clippy**

Run: `cargo clippy -p rwd`
Expected: 0 warnings

- [ ] **Step 15: Commit**

```bash
git add src/parser/codex.rs src/parser/mod.rs
git commit -m "feat: parser::codex entry type definitions and basic parsing"
```

---

### Task 2: Codex file discovery and JSONL parsing

**Files:**
- Modify: `src/parser/codex.rs`

- [ ] **Step 1: discover_codex_sessions_dir test**

```rust
#[test]
fn test_discover_codex_sessions_dir_returns_path() {
    // Verify based on whether ~/.codex/sessions/ actually exists
    let result = discover_codex_sessions_dir();
    // May not exist in CI environments, so just check path format
    if let Ok(path) = result {
        assert!(path.ends_with("sessions"));
    }
}
```

- [ ] **Step 2: Implement discover_codex_sessions_dir**

```rust
/// Returns the ~/.codex/sessions/ directory path.
/// Codex stores sessions under YYYY/MM/DD/ subdirectories.
pub fn discover_codex_sessions_dir() -> Result<PathBuf, super::ParseError> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let sessions_dir = home.join(".codex").join("sessions");
    if !sessions_dir.exists() {
        return Err(format!(
            "Codex sessions directory not found: {}",
            sessions_dir.display()
        ).into());
    }
    Ok(sessions_dir)
}
```

- [ ] **Step 3: list_session_files_for_date test**

```rust
#[test]
fn test_list_session_files_for_date_with_temp_dir() {
    let temp = std::env::temp_dir().join("rwd_codex_test");
    let date_dir = temp.join("2026").join("03").join("11");
    std::fs::create_dir_all(&date_dir).unwrap();

    let test_file = date_dir.join("rollout-2026-03-11T10-00-00-abc123.jsonl");
    std::fs::write(&test_file, "").unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 3, 11).unwrap();
    let files = list_session_files_for_date(&temp, date).unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].extension().unwrap() == "jsonl");

    // Cleanup
    std::fs::remove_dir_all(&temp).ok();
}
```

- [ ] **Step 4: Implement list_session_files_for_date**

```rust
/// Returns Codex session files for the given date.
/// Codex stores sessions in a ~/.codex/sessions/YYYY/MM/DD/ structure,
/// so date filtering is done via directory path.
pub fn list_session_files_for_date(
    sessions_dir: &Path,
    date: NaiveDate,
) -> Result<Vec<PathBuf>, super::ParseError> {
    let date_dir = sessions_dir
        .join(format!("{}", date.format("%Y")))
        .join(format!("{}", date.format("%m")))
        .join(format!("{}", date.format("%d")));

    if !date_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(&date_dir)? {
        let path = entry?.path();
        if path.is_file()
            && let Some(ext) = path.extension()
            && ext == "jsonl"
        {
            files.push(path);
        }
    }
    Ok(files)
}
```

- [ ] **Step 5: parse_codex_jsonl_file test**

```rust
#[test]
fn test_parse_codex_jsonl_file_with_mixed_entries() {
    let temp = std::env::temp_dir().join("rwd_codex_parse_test");
    std::fs::create_dir_all(&temp).unwrap();
    let file = temp.join("test.jsonl");

    let content = [
        r#"{"timestamp":"2026-03-11T03:28:22.017Z","type":"session_meta","payload":{"id":"abc-123","cwd":"/test","model_provider":"openai"}}"#,
        r#"{"timestamp":"2026-03-11T03:28:22.019Z","type":"event_msg","payload":{"type":"user_message","message":"hello"}}"#,
        r#"{"timestamp":"2026-03-11T03:28:29.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"world"}]}}"#,
    ].join("\n");
    std::fs::write(&file, content).unwrap();

    let entries = parse_codex_jsonl_file(&file).unwrap();
    let meaningful: Vec<&CodexEntry> = entries.iter()
        .filter(|e| !matches!(e, CodexEntry::Other))
        .collect();
    assert_eq!(meaningful.len(), 3);

    std::fs::remove_dir_all(&temp).ok();
}
```

- [ ] **Step 6: Implement parse_codex_jsonl_file**

```rust
/// Reads a Codex JSONL file and converts it to a CodexEntry vector.
/// Same pattern as the Claude parser: reads line by line, skips failed lines.
pub fn parse_codex_jsonl_file(path: &Path) -> Result<Vec<CodexEntry>, super::ParseError> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<CodexRawEntry>(&line) {
            Ok(raw) => entries.push(CodexEntry::from_raw(&raw)),
            Err(err) => {
                eprintln!(
                    "Warning: Failed to parse Codex line {} in {}: {}",
                    line_num + 1,
                    path.display(),
                    err
                );
            }
        }
    }
    Ok(entries)
}
```

- [ ] **Step 7: Run all tests + clippy**

Run: `cargo test -p rwd parser::codex && cargo clippy -p rwd`
Expected: ALL PASS, 0 warnings

- [ ] **Step 8: Commit**

```bash
git add src/parser/codex.rs
git commit -m "feat: Codex file discovery and JSONL parsing implementation"
```

---

### Task 3: Codex session summary

**Files:**
- Modify: `src/parser/codex.rs`

- [ ] **Step 1: CodexSessionSummary test**

```rust
#[test]
fn test_summarize_codex_entries_counts_correctly() {
    let entries = vec![
        CodexEntry::SessionMeta {
            timestamp: "2026-03-11T03:28:22Z".parse().unwrap(),
            session_id: "abc-123".to_string(),
            cwd: "/test".to_string(),
            model_provider: "openai".to_string(),
        },
        CodexEntry::UserMessage {
            timestamp: "2026-03-11T03:28:22Z".parse().unwrap(),
            text: "hello".to_string(),
        },
        CodexEntry::UserMessage {
            timestamp: "2026-03-11T03:29:00Z".parse().unwrap(),
            text: "do something".to_string(),
        },
        CodexEntry::AssistantMessage {
            timestamp: "2026-03-11T03:28:30Z".parse().unwrap(),
            text: "world".to_string(),
        },
        CodexEntry::FunctionCall {
            timestamp: "2026-03-11T03:28:29Z".parse().unwrap(),
            name: "exec_command".to_string(),
        },
    ];
    let summary = summarize_codex_entries(&entries);
    assert_eq!(summary.session_id, "abc-123");
    assert_eq!(summary.user_count, 2);
    assert_eq!(summary.assistant_count, 1);
    assert_eq!(summary.function_call_count, 1);
}
```

- [ ] **Step 2: Implement CodexSessionSummary**

```rust
/// Summary information for a Codex session.
/// Corresponds to Claude's SessionSummary, but includes cwd/model_provider instead of token info.
#[derive(Debug)]
pub struct CodexSessionSummary {
    pub session_id: String,
    pub cwd: String,
    pub model_provider: String,
    pub user_count: usize,
    pub assistant_count: usize,
    pub function_call_count: usize,
}

/// Summarizes Codex entries.
/// Since one file equals one session in Codex, the ID comes from SessionMeta.
pub fn summarize_codex_entries(entries: &[CodexEntry]) -> CodexSessionSummary {
    let mut summary = CodexSessionSummary {
        session_id: String::new(),
        cwd: String::new(),
        model_provider: String::new(),
        user_count: 0,
        assistant_count: 0,
        function_call_count: 0,
    };

    for entry in entries {
        match entry {
            CodexEntry::SessionMeta {
                session_id, cwd, model_provider, ..
            } => {
                summary.session_id = session_id.clone();
                summary.cwd = cwd.clone();
                summary.model_provider = model_provider.clone();
            }
            CodexEntry::UserMessage { .. } => summary.user_count += 1,
            CodexEntry::AssistantMessage { .. } => summary.assistant_count += 1,
            CodexEntry::FunctionCall { .. } => summary.function_call_count += 1,
            CodexEntry::Other => {}
        }
    }
    summary
}
```

- [ ] **Step 3: Run tests + clippy**

Run: `cargo test -p rwd parser::codex && cargo clippy -p rwd`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add src/parser/codex.rs
git commit -m "feat: Codex session summary (CodexSessionSummary) implementation"
```

---

## Chunk 2: Analyzer + Output + Main Integration

### Task 4: Codex prompt builder

**Files:**
- Modify: `src/analyzer/prompt.rs`

- [ ] **Step 1: build_codex_prompt test**

```rust
#[test]
fn test_build_codex_prompt_extracts_conversation() {
    let entries = vec![
        CodexEntry::UserMessage {
            timestamp: "2026-03-11T10:00:00Z".parse().unwrap(),
            text: "Show me the project structure".to_string(),
        },
        CodexEntry::AssistantMessage {
            timestamp: "2026-03-11T10:00:30Z".parse().unwrap(),
            text: "Checked the src/ directory".to_string(),
        },
    ];
    let prompt = build_codex_prompt(&entries, "test-session").unwrap();
    assert!(prompt.contains("[USER] Show me the project structure"));
    assert!(prompt.contains("[ASSISTANT] Checked the src/ directory"));
    assert!(prompt.contains("[Session: test-session]"));
}

#[test]
fn test_build_codex_prompt_empty_entries_returns_error() {
    let entries: Vec<CodexEntry> = vec![];
    let result = build_codex_prompt(&entries, "s1");
    assert!(result.is_err());
}
```

- [ ] **Step 2: Implement build_codex_prompt**

Add import at the top of `src/analyzer/prompt.rs`:

```rust
use crate::parser::codex::CodexEntry;
```

Add function:

```rust
/// Converts Codex entries into conversation text for LLM analysis.
/// Since one file equals one session in Codex, the session_id is passed externally.
pub fn build_codex_prompt(
    entries: &[CodexEntry],
    session_id: &str,
) -> Result<String, super::AnalyzerError> {
    let mut output = format!("[Session: {session_id}]\n");

    for entry in entries {
        match entry {
            CodexEntry::UserMessage { text, .. } => {
                output.push_str(&format!("[USER] {text}\n"));
            }
            CodexEntry::AssistantMessage { text, .. } => {
                output.push_str(&format!("[ASSISTANT] {text}\n"));
            }
            _ => {}
        }
    }

    // If only the session header exists with no conversation content
    if !output.contains("[USER]") && !output.contains("[ASSISTANT]") {
        return Err("No conversation content found in Codex log.".into());
    }

    Ok(output)
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p rwd analyzer::prompt`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add src/analyzer/prompt.rs
git commit -m "feat: add Codex conversation text prompt builder"
```

---

### Task 5: analyze_codex_entries function

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: Add analyze_codex_entries**

```rust
use crate::parser::codex::CodexEntry;

/// Analyzes Codex session entries to extract insights.
/// Same pipeline as Claude's analyze_entries() but uses the Codex-specific prompt.
pub async fn analyze_codex_entries(
    entries: &[CodexEntry],
    session_id: &str,
) -> Result<AnalysisResult, AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;
    let prompt_text = prompt::build_codex_prompt(entries, session_id)?;
    let raw_response = provider.call_api(&api_key, &prompt_text).await?;
    let result = insight::parse_response(&raw_response)?;
    Ok(result)
}
```

- [ ] **Step 2: Run build + clippy**

Run: `cargo build -p rwd && cargo clippy -p rwd`
Expected: success, 0 warnings

- [ ] **Step 3: Commit**

```bash
git add src/analyzer/mod.rs
git commit -m "feat: add analyze_codex_entries() analysis function"
```

---

### Task 6: Multi-source Markdown rendering

**Files:**
- Modify: `src/output/markdown.rs`

- [ ] **Step 1: render_combined_markdown test**

```rust
#[test]
fn test_render_combined_markdown_separates_two_sources() {
    let claude = AnalysisResult {
        sessions: vec![SessionInsight {
            session_id: "c1".to_string(),
            work_summary: "Claude work".to_string(),
            decisions: vec![],
            curiosities: vec![],
            corrections: vec![],
        }],
    };
    let codex = AnalysisResult {
        sessions: vec![SessionInsight {
            session_id: "x1".to_string(),
            work_summary: "Codex work".to_string(),
            decisions: vec![],
            curiosities: vec![],
            corrections: vec![],
        }],
    };
    let sources = vec![
        ("Claude Code", &claude),
        ("Codex", &codex),
    ];
    let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
    let md = render_combined_markdown(&sources, date);

    assert!(md.contains("## Claude Code"));
    assert!(md.contains("## Codex"));
    assert!(md.contains("Claude work"));
    assert!(md.contains("Codex work"));
}

#[test]
fn test_render_combined_markdown_single_source_works() {
    let claude = AnalysisResult {
        sessions: vec![SessionInsight {
            session_id: "c1".to_string(),
            work_summary: "Claude work".to_string(),
            decisions: vec![],
            curiosities: vec![],
            corrections: vec![],
        }],
    };
    let sources = vec![("Claude Code", &claude)];
    let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
    let md = render_combined_markdown(&sources, date);

    assert!(md.contains("## Claude Code"));
    assert!(md.contains("Claude work"));
}
```

- [ ] **Step 2: Implement render_combined_markdown**

```rust
/// Combines analysis results from multiple sources into a single Markdown document.
/// Each source is separated by a ## heading.
///
/// sources: slice of (source name, analysis result) tuples.
/// When adding new agents in the future, just append to sources.
pub fn render_combined_markdown(
    sources: &[(&str, &AnalysisResult)],
    date: NaiveDate,
) -> String {
    let mut md = String::new();
    md.push_str(&format!("# {date} Dev Session Review\n\n"));

    let mut all_til_items: Vec<String> = Vec::new();

    for (source_name, analysis) in sources {
        md.push_str(&format!("## {source_name}\n\n"));
        for session in &analysis.sessions {
            render_session(&mut md, session, &mut all_til_items);
        }
    }

    render_til_section(&mut md, &all_til_items);
    md
}
```

- [ ] **Step 3: Add re-export to `output/mod.rs`**

Add to `src/output/mod.rs`:

```rust
pub use markdown::render_combined_markdown;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p rwd output::markdown`
Expected: ALL PASS (existing + new tests)

- [ ] **Step 5: Commit**

```bash
git add src/output/markdown.rs src/output/mod.rs
git commit -m "feat: multi-source Markdown rendering (render_combined_markdown)"
```

---

### Task 7: main.rs integration

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Integrate Codex collection/analysis into run_today()**

Modify `run_today()`:

```rust
async fn run_today() -> Result<(), parser::ParseError> {
    update::notify_if_update_available().await;

    if config::load_config_if_exists().is_none() {
        eprintln!("Config file not found. Please run `rwd init` first.");
        std::process::exit(1);
    }

    let today = chrono::Utc::now().date_naive();

    // === Collect Claude Code logs ===
    let claude_entries = collect_claude_entries(today);

    // === Collect Codex logs ===
    let codex_sessions = collect_codex_sessions(today);

    if claude_entries.is_empty() && codex_sessions.is_empty() {
        println!("No log entries found for today ({today}).");
        return Ok(());
    }

    // === Claude Code summary output ===
    if !claude_entries.is_empty() {
        let summaries = parser::summarize_entries(&claude_entries);
        println!("\n=== Claude Code ({today}) ===");
        println!("Sessions: {}", summaries.len());
        for s in &summaries {
            print_claude_summary(s);
        }
    }

    // === Codex summary output ===
    if !codex_sessions.is_empty() {
        println!("\n=== Codex ({today}) ===");
        println!("Sessions: {}", codex_sessions.len());
        for (summary, _) in &codex_sessions {
            print_codex_summary(summary);
        }
    }

    // === LLM Analysis ===
    let provider_label = analyzer::provider::load_provider()
        .map(|(p, _)| p.display_name().to_string())
        .unwrap_or_else(|_| "LLM".to_string());
    println!("\nAnalyzing insights with {provider_label} API...");

    let mut sources: Vec<(&str, analyzer::AnalysisResult)> = Vec::new();

    // Claude analysis
    if !claude_entries.is_empty() {
        match analyzer::analyze_entries(&claude_entries).await {
            Ok(result) => sources.push(("Claude Code", result)),
            Err(e) => eprintln!("Claude Code analysis failed: {e}"),
        }
    }

    // Codex analysis — individual per-session analysis
    for (summary, entries) in &codex_sessions {
        match analyzer::analyze_codex_entries(entries, &summary.session_id).await {
            Ok(result) => sources.push(("Codex", result)),
            Err(e) => eprintln!("Codex analysis failed ({}): {e}", &summary.session_id[..8.min(summary.session_id.len())]),
        }
    }

    // Output and save results
    if !sources.is_empty() {
        for (name, analysis) in &sources {
            println!("\n=== {name} Insights ===");
            print_insights(analysis);
        }
        save_combined_analysis(&sources, today);
    }

    Ok(())
}
```

- [ ] **Step 2: Add helper functions**

```rust
/// Collects Claude Code logs. Returns empty Vec if directory doesn't exist.
/// Previously run_today() would abort on missing directory,
/// but now proceeds with empty results to support Codex-only users.
fn collect_claude_entries(today: chrono::NaiveDate) -> Vec<parser::claude::LogEntry> {
    match parser::discover_log_dir() {
        Ok(dir) => println!("Scanning Claude Code: {}", dir.display()),
        Err(_) => return Vec::new(),
    }

    let mut all_entries = Vec::new();
    if let Ok(project_dirs) = parser::list_project_dirs() {
        for project_dir in project_dirs {
            if let Ok(session_files) = parser::list_session_files(&project_dir) {
                for session_file in session_files {
                    if let Ok(entries) = parser::parse_jsonl_file(&session_file) {
                        let today_entries = parser::filter_entries_by_date(entries, today);
                        all_entries.extend(today_entries);
                    }
                }
            }
        }
    }
    all_entries
}

/// Collects Codex session logs. Returns empty Vec if directory doesn't exist.
fn collect_codex_sessions(
    today: chrono::NaiveDate,
) -> Vec<(parser::codex::CodexSessionSummary, Vec<parser::codex::CodexEntry>)> {
    let sessions_dir = match parser::codex::discover_codex_sessions_dir() {
        Ok(dir) => {
            println!("Scanning Codex: {}", dir.display());
            dir
        }
        Err(_) => return Vec::new(),
    };

    let session_files = match parser::codex::list_session_files_for_date(&sessions_dir, today) {
        Ok(files) => files,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();
    for file in session_files {
        if let Ok(entries) = parser::codex::parse_codex_jsonl_file(&file) {
            let summary = parser::codex::summarize_codex_entries(&entries);
            // Only include sessions with conversation content
            if summary.user_count > 0 || summary.assistant_count > 0 {
                sessions.push((summary, entries));
            }
        }
    }
    sessions
}

fn print_claude_summary(s: &parser::claude::SessionSummary) {
    let total_in = s.total_input_tokens
        + s.total_cache_creation_tokens
        + s.total_cache_read_tokens;
    println!("\nSession: {}...", &s.session_id[..8]);
    println!("  User messages:      {}", s.user_count);
    println!("  Assistant messages:  {}", s.assistant_count);
    println!("  Tool uses:          {}", s.tool_use_count);
    println!("  Tokens (in/out):    {}/{}", total_in, s.total_output_tokens);
}

fn print_codex_summary(s: &parser::codex::CodexSessionSummary) {
    let id_display = if s.session_id.len() >= 8 {
        &s.session_id[..8]
    } else {
        &s.session_id
    };
    println!("\nSession: {id_display}...");
    println!("  Project:            {}", s.cwd);
    println!("  User messages:      {}", s.user_count);
    println!("  Assistant messages:  {}", s.assistant_count);
    println!("  Function calls:     {}", s.function_call_count);
}

/// Combines analysis results from multiple sources and saves as Markdown.
fn save_combined_analysis(
    sources: &[(&str, analyzer::AnalysisResult)],
    date: chrono::NaiveDate,
) {
    let vault_path = match output::load_vault_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to load vault path: {e}");
            return;
        }
    };

    // Convert to (&str, &AnalysisResult) slice
    let source_refs: Vec<(&str, &analyzer::AnalysisResult)> = sources
        .iter()
        .map(|(name, result)| (*name, result))
        .collect();

    let markdown = output::markdown::render_combined_markdown(&source_refs, date);

    match output::save_to_vault(&vault_path, date, &markdown) {
        Ok(saved) => println!("\nMarkdown saved: {}", saved.display()),
        Err(e) => eprintln!("File save failed: {e}"),
    }
}
```

- [ ] **Step 3: Clean up existing `save_analysis` function and `render_markdown` re-export**

Delete the existing `save_analysis()` function from `src/main.rs` (replaced by save_combined_analysis).
Delete `pub use markdown::render_markdown;` from `src/output/mod.rs` (replaced by render_combined_markdown).

> Note: The `render_markdown` function itself is NOT deleted — it shares the module with `render_session` and `render_til_section` used internally by `render_combined_markdown`. Only the external re-export is removed.

- [ ] **Step 4: Run build + all tests + clippy**

Run: `cargo build -p rwd && cargo test -p rwd && cargo clippy -p rwd`
Expected: ALL PASS, 0 warnings

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/output/mod.rs
git commit -m "feat: integrate Codex session collection/analysis into run_today()"
```

---

### Task 8: Documentation update

**Files:**
- Modify: `docs/ARCHITECTURE.md`

- [ ] **Step 1: Update ARCHITECTURE.md with Codex source info**

Replace the existing "Codex (future expansion)" section with actual implementation details:

```markdown
### Codex

- Log location: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`
- Format: each line is a JSON object with `{"timestamp", "type", "payload"}` structure
- Entry types: session_meta, response_item, event_msg, turn_context
- Parser: two-stage conversion (CodexRawEntry → CodexEntry)
```

Add to the project structure:

```
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── claude.rs      # Claude Code log parser
│   │   └── codex.rs       # Codex log parser
```

- [ ] **Step 2: Commit**

```bash
git add docs/ARCHITECTURE.md
git commit -m "docs: update architecture documentation for Codex parser"
```
