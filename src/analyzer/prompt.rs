// LogEntry 슬라이스를 Claude API에 보낼 프롬프트 텍스트로 변환하는 모듈.
//
// 네트워크 호출 없이 순수 데이터 변환만 수행하므로 단위 테스트가 용이합니다.
// Text 블록만 포함하고, Thinking/ToolUse/ToolResult는 제외합니다 (너무 장황하므로).

use crate::parser::claude::{ContentBlock, LogEntry};
use crate::parser::codex::CodexEntry;
use std::collections::HashMap;

/// LogEntry 슬라이스를 받아 Claude API에 보낼 프롬프트 텍스트를 생성합니다.
/// 세션별로 그룹화하여 [Session: id] 헤더와 [USER]/[ASSISTANT] 태그를 붙입니다.
pub fn build_prompt(entries: &[LogEntry]) -> Result<String, super::AnalyzerError> {
    let conversation_text = extract_conversation_text(entries);
    if conversation_text.is_empty() {
        return Err("로그 엔트리에서 대화 내용을 찾을 수 없습니다.".into());
    }
    Ok(conversation_text)
}

/// LogEntry에서 사람이 읽을 수 있는 대화 텍스트를 추출합니다.
///
/// HashMap으로 세션별 메시지를 그룹화한 뒤, 각 세션을 텍스트로 변환합니다.
/// Vec<(timestamp, role, text)> 형태로 저장하여 시간순 정렬을 유지합니다.
fn extract_conversation_text(entries: &[LogEntry]) -> String {
    // 세션별로 메시지를 그룹화합니다.
    // 튜플의 첫 번째 요소(timestamp)는 정렬용이지만, JSONL이 이미 시간순이므로
    // 순서를 유지하기 위해 Vec을 사용합니다.
    let mut sessions: HashMap<&str, Vec<(&str, String)>> = HashMap::new();
    // 세션 순서를 보존하기 위한 벡터 (HashMap은 순서를 보장하지 않습니다).
    let mut session_order: Vec<&str> = Vec::new();

    for entry in entries {
        match entry {
            LogEntry::User(e) => {
                if let Some(text) = e.message.as_ref().and_then(extract_user_text) {
                    let session_id = e.session_id.as_str();
                    if !sessions.contains_key(session_id) {
                        session_order.push(session_id);
                    }
                    sessions
                        .entry(session_id)
                        .or_default()
                        .push(("USER", text));
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
            // Progress, System, FileHistorySnapshot, Other는 대화 내용이 아니므로 건너뜁니다.
            _ => {}
        }
    }

    // 각 세션을 텍스트로 변환합니다.
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

/// UserEntry의 message (serde_json::Value)에서 텍스트를 추출합니다.
///
/// Claude Code 로그에서 user message는 두 가지 형태입니다:
/// 1. {"role":"user","content":"텍스트"} — content가 문자열
/// 2. {"role":"user","content":[{"type":"text","text":"텍스트"}]} — content가 배열
///
/// serde_json::Value의 .as_str()는 문자열이면 Some(&str), 아니면 None을 반환합니다.
/// .as_array()는 배열이면 Some(&Vec<Value>), 아니면 None을 반환합니다.
fn extract_user_text(value: &serde_json::Value) -> Option<String> {
    let content = value.get("content")?;

    // content가 문자열인 경우
    // let chains로 조건을 결합합니다 (Rust 2024 Edition).
    if let Some(text) = content.as_str()
        && !text.is_empty()
    {
        return Some(text.to_string());
    }

    // content가 배열인 경우 — 텍스트 블록만 추출하여 결합
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

/// AssistantEntry의 ContentBlock들에서 텍스트를 추출합니다.
///
/// Text 블록만 포함하고 Thinking, ToolUse, ToolResult는 건너뜁니다.
/// Thinking은 모델의 내부 추론이고, ToolUse/ToolResult는 너무 장황하여
/// 인사이트 추출에 노이즈가 됩니다.
fn extract_assistant_text(blocks: &[ContentBlock]) -> String {
    let texts: Vec<&str> = blocks
        .iter()
        .filter_map(|block| {
            // if let으로 Text variant만 매칭합니다 (Rust Book Ch.6.3 참조).
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

/// Codex 엔트리들을 LLM 분석용 대화 텍스트로 변환합니다.
/// Codex는 파일 하나가 세션 하나이므로, session_id를 외부에서 전달받습니다.
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

    // 세션 헤더만 있고 대화 내용이 없는 경우
    if !output.contains("[USER]") && !output.contains("[ASSISTANT]") {
        return Err("Codex 로그에서 대화 내용을 찾을 수 없습니다.".into());
    }

    Ok(output)
}

/// LogEntry 슬라이스에서 고유한 세션 ID 목록을 추출합니다.
/// 등장 순서를 유지하며 중복을 제거합니다.
/// fallback 시 세션별로 엔트리를 분할하기 위해 사용합니다.
pub fn extract_session_ids(entries: &[LogEntry]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in entries {
        // User/Assistant/Progress는 session_id: String
        // System은 session_id: Option<String>
        // FileHistorySnapshot은 session_id 필드가 없음
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
    fn test_extract_session_ids_중복_제거_순서_유지() {
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
    fn test_extract_session_ids_빈_엔트리_빈_결과() {
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
}
