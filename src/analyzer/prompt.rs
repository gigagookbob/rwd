// Converts LogEntry slices into prompt text for LLM analysis.
//
// Pure data transformation with no network calls, making it easy to unit-test.
// Default behavior preserves all extractable text from session entries.

use super::planner::SessionEstimate;
use crate::parser::claude::{ContentBlock, LogEntry};
use crate::parser::codex::CodexEntry;
use std::collections::HashMap;

/// Builds prompt text from a LogEntry slice for the Claude API.
/// Groups by session with [Session: id] headers and [USER]/[ASSISTANT] tags.
pub fn build_prompt(entries: &[LogEntry]) -> Result<String, super::AnalyzerError> {
    let conversation_text = extract_conversation_text(entries);
    if conversation_text.is_empty() {
        return Err(crate::messages::error::NO_CONVERSATION_CLAUDE.into());
    }
    Ok(conversation_text)
}

/// Extracts human-readable conversation text from LogEntries.
///
/// Groups messages by session using a HashMap, then converts each session to text.
/// Uses Vec<(role, text)> to maintain chronological order.
fn extract_conversation_text(entries: &[LogEntry]) -> String {
    let mut sessions: HashMap<&str, Vec<(&str, String)>> = HashMap::new();
    // Preserves session insertion order (HashMap does not guarantee order).
    let mut session_order: Vec<&str> = Vec::new();

    for entry in entries {
        let Some(session_id) = entry_session_id(entry) else {
            continue;
        };
        let Some((role, text)) = extract_entry_text(entry) else {
            continue;
        };
        if !sessions.contains_key(session_id) {
            session_order.push(session_id);
        }
        sessions.entry(session_id).or_default().push((role, text));
    }

    let mut output = String::new();
    for session_id in &session_order {
        if let Some(messages) = sessions.get(session_id) {
            output.push_str(&format!("[Session: {session_id}]\n"));
            for (role, text) in messages {
                output.push_str(&format!("[{role}] {text}\n"));
            }
            output.push('\n');
        }
    }

    output
}

/// Recursively collects all non-empty string leaves from JSON.
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
    let mut seen = std::collections::HashSet::new();
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

fn extract_all_json_text(value: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    collect_json_string_leaves(value, &mut parts);
    join_unique_lines(&parts)
}

fn extract_assistant_text(blocks: &[ContentBlock]) -> String {
    let mut parts = Vec::new();
    for block in blocks {
        match block {
            ContentBlock::Text { text } => {
                if let Some(text) = text {
                    parts.push(text.clone());
                }
            }
            ContentBlock::Thinking { thinking } => {
                if let Some(thinking) = thinking {
                    parts.push(thinking.clone());
                }
            }
            ContentBlock::ToolUse { name, id, input } => {
                if let Some(name) = name {
                    parts.push(name.clone());
                }
                if let Some(id) = id {
                    parts.push(id.clone());
                }
                if let Some(input) = input {
                    let text = extract_all_json_text(input);
                    if !text.is_empty() {
                        parts.push(text);
                    }
                }
            }
            ContentBlock::ToolResult { content } => {
                if let Some(content) = content {
                    let text = extract_all_json_text(content);
                    if !text.is_empty() {
                        parts.push(text);
                    }
                }
            }
            ContentBlock::Unknown => {}
        }
    }
    join_unique_lines(&parts)
}

fn session_id_from_other(entry: &serde_json::Value) -> Option<&str> {
    entry
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .or_else(|| entry.get("session_id").and_then(serde_json::Value::as_str))
}

fn entry_session_id(entry: &LogEntry) -> Option<&str> {
    match entry {
        LogEntry::User(e) => Some(e.session_id.as_str()),
        LogEntry::Assistant(e) => Some(e.session_id.as_str()),
        LogEntry::Progress(e) => Some(e.session_id.as_str()),
        LogEntry::System(e) => e.session_id.as_deref(),
        LogEntry::FileHistorySnapshot(_) => None,
        LogEntry::Other(raw) => session_id_from_other(raw),
    }
}

fn extract_entry_text(entry: &LogEntry) -> Option<(&'static str, String)> {
    match entry {
        LogEntry::User(e) => {
            let mut parts = Vec::new();
            if let Some(message) = &e.message {
                parts.push(extract_all_json_text(message));
            }
            if let Some(entrypoint) = &e.entrypoint {
                parts.push(entrypoint.clone());
            }
            if let Some(cwd) = &e.cwd {
                parts.push(cwd.clone());
            }
            let text = join_unique_lines(&parts);
            (!text.is_empty()).then_some(("USER", text))
        }
        LogEntry::Assistant(e) => {
            let mut parts = Vec::new();
            if let Some(message) = &e.message {
                parts.push(extract_assistant_text(&message.content));
                if let Some(model) = &message.model {
                    parts.push(model.clone());
                }
            }
            if let Some(entrypoint) = &e.entrypoint {
                parts.push(entrypoint.clone());
            }
            if let Some(cwd) = &e.cwd {
                parts.push(cwd.clone());
            }
            let text = join_unique_lines(&parts);
            (!text.is_empty()).then_some(("ASSISTANT", text))
        }
        LogEntry::Progress(e) => {
            let mut parts = Vec::new();
            if let Some(entrypoint) = &e.entrypoint {
                parts.push(entrypoint.clone());
            }
            if let Some(cwd) = &e.cwd {
                parts.push(cwd.clone());
            }
            let text = join_unique_lines(&parts);
            (!text.is_empty()).then_some(("PROGRESS", text))
        }
        LogEntry::System(_) => None,
        LogEntry::FileHistorySnapshot(e) => e
            .message_id
            .as_ref()
            .and_then(|id| (!id.trim().is_empty()).then_some(("SNAPSHOT", id.clone()))),
        LogEntry::Other(raw) => {
            let text = extract_all_json_text(raw);
            (!text.is_empty()).then_some(("EVENT", text))
        }
    }
}

/// Extracts (role, text) tuples from LogEntries.
/// Used by summarizer's split_into_chunks.
pub fn extract_messages(entries: &[LogEntry]) -> Vec<(String, String)> {
    let mut messages = Vec::new();
    for entry in entries {
        if let Some((role, text)) = extract_entry_text(entry) {
            messages.push((role.to_string(), text));
        }
    }
    messages
}

/// Converts Codex entries into prompt text for LLM analysis.
/// Each Codex file is a single session, so session_id is passed externally.
pub fn build_codex_prompt(
    entries: &[CodexEntry],
    session_id: &str,
) -> Result<String, super::AnalyzerError> {
    let mut output = format!("[Session: {session_id}]\n");
    let mut has_text = false;

    for entry in entries {
        match entry {
            CodexEntry::SessionMeta { text, .. } if !text.is_empty() => {
                has_text = true;
                output.push_str(&format!("[META] {text}\n"));
            }
            CodexEntry::UserMessage { text, .. } if !text.is_empty() => {
                has_text = true;
                output.push_str(&format!("[USER] {text}\n"));
            }
            CodexEntry::AssistantMessage { text, .. } if !text.is_empty() => {
                has_text = true;
                output.push_str(&format!("[ASSISTANT] {text}\n"));
            }
            CodexEntry::FunctionCall { text, .. } if !text.is_empty() => {
                has_text = true;
                output.push_str(&format!("[TOOL] {text}\n"));
            }
            CodexEntry::Other {
                entry_type, text, ..
            } if !text.is_empty() => {
                has_text = true;
                output.push_str(&format!("[EVENT:{entry_type}] {text}\n"));
            }
            _ => {}
        }
    }

    if !has_text {
        return Err(crate::messages::error::NO_CONVERSATION_CODEX.into());
    }

    Ok(output)
}

/// Extracts (role, text) tuples from Codex entries.
pub fn extract_codex_messages(entries: &[CodexEntry]) -> Vec<(String, String)> {
    let mut messages = Vec::new();
    for entry in entries {
        match entry {
            CodexEntry::SessionMeta { text, .. } if !text.is_empty() => {
                messages.push(("META".to_string(), text.clone()));
            }
            CodexEntry::UserMessage { text, .. } if !text.is_empty() => {
                messages.push(("USER".to_string(), text.clone()));
            }
            CodexEntry::AssistantMessage { text, .. } if !text.is_empty() => {
                messages.push(("ASSISTANT".to_string(), text.clone()));
            }
            CodexEntry::FunctionCall { text, .. } if !text.is_empty() => {
                messages.push(("TOOL".to_string(), text.clone()));
            }
            CodexEntry::Other {
                entry_type, text, ..
            } if !text.is_empty() => {
                messages.push((format!("EVENT:{entry_type}"), text.clone()));
            }
            _ => {}
        }
    }
    messages
}

/// Extracts unique session IDs from LogEntries, preserving insertion order.
/// Used to split entries by session during fallback execution.
pub fn extract_session_ids(entries: &[LogEntry]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in entries {
        let id = match entry {
            LogEntry::User(e) => Some(e.session_id.as_str()),
            LogEntry::Assistant(e) => Some(e.session_id.as_str()),
            LogEntry::Progress(e) => Some(e.session_id.as_str()),
            LogEntry::System(e) => e.session_id.as_deref(),
            LogEntry::FileHistorySnapshot(_) | LogEntry::Other(_) => None,
        };
        if let Some(session_id) = id
            && seen.insert(session_id.to_string())
        {
            ids.push(session_id.to_string());
        }
    }
    ids
}

/// Conservative estimate for system prompt tokens.
/// As of 2026-04-12, EN/KO system prompts estimate to ~1194/~1088 tokens via chars/2.
/// Keep a safety margin to avoid under-estimating planner input size.
pub const SYSTEM_PROMPT_ESTIMATED_TOKENS: u64 = 1_300;

/// Rough token estimate for text.
/// Korean syllables are ~1 token each, so char_count / 2 is a conservative estimate.
pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as u64) / 2
}

/// Estimates session tokens from a pre-built prompt text.
pub fn estimate_prompt_tokens(prompt_text: &str) -> u64 {
    estimate_tokens(prompt_text) + SYSTEM_PROMPT_ESTIMATED_TOKENS
}

/// Returns per-session token estimates.
pub fn estimate_sessions(entries: &[LogEntry]) -> Vec<SessionEstimate> {
    let session_ids = extract_session_ids(entries);
    let mut estimates = Vec::new();

    for session_id in &session_ids {
        let mut total_chars: u64 = 0;

        for entry in entries {
            let eid = entry_session_id(entry);
            if eid != Some(session_id.as_str()) {
                continue;
            }

            if let Some((_, text)) = extract_entry_text(entry) {
                total_chars += text.chars().count() as u64;
            }
        }

        let estimated_tokens = total_chars / 2 + SYSTEM_PROMPT_ESTIMATED_TOKENS;
        estimates.push(SessionEstimate {
            session_id: session_id.clone(),
            estimated_tokens,
        });
    }

    estimates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::claude::LogEntry;

    const SYSTEM_PROMPT_EN_TEXT: &str = include_str!("../../prompts/system_en.md");
    const SYSTEM_PROMPT_KO_TEXT: &str = include_str!("../../prompts/system_ko.md");
    const SLACK_PROMPT_EN_TEXT: &str = include_str!("../../prompts/slack_en.md");
    const SLACK_PROMPT_KO_TEXT: &str = include_str!("../../prompts/slack_ko.md");

    #[test]
    fn test_build_prompt_extracts_user_text() {
        let entries = vec![serde_json::from_str::<LogEntry>(
            r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:00:00Z","uuid":"u1","message":{"role":"user","content":"안녕하세요"}}"#,
        )
        .unwrap()];
        let prompt = build_prompt(&entries).unwrap();
        assert!(prompt.contains("[USER] 안녕하세요"));
        assert!(prompt.contains("[Session: s1]"));
    }

    #[test]
    fn test_build_prompt_extracts_assistant_text() {
        let entries = vec![serde_json::from_str::<LogEntry>(
            r#"{"type":"assistant","sessionId":"s1","timestamp":"2026-03-11T10:00:30Z","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"반갑습니다"}]}}"#,
        )
        .unwrap()];
        let prompt = build_prompt(&entries).unwrap();
        assert!(prompt.contains("[ASSISTANT] 반갑습니다"));
    }

    #[test]
    fn test_build_prompt_includes_thinking_blocks() {
        let entries = vec![serde_json::from_str::<LogEntry>(
            r#"{"type":"assistant","sessionId":"s1","timestamp":"2026-03-11T10:00:30Z","uuid":"a1","message":{"role":"assistant","content":[{"type":"thinking","thinking":"내부 추론"},{"type":"text","text":"보이는 텍스트"}]}}"#,
        )
        .unwrap()];
        let prompt = build_prompt(&entries).unwrap();
        assert!(prompt.contains("보이는 텍스트"));
        assert!(prompt.contains("내부 추론"));
    }

    #[test]
    fn test_build_prompt_empty_entries_returns_error() {
        let entries: Vec<LogEntry> = vec![];
        let result = build_prompt(&entries);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_prompt_groups_by_session() {
        let entries = vec![
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:00:00Z","uuid":"u1","message":{"role":"user","content":"첫 세션"}}"#,
            )
            .unwrap(),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s2","timestamp":"2026-03-11T11:00:00Z","uuid":"u2","message":{"role":"user","content":"두번째 세션"}}"#,
            )
            .unwrap(),
        ];
        let prompt = build_prompt(&entries).unwrap();
        assert!(prompt.contains("[Session: s1]"));
        assert!(prompt.contains("[Session: s2]"));
    }

    #[test]
    fn test_build_codex_prompt_extracts_conversation() {
        use crate::parser::codex::CodexEntry;
        let entries = vec![
            CodexEntry::UserMessage {
                timestamp: "2026-03-11T10:00:00Z".parse().unwrap(),
                text: "프로젝트 구조를 알려줘".to_string(),
            },
            CodexEntry::AssistantMessage {
                timestamp: "2026-03-11T10:00:30Z".parse().unwrap(),
                text: "src/ 디렉토리를 확인했습니다".to_string(),
            },
        ];
        let prompt = build_codex_prompt(&entries, "test-session").unwrap();
        assert!(prompt.contains("[USER] 프로젝트 구조를 알려줘"));
        assert!(prompt.contains("[ASSISTANT] src/ 디렉토리를 확인했습니다"));
        assert!(prompt.contains("[Session: test-session]"));
    }

    #[test]
    fn test_extract_codex_messages_includes_metadata_and_tool_entries() {
        use crate::parser::codex::CodexEntry;
        let entries = vec![
            CodexEntry::SessionMeta {
                timestamp: "2026-03-11T09:59:00Z".parse().unwrap(),
                session_id: "s1".to_string(),
                cwd: "/tmp".to_string(),
                model_provider: "openai".to_string(),
                text: "s1\n/tmp\nopenai".to_string(),
            },
            CodexEntry::UserMessage {
                timestamp: "2026-03-11T10:00:00Z".parse().unwrap(),
                text: "질문".to_string(),
            },
            CodexEntry::AssistantMessage {
                timestamp: "2026-03-11T10:00:10Z".parse().unwrap(),
                text: "답변".to_string(),
            },
            CodexEntry::FunctionCall {
                timestamp: "2026-03-11T10:00:20Z".parse().unwrap(),
                name: "Read".to_string(),
                text: "Read\nsrc/main.rs".to_string(),
            },
        ];
        let messages = extract_codex_messages(&entries);
        assert_eq!(
            messages,
            vec![
                ("META".to_string(), "s1\n/tmp\nopenai".to_string()),
                ("USER".to_string(), "질문".to_string()),
                ("ASSISTANT".to_string(), "답변".to_string()),
                ("TOOL".to_string(), "Read\nsrc/main.rs".to_string())
            ]
        );
    }

    #[test]
    fn test_extract_session_ids_dedup_preserves_order() {
        let entries = vec![
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:00:00Z","uuid":"u1","message":{"role":"user","content":"첫번째"}}"#,
            ).unwrap(),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s2","timestamp":"2026-03-11T11:00:00Z","uuid":"u2","message":{"role":"user","content":"두번째"}}"#,
            ).unwrap(),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T12:00:00Z","uuid":"u3","message":{"role":"user","content":"세번째"}}"#,
            ).unwrap(),
        ];
        let ids = extract_session_ids(&entries);
        assert_eq!(ids, vec!["s1".to_string(), "s2".to_string()]);
    }

    #[test]
    fn test_extract_session_ids_empty_entries_empty_result() {
        let entries: Vec<LogEntry> = vec![];
        let ids = extract_session_ids(&entries);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_build_codex_prompt_empty_entries_returns_error() {
        use crate::parser::codex::CodexEntry;
        let entries: Vec<CodexEntry> = vec![];
        let result = build_codex_prompt(&entries, "s1");
        assert!(result.is_err());
    }

    #[test]
    fn test_estimate_tokens_korean() {
        assert_eq!(super::estimate_tokens("안녕하세요"), 2);
    }

    #[test]
    fn test_estimate_tokens_english() {
        assert_eq!(super::estimate_tokens("hello world"), 5);
    }

    #[test]
    fn test_estimate_tokens_empty_string() {
        assert_eq!(super::estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_prompt_tokens_includes_system_overhead() {
        let tokens = estimate_prompt_tokens("hello");
        assert!(tokens > estimate_tokens("hello"));
    }

    #[test]
    fn test_estimate_sessions_per_session() {
        let entries = vec![
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:00:00Z","uuid":"u1","message":{"role":"user","content":"안녕하세요 반갑습니다"}}"#,
            ).unwrap(),
            serde_json::from_str::<LogEntry>(
                r#"{"type":"user","sessionId":"s2","timestamp":"2026-03-11T11:00:00Z","uuid":"u2","message":{"role":"user","content":"두번째 세션입니다"}}"#,
            ).unwrap(),
        ];
        let estimates = super::estimate_sessions(&entries);
        assert_eq!(estimates.len(), 2);
        assert_eq!(estimates[0].session_id, "s1");
        assert_eq!(estimates[1].session_id, "s2");
        assert!(estimates[0].estimated_tokens > 0);
        assert!(estimates[0].estimated_tokens > 0);
    }

    #[test]
    fn test_system_prompt_estimated_tokens_covers_en_prompt() {
        let estimated = estimate_tokens(SYSTEM_PROMPT_EN_TEXT);
        assert!(
            SYSTEM_PROMPT_ESTIMATED_TOKENS >= estimated,
            "SYSTEM_PROMPT_ESTIMATED_TOKENS={} must be >= EN estimate={estimated}",
            SYSTEM_PROMPT_ESTIMATED_TOKENS
        );
    }

    #[test]
    fn test_system_prompt_estimated_tokens_covers_ko_prompt() {
        let estimated = estimate_tokens(SYSTEM_PROMPT_KO_TEXT);
        assert!(
            SYSTEM_PROMPT_ESTIMATED_TOKENS >= estimated,
            "SYSTEM_PROMPT_ESTIMATED_TOKENS={} must be >= KO estimate={estimated}",
            SYSTEM_PROMPT_ESTIMATED_TOKENS
        );
    }

    #[test]
    fn test_slack_prompt_en_has_structured_guardrails() {
        assert!(SLACK_PROMPT_EN_TEXT.contains("[Today's Work Update]"));
        assert!(SLACK_PROMPT_EN_TEXT.contains("4-6"));
        assert!(SLACK_PROMPT_EN_TEXT.contains("- (Topic)"));
        assert!(SLACK_PROMPT_EN_TEXT.contains("Merge items that share the same topic"));
        assert!(SLACK_PROMPT_EN_TEXT.contains("what was done, what improved, what was solved"));
        assert!(SLACK_PROMPT_EN_TEXT.contains("No internal code/file names/paths"));
    }

    #[test]
    fn test_slack_prompt_ko_has_structured_guardrails() {
        assert!(SLACK_PROMPT_KO_TEXT.contains("[금일 작업 공유]"));
        assert!(SLACK_PROMPT_KO_TEXT.contains("4-6"));
        assert!(SLACK_PROMPT_KO_TEXT.contains("- (주제)"));
        assert!(SLACK_PROMPT_KO_TEXT.contains("같은 주제 항목은 하나의 불릿으로 병합"));
        assert!(SLACK_PROMPT_KO_TEXT.contains("한 일, 개선점, 해결한 문제"));
        assert!(SLACK_PROMPT_KO_TEXT.contains("내부 코드/파일명/경로"));
    }
}
