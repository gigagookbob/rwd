# Codex Resume Session Date Filtering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Codex resume 세션에서 오늘 날짜의 엔트리만 필터링하여 분석에 포함시킨다.

**Architecture:** `parser::codex`에 `filter_entries_by_local_date` 함수를 추가하고, `main.rs`의 `collect_codex_sessions`에서 기존 첫 엔트리 날짜 체크를 이 필터로 교체한다. SessionMeta는 날짜와 무관하게 항상 보존한다.

**Tech Stack:** Rust, chrono

---

### Task 1: filter_entries_by_local_date 테스트 작성

**Files:**
- Modify: `src/parser/codex.rs:258-431` (tests 모듈)

- [ ] **Step 1: 다중 날짜 엔트리에서 오늘 것만 필터링하는 테스트 작성**

```rust
#[test]
fn test_filter_entries_by_local_date_keeps_today_only() {
    use chrono::TimeZone;
    // Use UTC dates that map to the same local date regardless of timezone.
    // "yesterday" = 2026-03-15T12:00:00Z, "today" = 2026-03-16T12:00:00Z
    let yesterday_ts = chrono::Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap();
    let today_ts = chrono::Utc.with_ymd_and_hms(2026, 3, 16, 12, 0, 0).unwrap();
    let today_local = today_ts.with_timezone(&chrono::Local).date_naive();

    let entries = vec![
        CodexEntry::SessionMeta {
            timestamp: yesterday_ts,
            session_id: "s1".to_string(),
            cwd: "/p".to_string(),
            model_provider: "openai".to_string(),
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

    // SessionMeta preserved + 2 today entries = 3
    assert_eq!(filtered.len(), 3);
    assert!(matches!(filtered[0], CodexEntry::SessionMeta { .. }));
    assert!(matches!(filtered[1], CodexEntry::UserMessage { .. }));
    if let CodexEntry::UserMessage { text, .. } = &filtered[1] {
        assert_eq!(text, "new msg");
    }
    assert!(matches!(filtered[2], CodexEntry::AssistantMessage { .. }));
}
```

- [ ] **Step 2: 오늘 엔트리가 없으면 SessionMeta만 남는 테스트 작성**

```rust
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
        },
        CodexEntry::UserMessage {
            timestamp: yesterday_ts,
            text: "old msg".to_string(),
        },
    ];

    let filtered = filter_entries_by_local_date(entries, today_local);

    // Only SessionMeta remains
    assert_eq!(filtered.len(), 1);
    assert!(matches!(filtered[0], CodexEntry::SessionMeta { .. }));
}
```

- [ ] **Step 3: 같은 날짜 세션은 모든 엔트리가 보존되는 테스트 작성**

```rust
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
        },
    ];

    let filtered = filter_entries_by_local_date(entries, today_local);
    assert_eq!(filtered.len(), 4);
}
```

- [ ] **Step 4: 테스트가 컴파일 실패하는지 확인**

Run: `cargo test --lib parser::codex::tests::test_filter_entries_by_local_date_keeps_today_only 2>&1 | tail -5`
Expected: 컴파일 에러 — `filter_entries_by_local_date` 함수가 존재하지 않음

---

### Task 2: filter_entries_by_local_date 구현

**Files:**
- Modify: `src/parser/codex.rs:167-178` (entry_local_date 뒤에 추가)

- [ ] **Step 1: filter_entries_by_local_date 함수 구현**

`entry_local_date` 함수(line 178) 바로 뒤에 추가:

```rust
/// Filters Codex entries to only those belonging to the given local date.
/// SessionMeta entries are always preserved regardless of date, because
/// they carry session metadata needed by `summarize_codex_entries`.
pub fn filter_entries_by_local_date(
    entries: Vec<CodexEntry>,
    date: NaiveDate,
) -> Vec<CodexEntry> {
    entries
        .into_iter()
        .filter(|entry| {
            matches!(entry, CodexEntry::SessionMeta { .. })
                || entry_local_date(entry) == Some(date)
        })
        .collect()
}
```

- [ ] **Step 2: 테스트 실행**

Run: `cargo test --lib parser::codex::tests::test_filter_entries_by_local_date -v`
Expected: 3개 테스트 모두 PASS

---

### Task 3: collect_codex_sessions에서 필터 적용

**Files:**
- Modify: `src/main.rs:683-696`

- [ ] **Step 1: 기존 첫 엔트리 날짜 체크를 필터로 교체**

`src/main.rs:683-696`의 for 루프 본문을 다음으로 교체:

```rust
    let mut sessions = Vec::new();
    for file in session_files {
        if let Ok(entries) = parser::codex::parse_codex_jsonl_file(&file) {
            let entries = parser::codex::filter_entries_by_local_date(entries, today);
            let summary = parser::codex::summarize_codex_entries(&entries);
            // Only include sessions with actual conversation from today
            if summary.user_count > 0 || summary.assistant_count > 0 {
                sessions.push((summary, entries));
            }
        }
    }
    sessions
```

- [ ] **Step 2: cargo clippy 실행**

Run: `cargo clippy 2>&1 | tail -10`
Expected: 경고/에러 없음

- [ ] **Step 3: 전체 테스트 실행**

Run: `cargo test 2>&1 | tail -15`
Expected: 모든 테스트 PASS

- [ ] **Step 4: 커밋**

```bash
git add src/parser/codex.rs src/main.rs
git commit -m "fix: filter Codex resume session entries by date

When a Codex session is resumed on a different day, new entries are
appended to the original date's rollout file. Previously, rwd checked
only the first entry's date and skipped the entire session. Now it
filters entries to include only today's conversations while preserving
SessionMeta for session metadata extraction."
```
