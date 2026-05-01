# Codex Subagent Session Filtering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `session_meta`에 명시된 hard signal만 사용해 Codex subagent 세션을 기본 제외하고, 일반 interactive 세션과 worktree 기반 interactive 세션은 그대로 포함한다.

**Architecture:** `src/parser/codex.rs`의 `CodexEntry::SessionMeta`에 `source.subagent`, `agent_role`, `agent_nickname`를 보존하는 최소 확장을 추가한다. 그 뒤 parsed entries만 보고 `CodexSessionKind`를 판별하는 helper를 만들고, `src/main.rs`의 `collect_codex_sessions()`에서 merge 전에 `Subagent`만 early-continue로 제외한다. README에는 Claude와 별도로 Codex 기본 필터링 정책을 문서화한다.

**Tech Stack:** Rust 2024, serde_json, chrono, cargo build, cargo clippy, cargo test

---

## File Structure

| File | Change | Role |
|------|--------|------|
| `src/parser/codex.rs` | Modify | `SessionMeta` 확장, hard-signal 추출 helper, `CodexSessionKind` 분류 helper, parser/unit tests |
| `src/main.rs` | Modify | `collect_codex_sessions()`에서 Codex subagent 기본 제외, collection tests |
| `src/analyzer/prompt.rs` | Modify | 확장된 `CodexEntry::SessionMeta` 테스트 fixture 컴파일 유지 |
| `README.md` | Modify | Codex session filtering 기본 동작 문서화 |

**Behavior change:** Codex rollout JSONL 중 `session_meta`에 `source.subagent`, `agent_role`, `agent_nickname` 중 하나라도 명시된 세션은 기본적으로 수집에서 제외된다. `cwd`가 `~/.codex/worktrees/...`라는 사실만으로는 제외하지 않는다.

### Task 1: Parser hard-signal tests first

**Files:**
- Modify: `src/parser/codex.rs:484-862`

- [ ] **Step 1: `session_meta`가 explicit subagent metadata를 보존하는 테스트 추가**

`src/parser/codex.rs`의 기존 parser tests 바로 아래에 다음 테스트를 추가한다.

```rust
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
```

- [ ] **Step 2: 문자열 `source`는 hard signal로 취급하지 않는 테스트 추가**

```rust
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
```

- [ ] **Step 3: `CodexSessionKind` 분류 규칙 테스트 추가**

```rust
#[test]
fn test_classify_codex_session_kind_uses_only_explicit_metadata() {
    use chrono::TimeZone;
    let ts = chrono::Utc.with_ymd_and_hms(2026, 4, 22, 9, 0, 0).unwrap();

    let interactive = vec![CodexEntry::SessionMeta {
        timestamp: ts,
        session_id: "interactive".to_string(),
        cwd: "/Users/jinwoohan/.codex/worktrees/1234/rwd".to_string(),
        model_provider: "openai".to_string(),
        subagent_source: None,
        agent_role: None,
        agent_nickname: None,
        text: "interactive".to_string(),
    }];
    assert_eq!(
        classify_codex_session(&interactive),
        CodexSessionKind::Interactive
    );

    let by_source = vec![CodexEntry::SessionMeta {
        timestamp: ts,
        session_id: "subagent-source".to_string(),
        cwd: "/Users/jinwoohan/.codex/memories".to_string(),
        model_provider: "openai".to_string(),
        subagent_source: Some("memory_consolidation".to_string()),
        agent_role: None,
        agent_nickname: None,
        text: "subagent".to_string(),
    }];
    assert_eq!(
        classify_codex_session(&by_source),
        CodexSessionKind::Subagent
    );

    let by_role = vec![CodexEntry::SessionMeta {
        timestamp: ts,
        session_id: "subagent-role".to_string(),
        cwd: "/Users/jinwoohan/.codex/memories".to_string(),
        model_provider: "openai".to_string(),
        subagent_source: None,
        agent_role: Some("memory builder".to_string()),
        agent_nickname: None,
        text: "subagent".to_string(),
    }];
    assert_eq!(
        classify_codex_session(&by_role),
        CodexSessionKind::Subagent
    );

    let by_nickname = vec![CodexEntry::SessionMeta {
        timestamp: ts,
        session_id: "subagent-nickname".to_string(),
        cwd: "/Users/jinwoohan/.codex/memories".to_string(),
        model_provider: "openai".to_string(),
        subagent_source: None,
        agent_role: None,
        agent_nickname: Some("Morpheus".to_string()),
        text: "subagent".to_string(),
    }];
    assert_eq!(
        classify_codex_session(&by_nickname),
        CodexSessionKind::Subagent
    );
}
```

- [ ] **Step 4: 새 parser tests가 현재 코드에서 실패하는지 확인**

Run: `cargo test test_parse_session_meta_entry_preserves_subagent_metadata -- --exact`
Expected: FAIL with compile errors for missing `subagent_source`, `agent_role`, `agent_nickname`, `CodexSessionKind`, or `classify_codex_session`

### Task 2: Extend `SessionMeta` and add the classification helper

**Files:**
- Modify: `src/parser/codex.rs:35-170`
- Modify: `src/parser/codex.rs:336-482`
- Modify: `src/parser/codex.rs:653-861`
- Modify: `src/main.rs:1551-1569`
- Modify: `src/analyzer/prompt.rs:512-518`

- [ ] **Step 1: `CodexEntry::SessionMeta`와 `CodexSessionKind` 정의 확장**

`src/parser/codex.rs`의 enum definitions를 다음 형태로 업데이트한다.

```rust
#[derive(Debug)]
pub enum CodexEntry {
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
        text: String,
    },
    Other {
        timestamp: DateTime<Utc>,
        entry_type: String,
        text: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexSessionKind {
    Interactive,
    Subagent,
}
```

- [ ] **Step 2: hard-signal extraction helper와 `from_raw()` parsing 추가**

`src/parser/codex.rs`의 `extract_payload_text()` 아래에 helper를 추가하고, `session_meta` branch를 교체한다.

```rust
fn optional_non_empty_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn extract_subagent_source(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("source")
        .and_then(|value| value.as_object())
        .and_then(|source| source.get("subagent"))
        .and_then(optional_non_empty_string)
}

pub fn classify_codex_session(entries: &[CodexEntry]) -> CodexSessionKind {
    for entry in entries {
        if let CodexEntry::SessionMeta {
            subagent_source,
            agent_role,
            agent_nickname,
            ..
        } = entry
        {
            if subagent_source.is_some() || agent_role.is_some() || agent_nickname.is_some() {
                return CodexSessionKind::Subagent;
            }
        }
    }

    CodexSessionKind::Interactive
}
```

`from_raw()`의 `session_meta` branch는 다음 형태로 바꾼다.

```rust
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
```

- [ ] **Step 3: fingerprint와 수동 `SessionMeta` fixtures를 expanded shape로 맞추기**

`src/parser/codex.rs`의 `CodexEntryFingerprint::SessionMeta`와 `entry_fingerprint()`에 아래 필드를 추가한다.

```rust
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
```

그리고 `CodexEntry::SessionMeta`를 수동으로 만드는 모든 테스트 fixture를 다음 shape로 맞춘다.

```rust
CodexEntry::SessionMeta {
    timestamp: ts,
    session_id: "s1".to_string(),
    cwd: "/p".to_string(),
    model_provider: "openai".to_string(),
    subagent_source: None,
    agent_role: None,
    agent_nickname: None,
    text: "s1\n/p\nopenai".to_string(),
}
```

이 패턴을 아래 위치에 모두 적용한다.
- `src/parser/codex.rs:658-664`
- `src/parser/codex.rs:721-727`
- `src/parser/codex.rs:765-771`
- `src/parser/codex.rs:791-797`
- `src/parser/codex.rs:823-829`
- `src/main.rs:1554-1560`
- `src/analyzer/prompt.rs:512-518`

- [ ] **Step 4: parser 대상 tests만 먼저 통과시키기**

Run: `cargo test parser::codex::tests:: -- --nocapture`
Expected: new parsing/classification tests PASS and existing parser tests still PASS

- [ ] **Step 5: parser 단위 커밋**

```bash
git add src/parser/codex.rs src/main.rs src/analyzer/prompt.rs
git commit -m "feat: capture codex subagent metadata"
```

### Task 3: Add collection tests for exclusion and false-positive protection

**Files:**
- Modify: `src/main.rs:1466-1577`

- [ ] **Step 1: explicit subagent metadata가 있는 세션을 제외하는 collection test 추가**

`src/main.rs` tests module에 다음 테스트를 추가한다.

```rust
#[test]
fn test_collect_codex_sessions_excludes_explicit_subagent_sessions() {
    let date = chrono::NaiveDate::from_ymd_opt(2099, 1, 1).expect("date");
    let base = unique_temp_dir("rwd_test_codex_subagent_filter");
    let root = base.join("codex-root");
    let day_dir = root.join("2099").join("01").join("01");
    std::fs::create_dir_all(&day_dir).expect("day dir");

    let mut file = std::fs::File::create(day_dir.join("rollout-subagent.jsonl")).expect("file");
    writeln!(
        file,
        r#"{{"timestamp":"2099-01-01T12:00:00Z","type":"session_meta","payload":{{"id":"codex-subagent","cwd":"/Users/jinwoohan/.codex/memories","model_provider":"openai","source":{{"subagent":"memory_consolidation"}},"agent_role":"memory builder","agent_nickname":"Morpheus"}}}}"#
    )
    .expect("meta");
    writeln!(
        file,
        r#"{{"timestamp":"2099-01-01T12:01:00Z","type":"event_msg","payload":{{"type":"user_message","message":"summarize memories"}}}}"#
    )
    .expect("user");
    writeln!(
        file,
        r#"{{"timestamp":"2099-01-01T12:02:00Z","type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"done"}}]}}}}"#
    )
    .expect("assistant");

    let cfg = test_config(Some(vec![root.to_string_lossy().to_string()]), None);
    let (sessions, roots) = collect_codex_sessions(date, Some(&cfg));

    assert_eq!(roots, vec![root.clone()]);
    assert!(sessions.is_empty());

    std::fs::remove_dir_all(&base).ok();
}
```

- [ ] **Step 2: worktree cwd만 있는 interactive session은 유지하는 test 추가**

```rust
#[test]
fn test_collect_codex_sessions_keeps_worktree_interactive_sessions_without_hard_signal() {
    let date = chrono::NaiveDate::from_ymd_opt(2099, 1, 1).expect("date");
    let base = unique_temp_dir("rwd_test_codex_worktree_interactive");
    let root = base.join("codex-root");
    let day_dir = root.join("2099").join("01").join("01");
    std::fs::create_dir_all(&day_dir).expect("day dir");

    let mut file = std::fs::File::create(day_dir.join("rollout-interactive.jsonl")).expect("file");
    writeln!(
        file,
        r#"{{"timestamp":"2099-01-01T12:00:00Z","type":"session_meta","payload":{{"id":"codex-interactive","cwd":"/Users/jinwoohan/.codex/worktrees/1234/rwd","model_provider":"openai","source":"cli"}}}}"#
    )
    .expect("meta");
    writeln!(
        file,
        r#"{{"timestamp":"2099-01-01T12:01:00Z","type":"event_msg","payload":{{"type":"user_message","message":"continue the bugfix"}}}}"#
    )
    .expect("user");
    writeln!(
        file,
        r#"{{"timestamp":"2099-01-01T12:02:00Z","type":"response_item","payload":{{"type":"message","role":"assistant","content":[{{"type":"output_text","text":"working on it"}}]}}}}"#
    )
    .expect("assistant");

    let cfg = test_config(Some(vec![root.to_string_lossy().to_string()]), None);
    let (sessions, _) = collect_codex_sessions(date, Some(&cfg));

    assert_eq!(sessions.len(), 1);
    let (summary, _) = &sessions[0];
    assert_eq!(summary.session_id, "codex-interactive");
    assert_eq!(summary.cwd, "/Users/jinwoohan/.codex/worktrees/1234/rwd");

    std::fs::remove_dir_all(&base).ok();
}
```

- [ ] **Step 3: 새 collection tests가 현재 코드에서 실패하는지 확인**

Run: `cargo test test_collect_codex_sessions_excludes_explicit_subagent_sessions -- --exact`
Expected: FAIL because `collect_codex_sessions()` still includes explicit subagent sessions

### Task 4: Filter subagents in `collect_codex_sessions()`, document it, and verify the repo

**Files:**
- Modify: `src/main.rs:1012-1038`
- Modify: `README.md:123-130`

- [ ] **Step 1: `collect_codex_sessions()`에 hard-signal subagent early-continue 추가**

`src/main.rs`의 inner file loop를 다음으로 바꾼다.

```rust
for file in session_files {
    let Ok(entries) = parser::codex::parse_codex_jsonl_file(&file) else {
        continue;
    };
    let entries = parser::codex::filter_entries_by_local_date(entries, today);
    let summary = parser::codex::summarize_codex_entries(&entries);
    if summary.user_count == 0 && summary.assistant_count == 0 {
        continue;
    }
    if parser::codex::classify_codex_session(&entries)
        == parser::codex::CodexSessionKind::Subagent
    {
        continue;
    }

    let merge_key = if !summary.session_id.is_empty() {
        SessionMergeKey::SessionId(summary.session_id)
    } else {
        let rollout_filename = file
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| file.to_string_lossy().to_string());
        SessionMergeKey::RootAndFile {
            root: root.clone(),
            rollout_filename,
        }
    };

    merged_entries.entry(merge_key).or_default().extend(entries);
}
```

- [ ] **Step 2: README에 Codex filtering 정책 추가**

`README.md`의 Claude filtering section 바로 아래에 다음 블록을 추가한다.

```md
Codex session filtering:
- Default behavior excludes Codex sessions that contain explicit subagent metadata in `session_meta`.
- Hard signals are `source.subagent`, `agent_role`, and `agent_nickname`.
- Interactive Codex sessions are still included by default, including sessions launched from `~/.codex/worktrees/...`.
- `cwd` patterns and prompt heuristics are not used for Codex subagent filtering.
```

- [ ] **Step 3: 전체 검증 실행**

Run: `cargo build`
Expected: build succeeds

Run: `cargo clippy`
Expected: no warnings or errors

Run: `cargo test`
Expected: all tests PASS, including parser and collection tests for subagent exclusion and worktree false-positive protection

- [ ] **Step 4: 최종 커밋**

```bash
git add src/parser/codex.rs src/main.rs src/analyzer/prompt.rs README.md docs/superpowers/plans/2026-04-22-codex-subagent-session-filtering.md
git commit -m "feat: filter codex subagent sessions by metadata"
```
