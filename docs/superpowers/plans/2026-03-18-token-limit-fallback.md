# 토큰 제한 fallback 구현 계획

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Claude Code 분석 시 LLM 토큰 제한 에러가 발생하면 세션별 개별 분석으로 자동 fallback하고, 429 에러는 친절한 안내 메시지를 출력한다.

**Architecture:** `analyze_entries()`에서 API 호출 실패 시 에러 메시지를 검사하여 400(컨텍스트 초과)이면 세션별 분할 분석, 429(TPM)이면 안내 메시지를 반환한다. 세션별 분석은 기존 `build_prompt()`를 재사용하고 결과를 병합한다.

**Tech Stack:** Rust, serde_json

**Spec:** `docs/superpowers/specs/2026-03-18-token-limit-fallback-design.md`

---

## Chunk 1: 에러 판별 + 결과 병합 + 세션 ID 추출 + fallback 로직

### 파일 구조

| 파일 | 변경 | 역할 |
|------|------|------|
| `src/analyzer/mod.rs` | 수정 | `analyze_entries()` fallback 로직, 에러 판별 함수 |
| `src/analyzer/prompt.rs` | 수정 | `extract_session_ids()` 추가 |
| `src/analyzer/insight.rs` | 수정 | `merge_results()` 추가 |

---

### Task 1: 에러 판별 함수 테스트 및 구현

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: 에러 판별 테스트 작성**

`src/analyzer/mod.rs` 끝에 테스트 모듈 추가:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_context_limit_error_400_token_포함시_true() {
        let err = "API 요청 실패 (400 Bad Request): {\"error\":{\"message\":\"This model's maximum context length is 128000 tokens\"}}";
        assert!(is_context_limit_error(err));
    }

    #[test]
    fn test_is_context_limit_error_400_context_포함시_true() {
        let err = "OpenAI API 요청 실패 (400 Bad Request): {\"error\":{\"code\":\"context_length_exceeded\"}}";
        assert!(is_context_limit_error(err));
    }

    #[test]
    fn test_is_context_limit_error_429_에러는_false() {
        let err = "OpenAI API 요청 실패 (429 Too Many Requests): rate limit";
        assert!(!is_context_limit_error(err));
    }

    #[test]
    fn test_is_context_limit_error_일반_에러는_false() {
        let err = "API 요청 실패 (500 Internal Server Error): server error";
        assert!(!is_context_limit_error(err));
    }

    #[test]
    fn test_is_rate_limit_error_429_포함시_true() {
        let err = "OpenAI API 요청 실패 (429 Too Many Requests): {\"error\":{\"message\":\"Rate limit exceeded\"}}";
        assert!(is_rate_limit_error(err));
    }

    #[test]
    fn test_is_rate_limit_error_400_에러는_false() {
        let err = "API 요청 실패 (400 Bad Request): token limit";
        assert!(!is_rate_limit_error(err));
    }
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p rwd test_is_context_limit -- --nocapture`
Expected: 컴파일 에러 — 함수가 없음

- [ ] **Step 3: 에러 판별 함수 구현**

`src/analyzer/mod.rs`에서 `analyze_codex_entries` 함수 아래에 추가:

```rust
/// API 에러가 컨텍스트 윈도우 초과(400)인지 판별합니다.
/// 에러 메시지에 "400"과 ("token" 또는 "context")가 포함되면 컨텍스트 제한 에러로 판단합니다.
/// 주의: 에러 메시지 형식에 의존하므로, M5에서 구조화된 에러 타입으로 전환 예정.
fn is_context_limit_error(err_msg: &str) -> bool {
    let lower = err_msg.to_lowercase();
    lower.contains("400") && (lower.contains("token") || lower.contains("context"))
}

/// API 에러가 TPM/RPM 제한 초과(429)인지 판별합니다.
fn is_rate_limit_error(err_msg: &str) -> bool {
    err_msg.contains("429")
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test -p rwd test_is_context_limit -- --nocapture && cargo test -p rwd test_is_rate_limit -- --nocapture`
Expected: 6개 테스트 모두 PASS

- [ ] **Step 5: 커밋**

```bash
git add src/analyzer/mod.rs
git commit -m "feat: 토큰 제한 에러 판별 함수 추가 (#36)"
```

---

### Task 2: merge_results 테스트 및 구현

**Files:**
- Modify: `src/analyzer/insight.rs`

- [ ] **Step 1: merge_results 테스트 작성**

`src/analyzer/insight.rs`의 `mod tests` 블록에 추가:

```rust
    #[test]
    fn test_merge_results_여러_결과_병합() {
        let r1 = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "s1".to_string(),
                work_summary: "작업1".to_string(),
                decisions: vec![],
                curiosities: vec![],
                corrections: vec![],
                til: vec![],
            }],
        };
        let r2 = AnalysisResult {
            sessions: vec![SessionInsight {
                session_id: "s2".to_string(),
                work_summary: "작업2".to_string(),
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
    fn test_merge_results_빈_벡터_빈_결과() {
        let merged = merge_results(vec![]);
        assert!(merged.sessions.is_empty());
    }
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p rwd test_merge_results -- --nocapture`
Expected: 컴파일 에러 — 함수가 없음

- [ ] **Step 3: merge_results 구현**

`src/analyzer/insight.rs`에서 `parse_response` 함수 아래에 추가:

```rust
/// 여러 AnalysisResult를 하나로 병합합니다.
/// 각 결과의 sessions Vec을 순서대로 합칩니다.
/// fallback 시 세션별 개별 분석 결과를 하나의 결과로 조합하기 위해 사용합니다.
pub fn merge_results(results: Vec<AnalysisResult>) -> AnalysisResult {
    let sessions = results
        .into_iter()
        .flat_map(|r| r.sessions)
        .collect();
    AnalysisResult { sessions }
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test -p rwd test_merge_results -- --nocapture`
Expected: 2개 테스트 모두 PASS

- [ ] **Step 5: 모듈 주석 업데이트**

`src/analyzer/insight.rs` 1행의 주석을 변경:

```rust
// LLM API 응답을 구조화된 인사이트 타입으로 파싱하고, 분할 분석 결과를 병합하는 모듈.
```

- [ ] **Step 6: 커밋**

```bash
git add src/analyzer/insight.rs
git commit -m "feat: merge_results 결과 병합 함수 추가 (#36)"
```

---

### Task 3: LogEntry에 Clone derive 추가 (준비 작업)

**Files:**
- Modify: `src/parser/claude.rs`

- [ ] **Step 1: Clone derive 추가**

`src/parser/claude.rs`에서 아래 타입들에 `Clone`을 추가합니다 (기존 `Debug, Deserialize` 옆에):

- `LogEntry` (enum, 29행)
- `UserEntry` (45행)
- `AssistantEntry` (58행)
- `AssistantMessage` (68행 부근)
- `ContentBlock` (enum)
- `Usage` (struct)
- `ProgressEntry`
- `SystemEntry`
- `FileHistorySnapshotEntry`

`serde_json::Value`와 `String`, `DateTime<Utc>` 등 기본 타입은 이미 `Clone`을 구현하고 있으므로, derive만 추가하면 됩니다.

- [ ] **Step 2: 빌드 확인**

Run: `cargo build`
Expected: 컴파일 성공

- [ ] **Step 3: 커밋**

```bash
git add src/parser/claude.rs
git commit -m "chore: LogEntry 관련 타입에 Clone derive 추가 (#36)"
```

---

### Task 4: extract_session_ids 테스트 및 구현 (구 Task 3)

**Files:**
- Modify: `src/analyzer/prompt.rs`

- [ ] **Step 1: extract_session_ids 테스트 작성**

`src/analyzer/prompt.rs`의 `mod tests` 블록에 추가:

```rust
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
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test -p rwd test_extract_session_ids -- --nocapture`
Expected: 컴파일 에러 — 함수가 없음

- [ ] **Step 3: extract_session_ids 구현**

`src/analyzer/prompt.rs`에서 `build_codex_prompt` 함수 아래에 추가:

```rust
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
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test -p rwd test_extract_session_ids -- --nocapture`
Expected: 2개 테스트 모두 PASS

- [ ] **Step 5: clippy 확인**

Run: `cargo clippy`
Expected: warning 0개

- [ ] **Step 6: 커밋**

```bash
git add src/analyzer/prompt.rs
git commit -m "feat: extract_session_ids 세션 ID 추출 함수 추가 (#36)"
```

---

### Task 5: analyze_entries fallback 로직 구현

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: analyze_entries에 fallback 로직 추가**

`src/analyzer/mod.rs`의 `analyze_entries()` 함수를 아래로 교체:

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

            // 429 TPM 제한 → 친절한 에러 메시지
            if is_rate_limit_error(&err_msg) {
                return Err(
                    "API 요청 빈도(TPM) 제한을 초과했습니다.\n\
                     해결 방법:\n  \
                     • rwd config provider anthropic  (Anthropic으로 전환)\n  \
                     • LLM 프로바이더 플랜 업그레이드  (TPM 한도 증가)"
                        .into(),
                );
            }

            // 400 컨텍스트 제한 → 세션별 분할 fallback
            if is_context_limit_error(&err_msg) {
                eprintln!("프롬프트가 토큰 제한을 초과하여 세션별 분석으로 전환합니다...");
                return analyze_entries_by_session(
                    entries,
                    &provider,
                    &api_key,
                    redactor_enabled,
                )
                .await;
            }

            // 기타 에러 → 그대로 전파
            Err(e)
        }
    }
}
```

- [ ] **Step 2: analyze_entries_by_session 함수 구현**

`analyze_entries` 함수 아래에 추가:

```rust
/// 세션별로 엔트리를 분할하여 개별 분석 후 결과를 병합합니다.
/// 400 컨텍스트 초과 에러 발생 시 fallback으로 호출됩니다.
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
        eprintln!("  세션 {}/{total} 분석 중... ({session_id})", i + 1);

        // 해당 세션의 엔트리만 필터링하여 새 Vec으로 수집합니다.
        // clone이 필요한 이유: build_prompt()가 &[LogEntry]를 받으므로 소유권이 있는 Vec이 필요합니다.
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
                eprintln!("  세션 {session_id} 프롬프트 생성 실패: {e}");
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
                    Err(e) => eprintln!("  세션 {session_id} 응답 파싱 실패: {e}"),
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                if is_context_limit_error(&err_msg) || is_rate_limit_error(&err_msg) {
                    eprintln!("  세션 {session_id} 분석 스킵 (토큰 제한 초과)");
                } else {
                    eprintln!("  세션 {session_id} 분석 실패: {err_msg}");
                }
            }
        }
    }

    if results.is_empty() {
        return Err("모든 세션의 분석에 실패했습니다.".into());
    }

    Ok((insight::merge_results(results), total_redact))
}

/// LogEntry에서 session_id를 추출합니다.
/// SystemEntry는 Option<String>, FileHistorySnapshotEntry는 session_id 없음.
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

- [ ] **Step 4: 빌드 확인**

Run: `cargo build`
Expected: 컴파일 성공

- [ ] **Step 5: 전체 테스트 확인**

Run: `cargo test`
Expected: 모든 테스트 PASS

- [ ] **Step 6: clippy 확인**

Run: `cargo clippy`
Expected: warning 0개

- [ ] **Step 7: 커밋**

```bash
git add src/analyzer/mod.rs src/parser/claude.rs
git commit -m "feat: analyze_entries 토큰 제한 fallback 구현 (#36)"
```
