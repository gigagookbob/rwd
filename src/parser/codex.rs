// Codex CLI session log (.jsonl) parser.
//
// Codex JSONL uses a nested payload structure unlike Claude Code.
// Two-stage parsing: CodexRawEntry(loose) → CodexEntry(structured).

#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use std::collections::HashSet;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use super::roots;

// === Stage 1: Loose parsing types ===

/// Loose representation of a single JSONL line.
/// Keeps payload as serde_json::Value for flexibility.
#[derive(Debug, Deserialize)]
pub struct CodexRawEntry {
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "type")]
    pub entry_type: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

// === Stage 2: Structured enum types ===

/// Categorizes each Codex log entry into meaningful variants.
/// Requires two-stage conversion because Codex has "type" at both
/// the top level and inside the payload.
#[derive(Debug)]
pub enum CodexEntry {
    /// Session start metadata: session ID, working directory, model provider
    SessionMeta {
        timestamp: DateTime<Utc>,
        session_id: String,
        cwd: String,
        model_provider: String,
        subagent_source: Option<String>,
        agent_role: Option<String>,
        agent_nickname: Option<String>,
        text: String,
    },
    /// User input message
    UserMessage {
        timestamp: DateTime<Utc>,
        text: String,
    },
    /// Assistant (AI) text response
    AssistantMessage {
        timestamp: DateTime<Utc>,
        text: String,
    },
    /// Function (tool) call by the AI
    FunctionCall {
        timestamp: DateTime<Utc>,
        name: String,
        text: String,
    },
    /// Unknown or unhandled entry
    Other {
        timestamp: DateTime<Utc>,
        entry_type: String,
        text: String,
    },
}

/// Classifies Codex sessions from explicit SessionMeta metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexSessionKind {
    Interactive,
    Subagent,
}

fn collect_json_string_leaves(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if !s.trim().is_empty() {
                out.push(s.clone());
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_json_string_leaves(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values() {
                collect_json_string_leaves(value, out);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn join_unique_lines(parts: &[String]) -> String {
    let mut seen = HashSet::new();
    let mut lines = Vec::new();
    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            lines.push(trimmed.to_string());
        }
    }
    lines.join("\n")
}

fn extract_payload_text(payload: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    collect_json_string_leaves(payload, &mut parts);
    join_unique_lines(&parts)
}

fn optional_non_empty_string(value: &serde_json::Value) -> Option<String> {
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.to_string())
}

fn extract_subagent_source(payload: &serde_json::Value) -> Option<String> {
    let source = payload.get("source")?.as_object()?;
    optional_non_empty_string(source.get("subagent")?)
}

pub fn classify_codex_session(entries: &[CodexEntry]) -> CodexSessionKind {
    for entry in entries {
        if let CodexEntry::SessionMeta {
            subagent_source,
            agent_role,
            agent_nickname,
            ..
        } = entry
            && (subagent_source.is_some() || agent_role.is_some() || agent_nickname.is_some())
        {
            return CodexSessionKind::Subagent;
        }
    }

    CodexSessionKind::Interactive
}

impl CodexEntry {
    /// Converts a CodexRawEntry into a structured CodexEntry variant.
    /// Missing fields default to empty strings since the Codex JSONL schema
    /// is unofficial and may vary across versions.
    pub fn from_raw(raw: CodexRawEntry) -> Self {
        let ts = raw.timestamp;
        let entry_type = raw.entry_type;
        let payload = &raw.payload;
        let payload_text = extract_payload_text(payload);

        match entry_type.as_str() {
            "session_meta" => {
                let session_id = payload["id"].as_str().unwrap_or_default().to_string();
                let cwd = payload["cwd"].as_str().unwrap_or_default().to_string();
                let model_provider = payload["model_provider"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let subagent_source = extract_subagent_source(payload);
                let agent_role = payload
                    .get("agent_role")
                    .and_then(optional_non_empty_string);
                let agent_nickname = payload
                    .get("agent_nickname")
                    .and_then(optional_non_empty_string);
                CodexEntry::SessionMeta {
                    timestamp: ts,
                    session_id,
                    cwd,
                    model_provider,
                    subagent_source,
                    agent_role,
                    agent_nickname,
                    text: payload_text.clone(),
                }
            }
            "event_msg" if payload["type"].as_str() == Some("user_message") => {
                let text = payload["message"].as_str().unwrap_or_default().to_string();
                let text = join_unique_lines(&[text, payload_text.clone()]);
                CodexEntry::UserMessage {
                    timestamp: ts,
                    text,
                }
            }
            "response_item"
                if payload["type"].as_str() == Some("message")
                    && payload["role"].as_str() == Some("assistant") =>
            {
                let text = extract_codex_output_text(payload);
                let text = join_unique_lines(&[text, payload_text.clone()]);
                CodexEntry::AssistantMessage {
                    timestamp: ts,
                    text,
                }
            }
            "response_item" if payload["type"].as_str() == Some("function_call") => {
                let name = payload["name"].as_str().unwrap_or_default().to_string();
                let text = join_unique_lines(&[name.clone(), payload_text.clone()]);
                CodexEntry::FunctionCall {
                    timestamp: ts,
                    name,
                    text,
                }
            }
            _ => CodexEntry::Other {
                timestamp: ts,
                entry_type,
                text: payload_text,
            },
        }
    }
}

/// Extracts text from output_text blocks in the payload's content array.
fn extract_codex_output_text(payload: &serde_json::Value) -> String {
    let Some(content) = payload["content"].as_array() else {
        return String::new();
    };

    content
        .iter()
        .filter(|block| block["type"].as_str() == Some("output_text"))
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("")
}

// === File discovery ===

/// Returns all existing Codex session roots in priority order.
/// Priority: config overrides -> CODEX_HOME -> native home -> WSL Windows homes.
pub fn discover_codex_session_roots(config_roots: Option<&[String]>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(roots) = config_roots {
        candidates.extend(roots.iter().map(PathBuf::from));
    }

    if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
        candidates.push(PathBuf::from(codex_home).join("sessions"));
    }

    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".codex").join("sessions"));
    }

    if cfg!(target_os = "linux") && roots::is_wsl_environment() {
        for win_home in roots::wsl_windows_home_candidates() {
            candidates.push(win_home.join(".codex").join("sessions"));
        }
    }

    roots::dedupe_existing_paths(candidates)
}

/// Backward-compatible single-root API.
pub fn discover_codex_sessions_dir() -> Result<PathBuf, super::ParseError> {
    if let Some(root) = discover_codex_session_roots(None).into_iter().next() {
        return Ok(root);
    }

    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".codex").join("sessions"))
}

/// Lists session files for a specific date.
/// Codex stores sessions in YYYY/MM/DD/ subdirectories.
pub fn list_session_files_for_date(
    sessions_dir: &Path,
    date: NaiveDate,
) -> Result<Vec<PathBuf>, super::ParseError> {
    let date_path = sessions_dir
        .join(date.format("%Y").to_string())
        .join(date.format("%m").to_string())
        .join(date.format("%d").to_string());

    if !date_path.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(&date_path)? {
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

/// Lists session files for a local date by deriving the exact UTC date set
/// touched by the local [00:00, next 00:00) window.
pub fn list_session_files_for_local_date(
    sessions_dir: &Path,
    local_date: NaiveDate,
) -> Result<Vec<PathBuf>, super::ParseError> {
    let mut files = Vec::new();
    for date in super::utc_dates_for_local_date(local_date)? {
        files.extend(list_session_files_for_date(sessions_dir, date)?);
    }
    Ok(files)
}

/// Extracts the local date from a CodexEntry's timestamp.
pub fn entry_local_date(entry: &CodexEntry) -> Option<NaiveDate> {
    entry_timestamp(entry).map(|ts| ts.with_timezone(&chrono::Local).date_naive())
}

/// Returns UTC timestamp for timestamp-bearing Codex entries.
pub fn entry_timestamp(entry: &CodexEntry) -> Option<DateTime<Utc>> {
    match entry {
        CodexEntry::SessionMeta { timestamp, .. }
        | CodexEntry::UserMessage { timestamp, .. }
        | CodexEntry::AssistantMessage { timestamp, .. }
        | CodexEntry::FunctionCall { timestamp, .. }
        | CodexEntry::Other { timestamp, .. } => Some(*timestamp),
    }
}

/// Filters Codex entries to only those belonging to the given local date.
/// SessionMeta entries are always preserved regardless of date, because
/// they carry session metadata needed by `summarize_codex_entries`.
pub fn filter_entries_by_local_date(entries: Vec<CodexEntry>, date: NaiveDate) -> Vec<CodexEntry> {
    let window = super::local_date_to_utc_window(date).ok();

    entries
        .into_iter()
        .filter(|entry| {
            matches!(entry, CodexEntry::SessionMeta { .. })
                || entry_timestamp(entry).is_some_and(|ts| {
                    if let Some(window) = window {
                        window.contains(ts)
                    } else {
                        // Fallback if local midnight resolution fails unexpectedly.
                        ts.with_timezone(&chrono::Local).date_naive() == date
                    }
                })
        })
        .collect()
}

/// Parses a JSONL file into a vector of CodexEntry.
/// Skips invalid lines with a warning.
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
            Ok(raw) => entries.push(CodexEntry::from_raw(raw)),
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CodexEntryFingerprint {
    SessionMeta {
        timestamp_millis: i64,
        session_id: String,
        cwd: String,
        model_provider: String,
        subagent_source: Option<String>,
        agent_role: Option<String>,
        agent_nickname: Option<String>,
        text: String,
    },
    UserMessage {
        timestamp_millis: i64,
        text: String,
    },
    AssistantMessage {
        timestamp_millis: i64,
        text: String,
    },
    FunctionCall {
        timestamp_millis: i64,
        name: String,
        text: String,
    },
    Other {
        timestamp_millis: i64,
        entry_type: String,
        text: String,
    },
}

fn entry_fingerprint(entry: &CodexEntry) -> CodexEntryFingerprint {
    match entry {
        CodexEntry::SessionMeta {
            timestamp,
            session_id,
            cwd,
            model_provider,
            subagent_source,
            agent_role,
            agent_nickname,
            text,
        } => CodexEntryFingerprint::SessionMeta {
            timestamp_millis: timestamp.timestamp_millis(),
            session_id: session_id.clone(),
            cwd: cwd.clone(),
            model_provider: model_provider.clone(),
            subagent_source: subagent_source.clone(),
            agent_role: agent_role.clone(),
            agent_nickname: agent_nickname.clone(),
            text: text.clone(),
        },
        CodexEntry::UserMessage { timestamp, text } => CodexEntryFingerprint::UserMessage {
            timestamp_millis: timestamp.timestamp_millis(),
            text: text.clone(),
        },
        CodexEntry::AssistantMessage { timestamp, text } => {
            CodexEntryFingerprint::AssistantMessage {
                timestamp_millis: timestamp.timestamp_millis(),
                text: text.clone(),
            }
        }
        CodexEntry::FunctionCall {
            timestamp,
            name,
            text,
        } => CodexEntryFingerprint::FunctionCall {
            timestamp_millis: timestamp.timestamp_millis(),
            name: name.clone(),
            text: text.clone(),
        },
        CodexEntry::Other {
            timestamp,
            entry_type,
            text,
        } => CodexEntryFingerprint::Other {
            timestamp_millis: timestamp.timestamp_millis(),
            entry_type: entry_type.clone(),
            text: text.clone(),
        },
    }
}

/// Dedupe Codex entries by fingerprint.
pub fn dedupe_entries(entries: Vec<CodexEntry>) -> Vec<CodexEntry> {
    let mut seen: HashSet<CodexEntryFingerprint> = HashSet::new();
    let mut deduped = Vec::new();

    for entry in entries {
        let fingerprint = entry_fingerprint(&entry);
        if seen.insert(fingerprint) {
            deduped.push(entry);
        }
    }

    deduped
}

/// Sorts entries by timestamp while keeping non-timestamp entries at the end.
pub fn sort_entries_by_timestamp(entries: &mut [CodexEntry]) {
    entries.sort_by(|a, b| match (entry_timestamp(a), entry_timestamp(b)) {
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
}

// === Session summary ===

/// Summary of a single Codex session.
#[derive(Debug)]
pub struct CodexSessionSummary {
    pub session_id: String,
    pub cwd: String,
    pub model_provider: String,
    pub user_count: usize,
    pub assistant_count: usize,
    pub function_call_count: usize,
}

/// Builds a session summary from parsed entries.
/// Uses the first SessionMeta for session info; defaults to empty strings if absent.
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
                session_id,
                cwd,
                model_provider,
                ..
            } => {
                if summary.session_id.is_empty() {
                    summary.session_id.clone_from(session_id);
                    summary.cwd.clone_from(cwd);
                    summary.model_provider.clone_from(model_provider);
                }
            }
            CodexEntry::UserMessage { .. } => summary.user_count += 1,
            CodexEntry::AssistantMessage { .. } => summary.assistant_count += 1,
            CodexEntry::FunctionCall { .. } => summary.function_call_count += 1,
            CodexEntry::Other { .. } => {}
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_session_meta_entry() {
        let json = r#"{"timestamp":"2026-03-16T09:00:00Z","type":"session_meta","payload":{"id":"sess-abc","cwd":"/home/user/project","model_provider":"openai"}}"#;
        let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
        let entry = CodexEntry::from_raw(raw);

        if let CodexEntry::SessionMeta {
            session_id,
            cwd,
            model_provider,
            ..
        } = entry
        {
            assert_eq!(session_id, "sess-abc");
            assert_eq!(cwd, "/home/user/project");
            assert_eq!(model_provider, "openai");
        } else {
            panic!("Expected SessionMeta variant");
        }
    }

    #[test]
    fn test_parse_session_meta_entry_preserves_subagent_metadata() {
        let json = r#"{"timestamp":"2026-04-22T09:00:00Z","type":"session_meta","payload":{"id":"sess-sub","cwd":"/Users/jinwoohan/.codex/memories","model_provider":"openai","source":{"subagent":"memory_consolidation"},"agent_role":"memory builder","agent_nickname":"Morpheus"}}"#;
        let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
        let entry = CodexEntry::from_raw(raw);

        if let CodexEntry::SessionMeta {
            session_id,
            cwd,
            model_provider,
            subagent_source,
            agent_role,
            agent_nickname,
            ..
        } = entry
        {
            assert_eq!(session_id, "sess-sub");
            assert_eq!(cwd, "/Users/jinwoohan/.codex/memories");
            assert_eq!(model_provider, "openai");
            assert_eq!(subagent_source.as_deref(), Some("memory_consolidation"));
            assert_eq!(agent_role.as_deref(), Some("memory builder"));
            assert_eq!(agent_nickname.as_deref(), Some("Morpheus"));
        } else {
            panic!("Expected SessionMeta variant");
        }
    }

    #[test]
    fn test_parse_session_meta_entry_ignores_string_source_for_subagent_detection() {
        let json = r#"{"timestamp":"2026-04-22T09:00:00Z","type":"session_meta","payload":{"id":"sess-cli","cwd":"/Users/jinwoohan/workspace/repos/personal/rwd","model_provider":"openai","source":"cli"}}"#;
        let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
        let entry = CodexEntry::from_raw(raw);

        if let CodexEntry::SessionMeta {
            subagent_source,
            agent_role,
            agent_nickname,
            ..
        } = entry
        {
            assert_eq!(subagent_source, None);
            assert_eq!(agent_role, None);
            assert_eq!(agent_nickname, None);
        } else {
            panic!("Expected SessionMeta variant");
        }
    }

    fn make_session_meta_entry(
        session_id: &str,
        cwd: &str,
        subagent_source: Option<&str>,
        agent_role: Option<&str>,
        agent_nickname: Option<&str>,
    ) -> CodexEntry {
        use chrono::TimeZone;

        CodexEntry::SessionMeta {
            timestamp: chrono::Utc
                .with_ymd_and_hms(2026, 4, 22, 9, 0, 0)
                .unwrap(),
            session_id: session_id.to_string(),
            cwd: cwd.to_string(),
            model_provider: "openai".to_string(),
            subagent_source: subagent_source.map(str::to_string),
            agent_role: agent_role.map(str::to_string),
            agent_nickname: agent_nickname.map(str::to_string),
            text: "subagent".to_string(),
        }
    }

    #[test]
    fn test_classify_codex_session_kind_interactive_when_only_suspicious_cwd_present() {
        let entries = vec![make_session_meta_entry(
            "interactive",
            "/Users/jinwoohan/.codex/worktrees/1234/rwd",
            None,
            None,
            None,
        )];

        assert_eq!(
            classify_codex_session(&entries),
            CodexSessionKind::Interactive
        );
    }

    #[test]
    fn test_classify_codex_session_kind_uses_subagent_source_only() {
        let entries = vec![make_session_meta_entry(
            "subagent-source",
            "/Users/jinwoohan/workspace/repos/personal/rwd",
            Some("memory_consolidation"),
            None,
            None,
        )];

        assert_eq!(
            classify_codex_session(&entries),
            CodexSessionKind::Subagent
        );
    }

    #[test]
    fn test_classify_codex_session_kind_uses_agent_role_only() {
        let entries = vec![make_session_meta_entry(
            "subagent-role",
            "/Users/jinwoohan/.codex/memories",
            None,
            Some("memory builder"),
            None,
        )];

        assert_eq!(
            classify_codex_session(&entries),
            CodexSessionKind::Subagent
        );
    }

    #[test]
    fn test_classify_codex_session_kind_uses_agent_nickname_only() {
        let entries = vec![make_session_meta_entry(
            "subagent-nickname",
            "/Users/jinwoohan/.codex/memories",
            None,
            None,
            Some("Morpheus"),
        )];

        assert_eq!(
            classify_codex_session(&entries),
            CodexSessionKind::Subagent
        );
    }

    #[test]
    fn test_parse_user_message_from_event_msg() {
        let json = r#"{"timestamp":"2026-03-16T09:01:00Z","type":"event_msg","payload":{"type":"user_message","message":"fix the bug"}}"#;
        let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
        let entry = CodexEntry::from_raw(raw);

        if let CodexEntry::UserMessage { text, .. } = entry {
            assert!(text.contains("fix the bug"));
        } else {
            panic!("Expected UserMessage variant");
        }
    }

    #[test]
    fn test_parse_assistant_message_from_response_item() {
        let json = r#"{"timestamp":"2026-03-16T09:02:00Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Sure, I'll fix it."},{"type":"output_text","text":" Done."}]}}"#;
        let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
        let entry = CodexEntry::from_raw(raw);

        if let CodexEntry::AssistantMessage { text, .. } = entry {
            assert!(text.contains("Sure, I'll fix it."));
            assert!(text.contains("Done."));
        } else {
            panic!("Expected AssistantMessage variant");
        }
    }

    #[test]
    fn test_parse_function_call_from_response_item() {
        let json = r#"{"timestamp":"2026-03-16T09:03:00Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{}"}}"#;
        let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
        let entry = CodexEntry::from_raw(raw);

        if let CodexEntry::FunctionCall { name, .. } = entry {
            assert_eq!(name, "shell");
        } else {
            panic!("Expected FunctionCall variant");
        }
    }

    #[test]
    fn test_parse_unknown_entry_returns_other() {
        let json =
            r#"{"timestamp":"2026-03-16T09:04:00Z","type":"unknown_future_type","payload":{}}"#;
        let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
        let entry = CodexEntry::from_raw(raw);
        assert!(matches!(entry, CodexEntry::Other { .. }));
    }

    #[test]
    fn test_discover_codex_sessions_dir_returns_path() {
        let result = discover_codex_sessions_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("sessions"));
    }

    #[test]
    fn test_discover_codex_session_roots_keeps_config_priority() {
        let base = std::env::temp_dir().join(format!(
            "rwd_test_codex_roots_{}",
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
        let roots = discover_codex_session_roots(Some(&overrides));
        assert!(roots.starts_with(&[first.clone(), second.clone()]));

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_list_session_files_for_date_with_temp_dir() {
        let base = std::env::temp_dir().join(format!(
            "rwd_test_codex_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let date_dir = base.join("2026").join("03").join("16");
        std::fs::create_dir_all(&date_dir).unwrap();

        let f1 = date_dir.join("session1.jsonl");
        let f2 = date_dir.join("session2.jsonl");
        let f3 = date_dir.join("not_a_session.txt");
        std::fs::File::create(&f1).unwrap();
        std::fs::File::create(&f2).unwrap();
        std::fs::File::create(&f3).unwrap();

        let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
        let files = list_session_files_for_date(&base, date).unwrap();

        assert_eq!(files.len(), 2);
        assert!(
            files
                .iter()
                .all(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
        );

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_parse_codex_jsonl_file_with_mixed_entries() {
        let base = std::env::temp_dir().join(format!(
            "rwd_test_codex_parse_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();

        let file_path = base.join("test.jsonl");
        let mut file = std::fs::File::create(&file_path).unwrap();

        writeln!(file, r#"{{"timestamp":"2026-03-16T09:00:00Z","type":"session_meta","payload":{{"id":"s1","cwd":"/p","model_provider":"openai"}}}}"#).unwrap();
        writeln!(file, r#"{{"timestamp":"2026-03-16T09:01:00Z","type":"event_msg","payload":{{"type":"user_message","message":"hello"}}}}"#).unwrap();
        writeln!(file, r#"{{"timestamp":"2026-03-16T09:02:00Z","type":"response_item","payload":{{"type":"function_call","name":"shell","arguments":"{{}}"}}}}"#).unwrap();
        writeln!(file, "not valid json").unwrap();

        let entries = parse_codex_jsonl_file(&file_path).unwrap();

        assert_eq!(entries.len(), 3);
        assert!(matches!(entries[0], CodexEntry::SessionMeta { .. }));
        assert!(matches!(entries[1], CodexEntry::UserMessage { .. }));
        assert!(matches!(entries[2], CodexEntry::FunctionCall { .. }));

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_dedupe_entries_removes_exact_duplicates() {
        use chrono::TimeZone;
        let ts = chrono::Utc.with_ymd_and_hms(2026, 3, 16, 12, 0, 0).unwrap();
        let entries = vec![
            CodexEntry::SessionMeta {
                timestamp: ts,
                session_id: "s1".to_string(),
                cwd: "/p".to_string(),
                model_provider: "openai".to_string(),
                subagent_source: None,
                agent_role: None,
                agent_nickname: None,
                text: "s1\n/p\nopenai".to_string(),
            },
            CodexEntry::UserMessage {
                timestamp: ts,
                text: "hello".to_string(),
            },
            CodexEntry::UserMessage {
                timestamp: ts,
                text: "hello".to_string(),
            },
        ];

        let deduped = dedupe_entries(entries);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_summarize_codex_entries_counts_correctly() {
        let raws = [
            r#"{"timestamp":"2026-03-16T09:00:00Z","type":"session_meta","payload":{"id":"sess-xyz","cwd":"/project","model_provider":"openai"}}"#,
            r#"{"timestamp":"2026-03-16T09:01:00Z","type":"event_msg","payload":{"type":"user_message","message":"msg1"}}"#,
            r#"{"timestamp":"2026-03-16T09:02:00Z","type":"event_msg","payload":{"type":"user_message","message":"msg2"}}"#,
            r#"{"timestamp":"2026-03-16T09:03:00Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}}"#,
            r#"{"timestamp":"2026-03-16T09:04:00Z","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{}"}}"#,
            r#"{"timestamp":"2026-03-16T09:05:00Z","type":"response_item","payload":{"type":"function_call","name":"write_file","arguments":"{}"}}"#,
        ];

        let entries: Vec<CodexEntry> = raws
            .iter()
            .map(|s| CodexEntry::from_raw(serde_json::from_str::<CodexRawEntry>(s).unwrap()))
            .collect();

        let summary = summarize_codex_entries(&entries);

        assert_eq!(summary.session_id, "sess-xyz");
        assert_eq!(summary.cwd, "/project");
        assert_eq!(summary.model_provider, "openai");
        assert_eq!(summary.user_count, 2);
        assert_eq!(summary.assistant_count, 1);
        assert_eq!(summary.function_call_count, 2);
    }

    #[test]
    fn test_work_summary_object_stringify() {
        let json = r#"{"sessions":[{"session_id":"s1","work_summary":{"main":"요약","detail":"상세"},"decisions":[],"curiosities":[],"corrections":[]}]}"#;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        let ws = &parsed["sessions"][0]["work_summary"];
        assert!(ws.is_object());
    }

    #[test]
    fn test_filter_entries_by_local_date_keeps_today_only() {
        use chrono::TimeZone;
        let yesterday_ts = chrono::Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap();
        let today_ts = chrono::Utc.with_ymd_and_hms(2026, 3, 16, 12, 0, 0).unwrap();
        let today_local = today_ts.with_timezone(&chrono::Local).date_naive();

        let entries = vec![
            CodexEntry::SessionMeta {
                timestamp: yesterday_ts,
                session_id: "s1".to_string(),
                cwd: "/p".to_string(),
                model_provider: "openai".to_string(),
                subagent_source: None,
                agent_role: None,
                agent_nickname: None,
                text: "s1\n/p\nopenai".to_string(),
            },
            CodexEntry::UserMessage {
                timestamp: yesterday_ts,
                text: "old msg".to_string(),
            },
            CodexEntry::AssistantMessage {
                timestamp: yesterday_ts,
                text: "old reply".to_string(),
            },
            CodexEntry::UserMessage {
                timestamp: today_ts,
                text: "new msg".to_string(),
            },
            CodexEntry::AssistantMessage {
                timestamp: today_ts,
                text: "new reply".to_string(),
            },
        ];

        let filtered = filter_entries_by_local_date(entries, today_local);

        assert_eq!(filtered.len(), 3);
        assert!(matches!(filtered[0], CodexEntry::SessionMeta { .. }));
        assert!(matches!(filtered[1], CodexEntry::UserMessage { .. }));
        if let CodexEntry::UserMessage { text, .. } = &filtered[1] {
            assert_eq!(text, "new msg");
        }
        assert!(matches!(filtered[2], CodexEntry::AssistantMessage { .. }));
    }

    #[test]
    fn test_filter_entries_by_local_date_no_today_entries() {
        use chrono::TimeZone;
        let yesterday_ts = chrono::Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap();
        let today_ts = chrono::Utc.with_ymd_and_hms(2026, 3, 16, 12, 0, 0).unwrap();
        let today_local = today_ts.with_timezone(&chrono::Local).date_naive();

        let entries = vec![
            CodexEntry::SessionMeta {
                timestamp: yesterday_ts,
                session_id: "s1".to_string(),
                cwd: "/p".to_string(),
                model_provider: "openai".to_string(),
                subagent_source: None,
                agent_role: None,
                agent_nickname: None,
                text: "s1\n/p\nopenai".to_string(),
            },
            CodexEntry::UserMessage {
                timestamp: yesterday_ts,
                text: "old msg".to_string(),
            },
        ];

        let filtered = filter_entries_by_local_date(entries, today_local);

        assert_eq!(filtered.len(), 1);
        assert!(matches!(filtered[0], CodexEntry::SessionMeta { .. }));
    }

    #[test]
    fn test_filter_entries_by_local_date_same_day_keeps_all() {
        use chrono::TimeZone;
        let today_ts = chrono::Utc.with_ymd_and_hms(2026, 3, 16, 12, 0, 0).unwrap();
        let today_local = today_ts.with_timezone(&chrono::Local).date_naive();

        let entries = vec![
            CodexEntry::SessionMeta {
                timestamp: today_ts,
                session_id: "s1".to_string(),
                cwd: "/p".to_string(),
                model_provider: "openai".to_string(),
                subagent_source: None,
                agent_role: None,
                agent_nickname: None,
                text: "s1\n/p\nopenai".to_string(),
            },
            CodexEntry::UserMessage {
                timestamp: today_ts,
                text: "msg".to_string(),
            },
            CodexEntry::AssistantMessage {
                timestamp: today_ts,
                text: "reply".to_string(),
            },
            CodexEntry::FunctionCall {
                timestamp: today_ts,
                name: "shell".to_string(),
                text: "shell".to_string(),
            },
        ];

        let filtered = filter_entries_by_local_date(entries, today_local);
        assert_eq!(filtered.len(), 4);
    }

    #[test]
    fn test_filter_entries_by_local_date_respects_half_open_window_boundaries() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 4, 11).expect("valid date");
        let window = crate::parser::local_date_to_utc_window(date).expect("utc window");

        let entries = vec![
            CodexEntry::SessionMeta {
                timestamp: window.start_utc,
                session_id: "s1".to_string(),
                cwd: "/p".to_string(),
                model_provider: "openai".to_string(),
                subagent_source: None,
                agent_role: None,
                agent_nickname: None,
                text: "s1\n/p\nopenai".to_string(),
            },
            CodexEntry::UserMessage {
                timestamp: window.start_utc - chrono::Duration::nanoseconds(1),
                text: "before_start".to_string(),
            },
            CodexEntry::UserMessage {
                timestamp: window.start_utc,
                text: "at_start".to_string(),
            },
            CodexEntry::AssistantMessage {
                timestamp: window.end_utc - chrono::Duration::nanoseconds(1),
                text: "before_end".to_string(),
            },
            CodexEntry::AssistantMessage {
                timestamp: window.end_utc,
                text: "at_end".to_string(),
            },
        ];

        let filtered = filter_entries_by_local_date(entries, date);
        assert!(matches!(filtered[0], CodexEntry::SessionMeta { .. }));

        let kept_texts: Vec<&str> = filtered
            .iter()
            .filter_map(|entry| match entry {
                CodexEntry::UserMessage { text, .. }
                | CodexEntry::AssistantMessage { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(kept_texts, vec!["at_start", "before_end"]);
    }
}
