# Codex 세션 파서 구현 계획

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Codex CLI 세션 로그를 파싱하여 Claude Code와 동일한 인사이트 분석 파이프라인에 통합한다.

**Architecture:** `parser::codex` 모듈을 `parser::claude`와 병렬로 추가한다. Codex JSONL은 nested payload 구조이므로 2단계 파싱(loose parse → structured enum 변환)을 사용한다. 분석기는 `analyze_codex_entries()` 별도 함수로 분기하고, 출력은 같은 날짜 파일 안에 에이전트별 섹션으로 분리한다.

**Tech Stack:** Rust, serde, serde_json, chrono

---

## File Structure

| 파일 | 변경 | 역할 |
|------|------|------|
| `src/parser/codex.rs` | 생성 | Codex JSONL 파싱, 엔트리 타입, 요약 |
| `src/parser/mod.rs` | 수정 | `pub mod codex;` 추가 |
| `src/analyzer/prompt.rs` | 수정 | `build_codex_prompt()` 추가 |
| `src/analyzer/mod.rs` | 수정 | `analyze_codex_entries()` 추가 |
| `src/output/markdown.rs` | 수정 | 멀티소스 섹션 렌더링 |
| `src/output/mod.rs` | 수정 | `render_combined_markdown` re-export 추가 |
| `src/main.rs` | 수정 | Codex 수집/분석 통합, 기존 `save_analysis` 제거 |

**동작 변경 사항:** Claude 디렉토리(`~/.claude/projects/`)가 없어도 에러로 중단하지 않고 빈 결과로 진행한다. Codex 전용 사용자도 지원하기 위함이다.

---

## Chunk 1: parser::codex 모듈

### Task 1: Codex 엔트리 타입 정의 및 기본 파싱

**Files:**
- Create: `src/parser/codex.rs`
- Modify: `src/parser/mod.rs`

- [ ] **Step 1: `parser/mod.rs`에 codex 모듈 선언**

```rust
// src/parser/mod.rs — 기존 코드 끝에 추가
pub mod codex;
```

- [ ] **Step 2: Codex 엔트리 타입 정의 — 테스트 먼저**

`src/parser/codex.rs` 파일 생성. 테스트부터 작성한다.

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
Expected: FAIL — CodexRawEntry, CodexEntry 미정의

- [ ] **Step 4: 최소 타입 구현 — session_meta만**

```rust
// Codex CLI 세션 로그(.jsonl) 파서
//
// Codex JSONL은 Claude Code와 달리 nested payload 구조입니다.
// 각 줄: {"timestamp": "...", "type": "...", "payload": {...}}
// 2단계 파싱: CodexRawEntry(loose) → CodexEntry(structured)로 변환합니다.

#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use std::io::BufRead;
use std::path::{Path, PathBuf};

// === 1단계: Loose 파싱용 구조체 ===

/// JSONL 한 줄을 느슨하게 파싱하는 구조체.
/// payload를 serde_json::Value로 받아 2단계에서 타입별로 변환합니다.
#[derive(Debug, Deserialize)]
pub struct CodexRawEntry {
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "type")]
    pub entry_type: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

// === 2단계: 구조화된 엔트리 ===

/// Codex 로그의 의미 있는 엔트리만 추출한 enum.
/// Claude Code의 LogEntry와 대응하지만, Codex 고유 구조를 반영합니다.
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
    /// CodexRawEntry를 구조화된 CodexEntry로 변환합니다.
    /// payload의 내부 구조를 검사하여 적절한 variant를 선택합니다.
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

- [ ] **Step 6: user_message 파싱 테스트**

```rust
#[test]
fn test_parse_user_message_from_event_msg() {
    let json = r#"{"timestamp":"2026-03-11T03:28:22.019Z","type":"event_msg","payload":{"type":"user_message","message":"AGENTS.md 파일을 생성해줘"}}"#;
    let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
    let entry = CodexEntry::from_raw(&raw);
    assert!(matches!(entry, CodexEntry::UserMessage { .. }));
    if let CodexEntry::UserMessage { text, .. } = &entry {
        assert!(text.contains("AGENTS.md"));
    }
}
```

- [ ] **Step 7: from_raw에 event_msg → UserMessage 분기 추가**

`from_raw()` match에 추가:

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

- [ ] **Step 9: assistant message 파싱 테스트**

```rust
#[test]
fn test_parse_assistant_message_from_response_item() {
    let json = r#"{"timestamp":"2026-03-11T03:28:29.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"README를 확인했습니다."}]}}"#;
    let raw: CodexRawEntry = serde_json::from_str(json).unwrap();
    let entry = CodexEntry::from_raw(&raw);
    assert!(matches!(entry, CodexEntry::AssistantMessage { .. }));
    if let CodexEntry::AssistantMessage { text, .. } = &entry {
        assert!(text.contains("README"));
    }
}
```

- [ ] **Step 10: from_raw에 response_item → AssistantMessage 분기 추가**

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

헬퍼 함수:

```rust
/// response_item payload의 content 배열에서 output_text를 추출합니다.
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

- [ ] **Step 11: function_call 파싱 테스트**

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

- [ ] **Step 12: unknown entry 내성 테스트**

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
Expected: warning 0개

- [ ] **Step 15: Commit**

```bash
git add src/parser/codex.rs src/parser/mod.rs
git commit -m "feat: parser::codex 엔트리 타입 정의 및 기본 파싱"
```

---

### Task 2: Codex 파일 탐색 및 JSONL 파싱

**Files:**
- Modify: `src/parser/codex.rs`

- [ ] **Step 1: discover_codex_sessions_dir 테스트**

```rust
#[test]
fn test_discover_codex_sessions_dir_returns_path() {
    // 실제 ~/.codex/sessions/ 존재 여부로 검증
    let result = discover_codex_sessions_dir();
    // CI 환경에서는 없을 수 있으므로 경로 형태만 확인
    if let Ok(path) = result {
        assert!(path.ends_with("sessions"));
    }
}
```

- [ ] **Step 2: discover_codex_sessions_dir 구현**

```rust
/// ~/.codex/sessions/ 디렉토리 경로를 반환합니다.
/// Codex는 세션을 YYYY/MM/DD/ 하위에 저장합니다.
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

- [ ] **Step 3: list_session_files_for_date 테스트**

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

    // 정리
    std::fs::remove_dir_all(&temp).ok();
}
```

- [ ] **Step 4: list_session_files_for_date 구현**

```rust
/// 주어진 날짜에 해당하는 Codex 세션 파일들을 반환합니다.
/// Codex는 ~/.codex/sessions/YYYY/MM/DD/ 구조로 저장하므로
/// 디렉토리 경로로 날짜 필터링이 가능합니다.
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

- [ ] **Step 5: parse_codex_jsonl_file 테스트**

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

- [ ] **Step 6: parse_codex_jsonl_file 구현**

```rust
/// Codex JSONL 파일을 읽어 CodexEntry 벡터로 변환합니다.
/// Claude 파서와 동일한 패턴: 줄 단위로 읽고, 실패한 줄은 건너뜁니다.
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
Expected: ALL PASS, warning 0개

- [ ] **Step 8: Commit**

```bash
git add src/parser/codex.rs
git commit -m "feat: Codex 파일 탐색 및 JSONL 파싱 구현"
```

---

### Task 3: Codex 세션 요약

**Files:**
- Modify: `src/parser/codex.rs`

- [ ] **Step 1: CodexSessionSummary 테스트**

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

- [ ] **Step 2: CodexSessionSummary 구현**

```rust
/// Codex 세션의 요약 정보.
/// Claude의 SessionSummary와 대응하지만, 토큰 정보 대신 cwd/model_provider를 포함합니다.
#[derive(Debug)]
pub struct CodexSessionSummary {
    pub session_id: String,
    pub cwd: String,
    pub model_provider: String,
    pub user_count: usize,
    pub assistant_count: usize,
    pub function_call_count: usize,
}

/// Codex 엔트리들을 요약합니다.
/// Codex는 파일 하나가 세션 하나이므로, SessionMeta에서 ID를 가져옵니다.
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
git commit -m "feat: Codex 세션 요약(CodexSessionSummary) 구현"
```

---

## Chunk 2: analyzer + output + main 통합

### Task 4: Codex 프롬프트 빌더

**Files:**
- Modify: `src/analyzer/prompt.rs`

- [ ] **Step 1: build_codex_prompt 테스트**

```rust
#[test]
fn test_build_codex_prompt_extracts_conversation() {
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
fn test_build_codex_prompt_empty_entries_returns_error() {
    let entries: Vec<CodexEntry> = vec![];
    let result = build_codex_prompt(&entries, "s1");
    assert!(result.is_err());
}
```

- [ ] **Step 2: build_codex_prompt 구현**

`src/analyzer/prompt.rs` 상단에 import 추가:

```rust
use crate::parser::codex::CodexEntry;
```

함수 추가:

```rust
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
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p rwd analyzer::prompt`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add src/analyzer/prompt.rs
git commit -m "feat: Codex 대화 텍스트 프롬프트 빌더 추가"
```

---

### Task 5: analyze_codex_entries 함수

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: analyze_codex_entries 추가**

```rust
use crate::parser::codex::CodexEntry;

/// Codex 세션의 엔트리들을 분석하여 인사이트를 추출합니다.
/// Claude용 analyze_entries()와 동일한 파이프라인이지만, Codex용 프롬프트를 사용합니다.
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
Expected: 성공, warning 0개

- [ ] **Step 3: Commit**

```bash
git add src/analyzer/mod.rs
git commit -m "feat: analyze_codex_entries() 분석 함수 추가"
```

---

### Task 6: 멀티소스 Markdown 렌더링

**Files:**
- Modify: `src/output/markdown.rs`

- [ ] **Step 1: render_combined_markdown 테스트**

```rust
#[test]
fn test_render_combined_markdown_두소스_섹션_분리() {
    let claude = AnalysisResult {
        sessions: vec![SessionInsight {
            session_id: "c1".to_string(),
            work_summary: "Claude 작업".to_string(),
            decisions: vec![],
            curiosities: vec![],
            corrections: vec![],
        }],
    };
    let codex = AnalysisResult {
        sessions: vec![SessionInsight {
            session_id: "x1".to_string(),
            work_summary: "Codex 작업".to_string(),
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
    assert!(md.contains("Claude 작업"));
    assert!(md.contains("Codex 작업"));
}

#[test]
fn test_render_combined_markdown_단일소스_정상동작() {
    let claude = AnalysisResult {
        sessions: vec![SessionInsight {
            session_id: "c1".to_string(),
            work_summary: "Claude 작업".to_string(),
            decisions: vec![],
            curiosities: vec![],
            corrections: vec![],
        }],
    };
    let sources = vec![("Claude Code", &claude)];
    let date = NaiveDate::from_ymd_opt(2026, 3, 16).unwrap();
    let md = render_combined_markdown(&sources, date);

    assert!(md.contains("## Claude Code"));
    assert!(md.contains("Claude 작업"));
}
```

- [ ] **Step 2: render_combined_markdown 구현**

```rust
/// 여러 소스의 분석 결과를 하나의 Markdown으로 결합합니다.
/// 각 소스는 ## 헤딩으로 구분됩니다.
///
/// sources: (소스 이름, 분석 결과) 튜플의 슬라이스.
/// 향후 새로운 에이전트 추가 시 sources에 추가하면 됩니다.
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

- [ ] **Step 3: `output/mod.rs`에 re-export 추가**

`src/output/mod.rs`에 추가:

```rust
pub use markdown::render_combined_markdown;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p rwd output::markdown`
Expected: ALL PASS (기존 테스트 + 새 테스트)

- [ ] **Step 5: Commit**

```bash
git add src/output/markdown.rs src/output/mod.rs
git commit -m "feat: 멀티소스 Markdown 렌더링(render_combined_markdown) 추가"
```

---

### Task 7: main.rs 통합

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: run_today()에 Codex 수집/분석 통합**

`run_today()` 함수를 수정:

```rust
async fn run_today() -> Result<(), parser::ParseError> {
    update::notify_if_update_available().await;

    if config::load_config_if_exists().is_none() {
        eprintln!("설정 파일이 없습니다. 먼저 `rwd init`을 실행해 주세요.");
        std::process::exit(1);
    }

    let today = chrono::Utc::now().date_naive();

    // === Claude Code 로그 수집 ===
    let claude_entries = collect_claude_entries(today);

    // === Codex 로그 수집 ===
    let codex_sessions = collect_codex_sessions(today);

    if claude_entries.is_empty() && codex_sessions.is_empty() {
        println!("No log entries found for today ({today}).");
        return Ok(());
    }

    // === Claude Code 요약 출력 ===
    if !claude_entries.is_empty() {
        let summaries = parser::summarize_entries(&claude_entries);
        println!("\n=== Claude Code ({today}) ===");
        println!("Sessions: {}", summaries.len());
        for s in &summaries {
            print_claude_summary(s);
        }
    }

    // === Codex 요약 출력 ===
    if !codex_sessions.is_empty() {
        println!("\n=== Codex ({today}) ===");
        println!("Sessions: {}", codex_sessions.len());
        for (summary, _) in &codex_sessions {
            print_codex_summary(summary);
        }
    }

    // === LLM 분석 ===
    let provider_label = analyzer::provider::load_provider()
        .map(|(p, _)| p.display_name().to_string())
        .unwrap_or_else(|_| "LLM".to_string());
    println!("\n{provider_label} API로 인사이트 분석 중...");

    let mut sources: Vec<(&str, analyzer::AnalysisResult)> = Vec::new();

    // Claude 분석
    if !claude_entries.is_empty() {
        match analyzer::analyze_entries(&claude_entries).await {
            Ok(result) => sources.push(("Claude Code", result)),
            Err(e) => eprintln!("Claude Code 분석 실패: {e}"),
        }
    }

    // Codex 분석 — 세션별로 개별 분석
    for (summary, entries) in &codex_sessions {
        match analyzer::analyze_codex_entries(entries, &summary.session_id).await {
            Ok(result) => sources.push(("Codex", result)),
            Err(e) => eprintln!("Codex 분석 실패 ({}): {e}", &summary.session_id[..8.min(summary.session_id.len())]),
        }
    }

    // 결과 출력 및 저장
    if !sources.is_empty() {
        for (name, analysis) in &sources {
            println!("\n=== {name} 인사이트 ===");
            print_insights(analysis);
        }
        save_combined_analysis(&sources, today);
    }

    Ok(())
}
```

- [ ] **Step 2: 헬퍼 함수들 추가**

```rust
/// Claude Code 로그를 수집합니다. 디렉토리가 없으면 빈 Vec을 반환합니다.
/// 기존 run_today()는 디렉토리 부재 시 에러로 중단했지만,
/// Codex 전용 사용자도 지원하기 위해 빈 결과로 진행합니다.
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

/// Codex 세션 로그를 수집합니다. 디렉토리가 없으면 빈 Vec을 반환합니다.
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
            // 대화 내용이 있는 세션만 포함
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

/// 여러 소스의 분석 결과를 결합하여 Markdown으로 저장합니다.
fn save_combined_analysis(
    sources: &[(&str, analyzer::AnalysisResult)],
    date: chrono::NaiveDate,
) {
    let vault_path = match output::load_vault_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Vault 경로 로드 실패: {e}");
            return;
        }
    };

    // (&str, &AnalysisResult) 슬라이스로 변환
    let source_refs: Vec<(&str, &analyzer::AnalysisResult)> = sources
        .iter()
        .map(|(name, result)| (*name, result))
        .collect();

    let markdown = output::markdown::render_combined_markdown(&source_refs, date);

    match output::save_to_vault(&vault_path, date, &markdown) {
        Ok(saved) => println!("\nMarkdown 저장 완료: {}", saved.display()),
        Err(e) => eprintln!("파일 저장 실패: {e}"),
    }
}
```

- [ ] **Step 3: 기존 `save_analysis` 함수 및 `render_markdown` re-export 정리**

`src/main.rs`에서 기존 `save_analysis()` 함수를 삭제합니다 (save_combined_analysis로 대체됨).
`src/output/mod.rs`에서 `pub use markdown::render_markdown;` 행을 삭제합니다 (render_combined_markdown으로 대체됨).

> 참고: `render_markdown` 함수 자체는 `render_combined_markdown`이 내부적으로 사용하는 `render_session`, `render_til_section`과 같은 모듈에 있으므로 삭제하지 않습니다. 외부 re-export만 제거합니다.

- [ ] **Step 4: Run build + all tests + clippy**

Run: `cargo build -p rwd && cargo test -p rwd && cargo clippy -p rwd`
Expected: ALL PASS, warning 0개

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/output/mod.rs
git commit -m "feat: Codex 세션 수집/분석을 run_today()에 통합"
```

---

### Task 8: 문서 업데이트

**Files:**
- Modify: `docs/ARCHITECTURE.md`

- [ ] **Step 1: ARCHITECTURE.md에 Codex 소스 정보 업데이트**

기존 "Codex (추후 확장)" 부분을 실제 구현 내용으로 교체:

```markdown
### Codex

- 로그 위치: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`
- 형식: 각 줄이 `{"timestamp", "type", "payload"}` 구조의 JSON 객체
- 엔트리 타입: session_meta, response_item, event_msg, turn_context
- 파서: 2단계 변환 (CodexRawEntry → CodexEntry)
```

프로젝트 구조에 추가:

```
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── claude.rs      # Claude Code 로그 파서
│   │   └── codex.rs       # Codex 로그 파서
```

- [ ] **Step 2: Commit**

```bash
git add docs/ARCHITECTURE.md
git commit -m "docs: Codex 파서 아키텍처 문서 업데이트"
```
