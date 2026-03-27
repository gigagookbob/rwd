# Codex Resume Session Date Filtering

## Problem

Codex의 resume 기능으로 이전 날짜의 세션을 이어서 사용하면, 새 대화 내용이 기존 날짜의 rollout JSONL 파일에 append된다. rwd의 `collect_codex_sessions`는 첫 엔트리의 날짜만 확인하므로, resume된 세션의 오늘 대화가 누락된다.

### 재현 시나리오

1. 3/26에 Codex 세션 시작 -> `sessions/2026/03/26/rollout-...jsonl` 생성
2. 3/27에 해당 세션 resume -> 같은 파일에 새 엔트리 append
3. `rwd today` 실행 -> 첫 엔트리가 3/26이므로 세션 전체 스킵

## Solution

`collect_codex_sessions` (main.rs)에서 날짜 필터링 방식을 변경한다.

### Before

```rust
let session_date = entries.iter().find_map(parser::codex::entry_local_date);
if session_date != Some(today) {
    continue;
}
```

첫 엔트리의 날짜가 오늘이 아니면 세션 전체를 스킵한다.

### After

1. SessionMeta 엔트리는 날짜와 무관하게 항상 보존 (세션 메타데이터 용도)
2. UserMessage, AssistantMessage, FunctionCall은 오늘 날짜인 것만 필터링
3. 필터링 후 대화가 없으면 스킵

### SessionMeta를 항상 보존하는 이유

- `summarize_codex_entries`가 SessionMeta에서 session_id, cwd, model_provider 추출
- `build_codex_prompt`는 SessionMeta를 이미 무시함 (match의 `_` arm)
- SessionMeta는 대화 내용이 아닌 메타데이터

## Scope

- 변경 파일: `src/main.rs` (`collect_codex_sessions` 함수)
- parser, analyzer 변경 없음
- 배너 표시, 캐시 stale 감지 등 하류 로직은 필터링된 entries 기준으로 자동 정상 동작

## Test Plan

- 단위 테스트: 다중 날짜 엔트리가 섞인 entries에서 오늘 날짜만 필터링되는지 확인
- 단위 테스트: SessionMeta가 어제 날짜여도 보존되는지 확인
- 단위 테스트: 오늘 엔트리가 없으면 세션이 제외되는지 확인
