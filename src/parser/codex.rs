// Codex CLI 세션 로그(.jsonl) 파서
//
// Codex JSONL은 Claude Code와 달리 nested payload 구조입니다.
// 각 줄: {"timestamp": "...", "type": "...", "payload": {...}}
// 2단계 파싱: CodexRawEntry(loose) → CodexEntry(structured)로 변환합니다.

// M3(LLM 분석)에서 활용할 예정인 필드들이 있으므로 dead_code 경고를 허용합니다.
// #![...]은 "이 모듈 전체에 속성을 적용"하는 내부 속성입니다 (#[...]은 다음 항목에만 적용).
#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use std::io::BufRead;
use std::path::{Path, PathBuf};

// === 1단계: loose 파싱 타입 ===

/// JSONL 한 줄의 느슨한(loose) 표현.
/// payload를 serde_json::Value로 받아 구조 변화에 유연하게 대응합니다.
/// 2단계에서 CodexEntry로 변환됩니다.
#[derive(Debug, Deserialize)]
pub struct CodexRawEntry {
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "type")]
    pub entry_type: String,
    // serde(default)는 필드가 없을 때 기본값(빈 JSON 객체)을 사용합니다 (Rust Book Ch.9 참조).
    #[serde(default)]
    pub payload: serde_json::Value,
}

// === 2단계: 구조화된 enum 타입 ===

/// Codex 세션 로그의 각 엔트리를 의미 있는 variant로 구분합니다.
/// Claude Code의 LogEntry와 달리, Codex는 "type" 필드가 두 곳에 있어 2단계 변환이 필요합니다.
/// - 최상위 type: 엔트리 종류 (session_meta, event_msg, response_item 등)
/// - payload 내부 type: 세부 종류 (user_message, message, function_call 등)
#[derive(Debug)]
pub enum CodexEntry {
    /// 세션 시작 메타데이터: 세션 ID, 작업 디렉토리, 모델 제공자
    SessionMeta {
        timestamp: DateTime<Utc>,
        session_id: String,
        cwd: String,
        model_provider: String,
    },
    /// 사용자가 입력한 메시지
    UserMessage {
        timestamp: DateTime<Utc>,
        text: String,
    },
    /// 어시스턴트(AI)의 텍스트 응답
    AssistantMessage {
        timestamp: DateTime<Utc>,
        text: String,
    },
    /// AI가 호출한 함수(도구) 이름
    FunctionCall {
        timestamp: DateTime<Utc>,
        name: String,
    },
    /// 알 수 없거나 아직 처리하지 않는 엔트리
    Other,
}

impl CodexEntry {
    /// CodexRawEntry를 의미 있는 CodexEntry variant로 변환합니다.
    ///
    /// serde_json::Value의 인덱싱(&value["key"])은 키가 없으면 Value::Null을 반환합니다.
    /// .as_str()은 Value::String인 경우 &str을 반환하고, 아닌 경우 None을 반환합니다.
    /// .unwrap_or_default()는 None일 때 빈 문자열("")을 사용합니다 (Rust Book Ch.6.1 참조).
    ///
    /// 필드 누락 시 빈 문자열로 대체하는 이유: Codex JSONL 스키마가 비공식이므로
    /// 버전에 따라 필드가 없을 수 있습니다. 파싱 실패보다 빈 값으로 계속 진행하는 것이 낫습니다.
    pub fn from_raw(raw: CodexRawEntry) -> Self {
        let ts = raw.timestamp;
        let payload = &raw.payload;

        match raw.entry_type.as_str() {
            "session_meta" => {
                // payload에서 세션 ID, 작업 디렉토리, 모델 제공자를 추출합니다.
                let session_id = payload["id"].as_str().unwrap_or_default().to_string();
                let cwd = payload["cwd"].as_str().unwrap_or_default().to_string();
                let model_provider = payload["model_provider"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                CodexEntry::SessionMeta {
                    timestamp: ts,
                    session_id,
                    cwd,
                    model_provider,
                }
            }
            // event_msg 중 payload.type == "user_message"인 경우만 사용자 메시지로 처리합니다.
            "event_msg" if payload["type"].as_str() == Some("user_message") => {
                let text = payload["message"].as_str().unwrap_or_default().to_string();
                CodexEntry::UserMessage { timestamp: ts, text }
            }
            // response_item 중 payload.type == "message" && role == "assistant"인 경우 어시스턴트 응답.
            "response_item"
                if payload["type"].as_str() == Some("message")
                    && payload["role"].as_str() == Some("assistant") =>
            {
                let text = extract_codex_output_text(payload);
                CodexEntry::AssistantMessage { timestamp: ts, text }
            }
            // response_item 중 payload.type == "function_call"인 경우 도구 호출.
            "response_item" if payload["type"].as_str() == Some("function_call") => {
                let name = payload["name"].as_str().unwrap_or_default().to_string();
                CodexEntry::FunctionCall { timestamp: ts, name }
            }
            // 위의 패턴에 해당하지 않는 모든 엔트리는 Other로 처리합니다.
            _ => CodexEntry::Other,
        }
    }
}

/// payload의 content 배열에서 type == "output_text"인 블록의 텍스트를 추출합니다.
///
/// JSON 배열을 순회하며 조건에 맞는 텍스트를 이어 붙입니다.
/// .as_array()는 Value::Array인 경우 &Vec<Value>를 반환하고, 아닌 경우 None을 반환합니다.
fn extract_codex_output_text(payload: &serde_json::Value) -> String {
    let Some(content) = payload["content"].as_array() else {
        // let-else: 패턴 매칭 실패 시 else 블록을 실행합니다 (Rust Book Ch.18 참조).
        // 여기서는 content 배열이 없으면 빈 문자열을 반환합니다.
        return String::new();
    };

    content
        .iter()
        .filter(|block| block["type"].as_str() == Some("output_text"))
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("")
}

// === 파일 탐색 함수 ===

/// ~/.codex/sessions/ 디렉토리의 경로를 반환합니다.
/// Codex CLI는 세션 로그를 이 경로에 저장합니다.
pub fn discover_codex_sessions_dir() -> Result<PathBuf, super::ParseError> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".codex").join("sessions"))
}

/// 특정 날짜의 세션 파일 목록을 반환합니다.
///
/// Codex는 세션을 YYYY/MM/DD/ 형태의 하위 디렉토리에 저장합니다.
/// 예: ~/.codex/sessions/2026/03/16/*.jsonl
///
/// format! 매크로로 두 자리 수 날짜 형식(01, 02, ...)을 만듭니다.
pub fn list_session_files_for_date(
    sessions_dir: &Path,
    date: NaiveDate,
) -> Result<Vec<PathBuf>, super::ParseError> {
    // 날짜를 YYYY/MM/DD 경로로 변환합니다.
    // chrono의 format()은 이미 올바른 자릿수를 반환합니다 (%Y=4자리, %m/%d=2자리).
    let date_path = sessions_dir
        .join(date.format("%Y").to_string())
        .join(date.format("%m").to_string())
        .join(date.format("%d").to_string());

    // 디렉토리가 없으면 빈 목록을 반환합니다 (해당 날짜에 세션이 없는 경우).
    if !date_path.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(&date_path)? {
        let entry = entry?;
        let path = entry.path();
        // .jsonl 확장자를 가진 파일만 선택합니다.
        // if let chains: Rust 2024 Edition의 let 조건 연결 문법입니다.
        if path.is_file()
            && let Some(ext) = path.extension()
            && ext == "jsonl"
        {
            files.push(path);
        }
    }

    Ok(files)
}

/// 로컬 타임존 기준 날짜에 해당하는 Codex 세션 파일들을 반환합니다.
///
/// Codex 디렉토리 구조(YYYY/MM/DD)가 UTC 기준일 수 있으므로,
/// 타임존 오프셋을 고려하여 전날 디렉토리도 함께 스캔합니다.
/// 예: KST 2026-03-16 00:00 = UTC 2026-03-15 15:00 → 03/15 디렉토리도 확인
pub fn list_session_files_for_local_date(
    sessions_dir: &Path,
    local_date: NaiveDate,
) -> Result<Vec<PathBuf>, super::ParseError> {
    let mut files = Vec::new();
    // chrono::Duration::days()는 일 단위의 기간을 생성합니다 (Rust Book Ch.10 참조).
    let yesterday = local_date - chrono::Duration::days(1);
    for date in [yesterday, local_date] {
        files.extend(list_session_files_for_date(sessions_dir, date)?);
    }
    Ok(files)
}

/// CodexEntry에서 timestamp를 추출하는 헬퍼 함수.
/// 세션의 로컬 날짜 필터링에 사용합니다.
pub fn entry_local_date(entry: &CodexEntry) -> Option<NaiveDate> {
    let ts = match entry {
        CodexEntry::SessionMeta { timestamp, .. }
        | CodexEntry::UserMessage { timestamp, .. }
        | CodexEntry::AssistantMessage { timestamp, .. }
        | CodexEntry::FunctionCall { timestamp, .. } => *timestamp,
        CodexEntry::Other => return None,
    };
    // UTC 타임스탬프를 시스템 로컬 타임존으로 변환하여 날짜를 추출합니다.
    Some(ts.with_timezone(&chrono::Local).date_naive())
}

/// JSONL 파일을 읽어 CodexEntry 벡터로 변환합니다.
///
/// 파싱에 실패한 줄은 건너뛰고 eprintln으로 경고합니다.
/// Claude Code의 parse_jsonl_file과 같은 방어적 파싱 전략을 따릅니다.
pub fn parse_codex_jsonl_file(path: &Path) -> Result<Vec<CodexEntry>, super::ParseError> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }

        // 2단계 파싱: raw JSON → CodexRawEntry → CodexEntry
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

// === 세션 요약 ===

/// Codex 세션 한 개의 요약 정보.
/// Claude Code의 SessionSummary에 대응하며, Codex 특유의 필드(cwd, model_provider)를 추가합니다.
#[derive(Debug)]
pub struct CodexSessionSummary {
    pub session_id: String,
    pub cwd: String,
    pub model_provider: String,
    pub user_count: usize,
    pub assistant_count: usize,
    pub function_call_count: usize,
}

/// 파싱된 엔트리들을 순회하여 세션 요약을 생성합니다.
///
/// Codex 파일은 보통 단일 세션이므로 Vec 대신 단일 구조체를 반환합니다.
/// SessionMeta가 없는 경우 기본값으로 빈 문자열을 사용합니다.
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
                // 첫 번째 SessionMeta로 세션 정보를 초기화합니다.
                // 이미 설정된 경우에는 덮어쓰지 않습니다.
                if summary.session_id.is_empty() {
                    summary.session_id.clone_from(session_id);
                    summary.cwd.clone_from(cwd);
                    summary.model_provider.clone_from(model_provider);
                }
            }
            CodexEntry::UserMessage { .. } => summary.user_count += 1,
            CodexEntry::AssistantMessage { .. } => summary.assistant_count += 1,
            CodexEntry::FunctionCall { .. } => summary.function_call_count += 1,
            CodexEntry::Other => {}
        }
    }

    summary
}

// === 단위 테스트 ===

// #[cfg(test)]는 "cargo test 실행 시에만 컴파일"하라는 조건부 컴파일 속성입니다 (Rust Book Ch.11 참조).
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // === Task 1: 엔트리 타입 파싱 테스트 ===

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
    fn test_parse_user_message_from_event_msg() {
        let json = r#"{"timestamp":"2026-03-16T09:01:00Z","type":"event_msg","payload":{"type":"user_message","message":"fix the bug"}}"#;
        let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
        let entry = CodexEntry::from_raw(raw);

        if let CodexEntry::UserMessage { text, .. } = entry {
            assert_eq!(text, "fix the bug");
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
            assert_eq!(text, "Sure, I'll fix it. Done.");
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
        let json = r#"{"timestamp":"2026-03-16T09:04:00Z","type":"unknown_future_type","payload":{}}"#;
        let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
        let entry = CodexEntry::from_raw(raw);
        assert!(matches!(entry, CodexEntry::Other));
    }

    // === Task 2: 파일 탐색 테스트 ===

    #[test]
    fn test_discover_codex_sessions_dir_returns_path() {
        // 홈 디렉토리 없이도 경로 구조가 올바른지 확인합니다.
        // 실제 디렉토리 존재 여부는 검사하지 않습니다.
        let result = discover_codex_sessions_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        // 경로가 "sessions"으로 끝나는지 확인합니다.
        assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("sessions"));
    }

    #[test]
    fn test_list_session_files_for_date_with_temp_dir() {
        // 임시 디렉토리를 만들어 Codex의 날짜별 디렉토리 구조를 시뮬레이션합니다.
        // tempfile 크레이트 없이 std::env::temp_dir()를 사용합니다.
        let base = std::env::temp_dir().join(format!(
            "rwd_test_codex_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let date_dir = base.join("2026").join("03").join("16");
        std::fs::create_dir_all(&date_dir).unwrap();

        // .jsonl 파일 2개와 .txt 파일 1개를 생성합니다.
        let f1 = date_dir.join("session1.jsonl");
        let f2 = date_dir.join("session2.jsonl");
        let f3 = date_dir.join("not_a_session.txt");
        std::fs::File::create(&f1).unwrap();
        std::fs::File::create(&f2).unwrap();
        std::fs::File::create(&f3).unwrap();

        let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
        let files = list_session_files_for_date(&base, date).unwrap();

        // .jsonl 파일 2개만 반환되어야 합니다.
        assert_eq!(files.len(), 2);
        // 모두 .jsonl 확장자를 가져야 합니다.
        assert!(files
            .iter()
            .all(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl")));

        // 임시 디렉토리 정리
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_parse_codex_jsonl_file_with_mixed_entries() {
        // 임시 JSONL 파일을 만들어 파싱을 검증합니다.
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

        // 유효한 엔트리 3개와 잘못된 줄 1개를 포함합니다.
        writeln!(file, r#"{{"timestamp":"2026-03-16T09:00:00Z","type":"session_meta","payload":{{"id":"s1","cwd":"/p","model_provider":"openai"}}}}"#).unwrap();
        writeln!(file, r#"{{"timestamp":"2026-03-16T09:01:00Z","type":"event_msg","payload":{{"type":"user_message","message":"hello"}}}}"#).unwrap();
        writeln!(file, r#"{{"timestamp":"2026-03-16T09:02:00Z","type":"response_item","payload":{{"type":"function_call","name":"shell","arguments":"{{}}"}}}}"#).unwrap();
        writeln!(file, "not valid json").unwrap();

        let entries = parse_codex_jsonl_file(&file_path).unwrap();

        // 유효한 줄 3개만 파싱되어야 합니다 (잘못된 줄은 건너뜁니다).
        assert_eq!(entries.len(), 3);
        assert!(matches!(entries[0], CodexEntry::SessionMeta { .. }));
        assert!(matches!(entries[1], CodexEntry::UserMessage { .. }));
        assert!(matches!(entries[2], CodexEntry::FunctionCall { .. }));

        std::fs::remove_dir_all(&base).unwrap();
    }

    // === Task 3: 세션 요약 테스트 ===

    #[test]
    fn test_summarize_codex_entries_counts_correctly() {
        // 다양한 엔트리 타입으로 구성된 슬라이스를 만들어 요약을 검증합니다.
        // from_raw를 통해 실제 파싱 경로를 사용합니다.
        let raws = vec![
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
}
