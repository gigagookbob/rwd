// Claude Code session log (.jsonl) parser.
//
// Each line in a JSONL file is an independent JSON object, differentiated by the "type" field.
// Uses serde for automatic JSON-to-struct deserialization.

#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::{Path, PathBuf};

use super::roots;

// === Data type definitions ===

/// Each line in a JSONL file, dispatched by the "type" field.
/// rename_all = "kebab-case" maps PascalCase variants to kebab-case
/// (e.g., FileHistorySnapshot -> "file-history-snapshot").
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LogEntry {
    User(UserEntry),
    Assistant(AssistantEntry),
    Progress(ProgressEntry),
    System(SystemEntry),
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot(FileHistorySnapshotEntry),
    // Catch-all for unknown log types to prevent parse failures on new entry types.
    #[serde(untagged)]
    Other(serde_json::Value),
}

/// User message entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub uuid: String,
    #[serde(default)]
    pub message: Option<serde_json::Value>,
}

/// Assistant (AI) response entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub uuid: String,
    #[serde(default)]
    pub message: Option<AssistantMessage>,
}

/// Detailed structure of an assistant message.
#[derive(Debug, Clone, Deserialize)]
pub struct AssistantMessage {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// Block types that can appear in an assistant message's content array.
/// Dispatched by the "type" field; rename_all = "snake_case" maps PascalCase to snake_case
/// (e.g., ToolUse -> "tool_use").
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Thinking {
        #[serde(default)]
        thinking: Option<String>,
    },
    Text {
        #[serde(default)]
        text: Option<String>,
    },
    ToolUse {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        id: Option<String>,
    },
    ToolResult {
        #[serde(default)]
        content: Option<serde_json::Value>,
    },
    // Catches unknown block types so parsing doesn't fail on new additions.
    #[serde(other)]
    Unknown,
}

/// API token usage.
/// Total input tokens = input_tokens + cache_creation_input_tokens + cache_read_input_tokens.
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    /// Input tokens newly stored in the cache.
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    /// Input tokens read from the cache.
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

/// Progress entry (agent task progress, etc.).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
}

/// System entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemEntry {
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// File history snapshot entry (no detailed analysis needed).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistorySnapshotEntry {
    #[serde(default)]
    pub message_id: Option<String>,
}

/// Summary information for a session log.
#[derive(Debug)]
pub struct SessionSummary {
    pub session_id: String,
    pub user_count: usize,
    pub assistant_count: usize,
    pub tool_use_count: usize,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
}

// === File discovery functions ===

/// Returns all existing Claude project roots in priority order.
/// Priority: config overrides -> native home -> WSL Windows homes.
pub fn discover_claude_log_roots(config_roots: Option<&[String]>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(roots) = config_roots {
        candidates.extend(roots.iter().map(PathBuf::from));
    }

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".claude").join("projects"));
    }

    if cfg!(target_os = "linux") && roots::is_wsl_environment() {
        for win_home in roots::wsl_windows_home_candidates() {
            candidates.push(win_home.join(".claude").join("projects"));
        }
    }

    roots::dedupe_existing_paths(candidates)
}

/// Backward-compatible single-root API.
pub fn discover_log_dir() -> Result<PathBuf, super::ParseError> {
    discover_claude_log_roots(None)
        .into_iter()
        .next()
        .ok_or("Claude projects directory not found".into())
}

/// Returns all project directories under one Claude root.
pub fn list_project_dirs_in_root(base: &Path) -> Result<Vec<PathBuf>, super::ParseError> {
    let mut dirs = Vec::new();

    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }

    Ok(dirs)
}

/// Returns all .jsonl files in the given project directory.
pub fn list_session_files(project_dir: &Path) -> Result<Vec<PathBuf>, super::ParseError> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(project_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Some(ext) = path.extension()
            && ext == "jsonl"
        {
            files.push(path);
        }
    }

    Ok(files)
}

// === JSONL parsing functions ===

/// Reads a JSONL file and returns a vector of LogEntries.
/// Lines that fail to parse are skipped with a warning on stderr.
pub fn parse_jsonl_file(path: &Path) -> Result<Vec<LogEntry>, super::ParseError> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<LogEntry>(&line) {
            Ok(entry) => entries.push(entry),
            Err(err) => {
                eprintln!(
                    "Warning: Failed to parse line {} in {}: {}",
                    line_num + 1,
                    path.display(),
                    err
                );
            }
        }
    }

    Ok(entries)
}

// === Filtering and summary functions ===

/// Extracts the timestamp from a LogEntry, if available.
pub fn entry_timestamp(entry: &LogEntry) -> Option<DateTime<Utc>> {
    match entry {
        LogEntry::User(e) => Some(e.timestamp),
        LogEntry::Assistant(e) => Some(e.timestamp),
        LogEntry::Progress(e) => Some(e.timestamp),
        LogEntry::System(e) => Some(e.timestamp),
        LogEntry::FileHistorySnapshot(_) | LogEntry::Other(_) => None,
    }
}

/// Filters entries to only those matching the given date (in local timezone).
pub fn filter_entries_by_date(entries: Vec<LogEntry>, date: NaiveDate) -> Vec<LogEntry> {
    entries
        .into_iter()
        .filter(|entry| {
            // Convert UTC timestamp to local timezone before date comparison.
            // This ensures entries are filtered by the user's local date, not UTC.
            match entry_timestamp(entry) {
                Some(ts) => ts.with_timezone(&chrono::Local).date_naive() == date,
                None => false,
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ClaudeEntryFingerprint {
    User {
        session_id: String,
        uuid: String,
    },
    Assistant {
        session_id: String,
        uuid: String,
    },
    Progress {
        session_id: String,
        timestamp_millis: i64,
    },
    System {
        session_id: String,
        timestamp_millis: i64,
    },
    FileHistorySnapshot {
        message_id: String,
    },
    Other {
        value: String,
    },
}

fn entry_fingerprint(entry: &LogEntry) -> ClaudeEntryFingerprint {
    match entry {
        LogEntry::User(e) => ClaudeEntryFingerprint::User {
            session_id: e.session_id.clone(),
            uuid: e.uuid.clone(),
        },
        LogEntry::Assistant(e) => ClaudeEntryFingerprint::Assistant {
            session_id: e.session_id.clone(),
            uuid: e.uuid.clone(),
        },
        LogEntry::Progress(e) => ClaudeEntryFingerprint::Progress {
            session_id: e.session_id.clone(),
            timestamp_millis: e.timestamp.timestamp_millis(),
        },
        LogEntry::System(e) => ClaudeEntryFingerprint::System {
            session_id: e.session_id.clone().unwrap_or_default(),
            timestamp_millis: e.timestamp.timestamp_millis(),
        },
        LogEntry::FileHistorySnapshot(e) => ClaudeEntryFingerprint::FileHistorySnapshot {
            message_id: e.message_id.clone().unwrap_or_default(),
        },
        LogEntry::Other(v) => ClaudeEntryFingerprint::Other {
            value: v.to_string(),
        },
    }
}

/// Dedupe Claude entries by stable per-entry fingerprints.
pub fn dedupe_entries(entries: Vec<LogEntry>) -> Vec<LogEntry> {
    let mut seen: HashSet<ClaudeEntryFingerprint> = HashSet::new();
    let mut deduped = Vec::new();

    for entry in entries {
        let fingerprint = entry_fingerprint(&entry);
        if seen.insert(fingerprint) {
            deduped.push(entry);
        }
    }

    deduped
}

/// Groups parsed entries by session and produces per-session summaries.
pub fn summarize_entries(entries: &[LogEntry]) -> Vec<SessionSummary> {
    let mut sessions: HashMap<String, SessionSummary> = HashMap::new();

    for entry in entries {
        let session_id = match entry {
            LogEntry::User(e) => &e.session_id,
            LogEntry::Assistant(e) => &e.session_id,
            LogEntry::Progress(e) => &e.session_id,
            LogEntry::System(_) | LogEntry::FileHistorySnapshot(_) | LogEntry::Other(_) => continue,
        };

        let summary = sessions
            .entry(session_id.clone())
            .or_insert_with(|| SessionSummary {
                session_id: session_id.clone(),
                user_count: 0,
                assistant_count: 0,
                tool_use_count: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_cache_creation_tokens: 0,
                total_cache_read_tokens: 0,
            });

        match entry {
            LogEntry::User(_) => summary.user_count += 1,
            LogEntry::Assistant(e) => {
                summary.assistant_count += 1;
                if let Some(msg) = &e.message {
                    for block in &msg.content {
                        if matches!(block, ContentBlock::ToolUse { .. }) {
                            summary.tool_use_count += 1;
                        }
                    }
                    if let Some(usage) = &msg.usage {
                        summary.total_input_tokens += usage.input_tokens;
                        summary.total_output_tokens += usage.output_tokens;
                        summary.total_cache_creation_tokens += usage.cache_creation_input_tokens;
                        summary.total_cache_read_tokens += usage.cache_read_input_tokens;
                    }
                }
            }
            LogEntry::Progress(_) => {}
            _ => {}
        }
    }

    sessions.into_values().collect()
}

// === Unit tests ===

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_user_entry_returns_user_variant() {
        let json = r#"{"type":"user","sessionId":"abc-12345","timestamp":"2026-03-11T13:06:07.215Z","uuid":"def-456","message":{"role":"user","content":"hello"}}"#;
        let entry: LogEntry = serde_json::from_str(json).unwrap();
        assert!(matches!(entry, LogEntry::User(_)));
    }

    #[test]
    fn test_parse_assistant_entry_with_text_content() {
        let json = r#"{"type":"assistant","sessionId":"abc-12345","timestamp":"2026-03-11T13:06:11.990Z","uuid":"ghi-789","message":{"model":"claude-opus-4-6","role":"assistant","content":[{"type":"text","text":"Hello world"}],"usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let entry: LogEntry = serde_json::from_str(json).unwrap();
        if let LogEntry::Assistant(a) = entry {
            let msg = a.message.unwrap();
            assert_eq!(msg.content.len(), 1);
            assert!(matches!(msg.content[0], ContentBlock::Text { .. }));
            let usage = msg.usage.unwrap();
            assert_eq!(usage.input_tokens, 100);
            assert_eq!(usage.output_tokens, 50);
        } else {
            panic!("Expected Assistant entry");
        }
    }

    #[test]
    fn test_parse_assistant_entry_with_tool_use() {
        let json = r#"{"type":"assistant","sessionId":"abc-12345","timestamp":"2026-03-11T13:06:11.990Z","uuid":"ghi-789","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","id":"toolu_123","input":{"file_path":"test.rs"}}]}}"#;
        let entry: LogEntry = serde_json::from_str(json).unwrap();
        if let LogEntry::Assistant(a) = entry {
            let msg = a.message.unwrap();
            assert!(
                matches!(&msg.content[0], ContentBlock::ToolUse { name, .. } if name.as_deref() == Some("Read"))
            );
        } else {
            panic!("Expected Assistant entry");
        }
    }

    #[test]
    fn test_parse_progress_entry_returns_progress_variant() {
        let json = r#"{"type":"progress","sessionId":"abc-12345","timestamp":"2026-03-11T13:06:12.895Z","uuid":"jkl-012","data":{"type":"hook_progress"}}"#;
        let entry: LogEntry = serde_json::from_str(json).unwrap();
        assert!(matches!(entry, LogEntry::Progress(_)));
    }

    #[test]
    fn test_parse_file_history_snapshot() {
        let json = r#"{"type":"file-history-snapshot","messageId":"abc-123","snapshot":{"messageId":"abc-123","trackedFileBackups":{},"timestamp":"2026-03-11T13:06:07.216Z"},"isSnapshotUpdate":false}"#;
        let entry: LogEntry = serde_json::from_str(json).unwrap();
        assert!(matches!(entry, LogEntry::FileHistorySnapshot(_)));
    }

    #[test]
    fn test_parse_invalid_json_returns_error() {
        let bad_json = "this is not json";
        let result = serde_json::from_str::<LogEntry>(bad_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_summarize_entries_counts_messages() {
        let entries = vec![
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:00:00Z","uuid":"u1"}"#,
            )
            .unwrap(),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:01:00Z","uuid":"u2"}"#,
            )
            .unwrap(),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"assistant","sessionId":"s1","timestamp":"2026-03-11T10:00:30Z","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"hello"}],"usage":{"input_tokens":10,"output_tokens":5}}}"#,
            )
            .unwrap(),
        ];
        let summaries = summarize_entries(&entries);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].user_count, 2);
        assert_eq!(summaries[0].assistant_count, 1);
        assert_eq!(summaries[0].total_input_tokens, 10);
    }

    #[test]
    fn test_filter_entries_by_date_filters_correctly() {
        let entries = vec![
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:00:00Z","uuid":"u1"}"#,
            )
            .unwrap(),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-10T10:00:00Z","uuid":"u2"}"#,
            )
            .unwrap(),
        ];
        let date = NaiveDate::from_ymd_opt(2026, 3, 11).unwrap();
        let filtered = filter_entries_by_date(entries, date);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_discover_claude_log_roots_keeps_config_priority() {
        let base = std::env::temp_dir().join(format!(
            "rwd_test_claude_roots_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let first = base.join("first");
        let second = base.join("second");
        std::fs::create_dir_all(&first).expect("first dir");
        std::fs::create_dir_all(&second).expect("second dir");

        let overrides = vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
            first.to_string_lossy().to_string(),
        ];

        let roots = discover_claude_log_roots(Some(&overrides));
        assert!(roots.starts_with(&[first.clone(), second.clone()]));

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_dedupe_entries_uses_session_and_uuid_for_user_entries() {
        let entries = vec![
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:00:00Z","uuid":"dup"}"#,
            )
            .expect("first"),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:05:00Z","uuid":"dup"}"#,
            )
            .expect("duplicate"),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:10:00Z","uuid":"unique"}"#,
            )
            .expect("unique"),
        ];

        let deduped = dedupe_entries(entries);
        assert_eq!(deduped.len(), 2);
    }
}
