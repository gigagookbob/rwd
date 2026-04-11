// Converts LogEntry slices into prompt text for LLM analysis.
//
// Pure data transformation with no network calls, making it easy to unit-test.
// Only Text blocks are included; Thinking/ToolUse/ToolResult are excluded (too verbose).

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
        match entry {
            LogEntry::User(e) => {
                if let Some(text) = e.message.as_ref().and_then(extract_user_text) {
                    let session_id = e.session_id.as_str();
                    if !sessions.contains_key(session_id) {
                        session_order.push(session_id);
                    }
                    sessions.entry(session_id).or_default().push(("USER", text));
                }
            }
            LogEntry::Assistant(e) => {
                if let Some(msg) = &e.message {
                    let text = extract_assistant_text(&msg.content);
                    if !text.is_empty() {
                        let session_id = e.session_id.as_str();
                        if !sessions.contains_key(session_id) {
                            session_order.push(session_id);
                        }
                        sessions
                            .entry(session_id)
                            .or_default()
                            .push(("ASSISTANT", text));
                    }
                }
            }
            // Skip non-conversation entries (Progress, System, FileHistorySnapshot, Other).
            _ => {}
        }
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

/// Extracts text from a UserEntry's message (serde_json::Value).
///
/// Claude Code user messages come in two forms:
/// 1. {"role":"user","content":"text"} -- content is a string
/// 2. {"role":"user","content":[{"type":"text","text":"text"}]} -- content is an array
fn extract_user_text(value: &serde_json::Value) -> Option<String> {
    let content = value.get("content")?;

    if let Some(text) = content.as_str()
        && !text.is_empty()
    {
        return Some(text.to_string());
    }

    // Content is an array -- extract and join text blocks.
    if let Some(blocks) = content.as_array() {
        let texts: Vec<&str> = blocks
            .iter()
            .filter_map(|block| {
                if block.get("type")?.as_str()? == "text" {
                    block.get("text")?.as_str()
                } else {
                    None
                }
            })
            .collect();
        if !texts.is_empty() {
            return Some(texts.join("\n"));
        }
    }

    None
}

/// Extracts text from AssistantEntry ContentBlocks.
///
/// Only includes Text blocks; skips Thinking, ToolUse, and ToolResult
/// as they add noise without contributing to insight extraction.
fn extract_assistant_text(blocks: &[ContentBlock]) -> String {
    let texts: Vec<&str> = blocks
        .iter()
        .filter_map(|block| {
            if let ContentBlock::Text { text } = block {
                text.as_deref()
            } else {
                None
            }
        })
        .filter(|t| !t.is_empty())
        .collect();
    texts.join("\n")
}

/// Extracts (role, text) tuples from LogEntries.
/// Used by summarizer's split_into_chunks.
pub fn extract_messages(entries: &[LogEntry]) -> Vec<(String, String)> {
    let mut messages = Vec::new();
    for entry in entries {
        match entry {
            LogEntry::User(e) => {
                if let Some(text) = e.message.as_ref().and_then(extract_user_text) {
                    messages.push(("USER".to_string(), text));
                }
            }
            LogEntry::Assistant(e) => {
                if let Some(msg) = &e.message {
                    let text = extract_assistant_text(&msg.content);
                    if !text.is_empty() {
                        messages.push(("ASSISTANT".to_string(), text));
                    }
                }
            }
            _ => {}
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

    // Only session header present with no conversation content.
    if !output.contains("[USER]") && !output.contains("[ASSISTANT]") {
        return Err(crate::messages::error::NO_CONVERSATION_CODEX.into());
    }

    Ok(output)
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

/// Pre-computed estimated token count of the system prompt (provider::SYSTEM_PROMPT).
pub const SYSTEM_PROMPT_ESTIMATED_TOKENS: u64 = 800;

/// Rough token estimate for text.
/// Korean syllables are ~1 token each, so char_count / 2 is a conservative estimate.
pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as u64) / 2
}

/// Returns per-session token estimates.
pub fn estimate_sessions(entries: &[LogEntry]) -> Vec<SessionEstimate> {
    let session_ids = extract_session_ids(entries);
    let mut estimates = Vec::new();

    for session_id in &session_ids {
        let mut total_chars: u64 = 0;

        for entry in entries {
            let eid = match entry {
                LogEntry::User(u) => Some(u.session_id.as_str()),
                LogEntry::Assistant(a) => Some(a.session_id.as_str()),
                LogEntry::Progress(p) => Some(p.session_id.as_str()),
                LogEntry::System(s) => s.session_id.as_deref(),
                LogEntry::FileHistorySnapshot(_) | LogEntry::Other(_) => None,
            };
            if eid != Some(session_id.as_str()) {
                continue;
            }

            match entry {
                LogEntry::User(e) => {
                    if let Some(text) = e.message.as_ref().and_then(extract_user_text) {
                        total_chars += text.chars().count() as u64;
                    }
                }
                LogEntry::Assistant(e) => {
                    if let Some(msg) = &e.message {
                        for block in &msg.content {
                            if let ContentBlock::Text { text } = block
                                && let Some(t) = text
                            {
                                total_chars += t.chars().count() as u64;
                            }
                        }
                    }
                }
                _ => {}
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
    fn test_build_prompt_ignores_thinking_blocks() {
        let entries = vec![serde_json::from_str::<LogEntry>(
            r#"{"type":"assistant","sessionId":"s1","timestamp":"2026-03-11T10:00:30Z","uuid":"a1","message":{"role":"assistant","content":[{"type":"thinking","thinking":"내부 추론"},{"type":"text","text":"보이는 텍스트"}]}}"#,
        )
        .unwrap()];
        let prompt = build_prompt(&entries).unwrap();
        assert!(prompt.contains("보이는 텍스트"));
        assert!(!prompt.contains("내부 추론"));
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
}
