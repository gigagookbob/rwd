// Claude Code 세션 로그(.jsonl) 파서
//
// JSONL 파일의 각 줄은 독립된 JSON 객체이며, "type" 필드로 종류가 구분됩니다.
// serde 크레이트를 사용하여 JSON을 Rust 구조체로 자동 변환(역직렬화)합니다.

// serde 역직렬화를 위해 필드를 선언하지만, M2에서는 일부 필드를 아직 읽지 않습니다.
// M3(LLM 분석)에서 활용할 예정이므로 dead_code 경고를 허용합니다.
// #![...]은 "이 모듈 전체에 속성을 적용"하는 내부 속성입니다 (#[...]은 다음 항목에만 적용).
#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};

// === 데이터 타입 정의 ===

/// JSONL 파일의 각 줄을 나타내는 열거형(enum).
/// enum은 "여러 가능한 형태 중 하나"를 표현합니다 (Rust Book Ch.6 참조).
///
/// #[serde(tag = "type")]은 JSON의 "type" 필드 값으로 variant를 결정합니다.
/// 예: {"type": "user", ...} → LogEntry::User(UserEntry { ... })
///
/// rename_all = "kebab-case"는 PascalCase variant 이름을 kebab-case로 변환합니다.
/// 예: FileHistorySnapshot → "file-history-snapshot"
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LogEntry {
    User(UserEntry),
    Assistant(AssistantEntry),
    Progress(ProgressEntry),
    System(SystemEntry),
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot(FileHistorySnapshotEntry),
    // 새로운 로그 타입이 추가되어도 파싱이 실패하지 않도록 catch-all variant를 둡니다.
    // ContentBlock의 #[serde(other)]와 같은 역할입니다.
    #[serde(untagged)]
    Other(serde_json::Value),
}

/// 사용자 메시지 엔트리
/// #[serde(rename_all = "camelCase")]는 Rust의 snake_case 필드를 JSON의 camelCase에 매핑합니다.
/// 예: session_id ↔ "sessionId"
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub uuid: String,
    // Option<T>는 "값이 있거나 없거나"를 표현합니다 (Rust Book Ch.6.1 참조).
    // JSON에서 해당 필드가 없으면 None이 됩니다 (#[serde(default)] 덕분).
    #[serde(default)]
    pub message: Option<serde_json::Value>,
}

/// 어시스턴트(AI) 응답 엔트리
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub uuid: String,
    #[serde(default)]
    pub message: Option<AssistantMessage>,
}

/// 어시스턴트 메시지의 상세 구조
#[derive(Debug, Clone, Deserialize)]
pub struct AssistantMessage {
    #[serde(default)]
    pub model: Option<String>,
    // Vec<T>는 가변 길이 배열입니다 (Rust Book Ch.8.1 참조).
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// 어시스턴트 메시지의 content 배열 안에 올 수 있는 블록 타입들.
/// #[serde(tag = "type")]으로 "type" 필드 값에 따라 variant가 결정됩니다.
/// rename_all = "snake_case"는 PascalCase를 snake_case로 변환합니다.
/// 예: ToolUse → "tool_use"
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
    // #[serde(other)]는 알 수 없는 타입을 이 variant로 매핑합니다.
    // 새로운 블록 타입이 추가되어도 파싱이 실패하지 않습니다.
    #[serde(other)]
    Unknown,
}

/// API 토큰 사용량
/// Claude API는 캐시 히트/생성 토큰을 별도 필드로 분리합니다.
/// 총 입력 토큰 = input_tokens + cache_creation_input_tokens + cache_read_input_tokens
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    // 캐시에 새로 저장된 입력 토큰 수
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    // 캐시에서 읽어온 입력 토큰 수
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

/// 진행 상황 엔트리 (에이전트 작업 진행 등)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
}

/// 시스템 엔트리
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemEntry {
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// 파일 히스토리 스냅샷 엔트리 (M2에서는 상세 분석 불필요)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistorySnapshotEntry {
    #[serde(default)]
    pub message_id: Option<String>,
}

/// 세션 로그의 요약 정보를 담는 구조체
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

// === 파일 탐색 함수 ===

/// ~/.claude/projects/ 디렉토리의 경로를 반환합니다.
/// dirs 크레이트는 OS별 홈 디렉토리를 크로스플랫폼으로 찾아줍니다.
///
/// .ok_or()는 Option을 Result로 변환합니다 (Rust Book Ch.9 참조).
/// Some(value) → Ok(value), None → Err(에러메시지)
///
/// .into()는 String을 Box<dyn Error>로 변환합니다.
/// Rust의 From 트레이트 덕분에 가능합니다 (Rust Book Ch.10 참조).
pub fn discover_log_dir() -> Result<PathBuf, super::ParseError> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;

    let claude_projects = home.join(".claude").join("projects");

    if !claude_projects.exists() {
        return Err(format!(
            "Claude projects directory not found: {}",
            claude_projects.display()
        )
        .into());
    }

    Ok(claude_projects)
}

/// ~/.claude/projects/ 아래의 모든 프로젝트 디렉토리를 반환합니다.
/// std::fs::read_dir()은 디렉토리의 엔트리를 순회하는 이터레이터를 반환합니다 (Rust Book Ch.12 참조).
/// 각 엔트리는 Result<DirEntry>이므로 ?로 에러를 전파합니다.
pub fn list_project_dirs() -> Result<Vec<PathBuf>, super::ParseError> {
    let base = discover_log_dir()?;
    let mut dirs = Vec::new();

    for entry in std::fs::read_dir(&base)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }

    Ok(dirs)
}

/// 특정 프로젝트 디렉토리 안의 모든 .jsonl 파일을 반환합니다.
/// &Path는 경로의 빌림(borrow)입니다 — 소유권을 가져가지 않습니다 (Rust Book Ch.4 참조).
/// PathBuf : String = Path : &str 관계입니다 (소유 vs 빌림).
pub fn list_session_files(project_dir: &Path) -> Result<Vec<PathBuf>, super::ParseError> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(project_dir)? {
        let entry = entry?;
        let path = entry.path();
        // .jsonl 확장자를 가진 파일만 선택합니다.
        // if let과 &&를 결합하면 여러 조건을 한 줄에 표현할 수 있습니다 (Rust 2024 Edition).
        // let chains: if 조건 안에서 패턴 매칭과 불리언 조건을 연결합니다.
        if path.is_file()
            && let Some(ext) = path.extension()
            && ext == "jsonl"
        {
            files.push(path);
        }
    }

    Ok(files)
}

// === JSONL 파싱 함수 ===

/// 하나의 JSONL 파일을 읽어서 LogEntry 벡터로 변환합니다.
/// 파싱에 실패한 줄은 건너뛰고 eprintln으로 경고합니다.
///
/// BufReader는 파일을 한 줄씩 효율적으로 읽습니다 (Rust Book Ch.12 참조).
/// .lines()는 각 줄을 Result<String>으로 반환하는 이터레이터입니다.
/// .enumerate()는 (인덱스, 값) 쌍을 반환합니다 (Rust Book Ch.13 참조).
pub fn parse_jsonl_file(path: &Path) -> Result<Vec<LogEntry>, super::ParseError> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }

        // match로 파싱 결과를 명시적으로 처리합니다 (Rust Book Ch.6.2 참조).
        // Ok이면 entries에 추가, Err이면 경고 출력 후 다음 줄로 진행합니다.
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

// === 필터링 및 요약 함수 ===

/// LogEntry에서 timestamp를 추출하는 헬퍼 함수.
/// 모든 variant가 timestamp를 가지지는 않으므로 Option을 반환합니다.
/// match로 enum의 모든 variant를 처리합니다 — 빠뜨리면 컴파일 에러가 납니다 (Rust Book Ch.6.2 참조).
pub fn entry_timestamp(entry: &LogEntry) -> Option<DateTime<Utc>> {
    match entry {
        LogEntry::User(e) => Some(e.timestamp),
        LogEntry::Assistant(e) => Some(e.timestamp),
        LogEntry::Progress(e) => Some(e.timestamp),
        LogEntry::System(e) => Some(e.timestamp),
        LogEntry::FileHistorySnapshot(_) | LogEntry::Other(_) => None,
    }
}

/// 주어진 날짜에 해당하는 엔트리만 필터링합니다.
///
/// .into_iter()는 Vec의 소유권을 가져가는 이터레이터입니다 (Rust Book Ch.13.2 참조).
/// .filter()는 클로저(익명 함수)를 받아 조건에 맞는 요소만 남깁니다.
/// 클로저는 |파라미터| { 본문 } 형태로 작성합니다 (Rust Book Ch.13.1 참조).
/// .collect()는 이터레이터를 다시 Vec로 변환합니다.
pub fn filter_entries_by_date(entries: Vec<LogEntry>, date: NaiveDate) -> Vec<LogEntry> {
    entries
        .into_iter()
        .filter(|entry| {
            // UTC 타임스탬프를 시스템 로컬 타임존으로 변환한 뒤 날짜를 비교합니다.
            // 이렇게 해야 KST 기준 "오늘"에 해당하는 엔트리가 정확히 필터링됩니다.
            // 예: KST 2026-03-16 01:00 (= UTC 2026-03-15 16:00)은 KST 3월 16일에 포함됩니다.
            match entry_timestamp(entry) {
                Some(ts) => ts.with_timezone(&chrono::Local).date_naive() == date,
                None => false,
            }
        })
        .collect()
}

/// 파싱된 엔트리들을 세션별로 그룹화하고 요약합니다.
///
/// HashMap은 키-값 쌍을 저장하는 자료구조입니다 (Rust Book Ch.8.3 참조).
/// .entry().or_insert_with()는 키가 없으면 새 값을 삽입하고,
/// 있으면 기존 값의 가변 참조를 반환합니다 — 효율적인 "있으면 업데이트, 없으면 삽입" 패턴입니다.
pub fn summarize_entries(entries: &[LogEntry]) -> Vec<SessionSummary> {
    let mut sessions: HashMap<String, SessionSummary> = HashMap::new();

    for entry in entries {
        // 세션 ID가 있는 엔트리만 처리합니다.
        let session_id = match entry {
            LogEntry::User(e) => &e.session_id,
            LogEntry::Assistant(e) => &e.session_id,
            LogEntry::Progress(e) => &e.session_id,
            // continue는 현재 반복을 건너뛰고 다음 반복으로 넘어갑니다.
            LogEntry::System(_) | LogEntry::FileHistorySnapshot(_) | LogEntry::Other(_) => continue,
        };

        // .clone()은 값의 복사본을 만듭니다 (Rust Book Ch.4 참조).
        // HashMap의 키로 사용하려면 소유된(owned) String이 필요합니다.
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
                        // matches! 매크로는 패턴 매칭의 결과를 bool로 반환합니다.
                        if matches!(block, ContentBlock::ToolUse { .. }) {
                            summary.tool_use_count += 1;
                        }
                    }
                    if let Some(usage) = &msg.usage {
                        summary.total_input_tokens += usage.input_tokens;
                        summary.total_output_tokens += usage.output_tokens;
                        summary.total_cache_creation_tokens +=
                            usage.cache_creation_input_tokens;
                        summary.total_cache_read_tokens += usage.cache_read_input_tokens;
                    }
                }
            }
            LogEntry::Progress(_) => {}
            _ => {}
        }
    }

    // .into_values()는 HashMap에서 값(Value)만 추출하는 이터레이터를 반환합니다.
    sessions.into_values().collect()
}

// === 단위 테스트 ===

// #[cfg(test)]는 "cargo test 실행 시에만 컴파일"하라는 조건부 컴파일 속성입니다 (Rust Book Ch.11 참조).
// mod tests는 테스트 전용 모듈을 정의합니다.
#[cfg(test)]
mod tests {
    // super::*는 부모 모듈(claude.rs)의 모든 항목을 가져옵니다 (Rust Book Ch.7 참조).
    use super::*;

    // r#"..."#은 raw string literal입니다 — 이스케이프 없이 " 등을 포함할 수 있습니다 (Rust Book Ch.8 참조).
    // 테스트에서는 unwrap()을 사용해도 됩니다 — 실패 시 panic으로 테스트가 실패 처리됩니다.

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
}
