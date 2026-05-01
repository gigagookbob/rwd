# Codex Subagent Session Filtering

## Problem

현재 `rwd`의 Codex 수집 로직은 날짜에 해당하는 rollout JSONL 파일을 모두 읽고, 오늘 대화가 있으면 세션으로 포함한다.

이 구조에는 "Codex 서브에이전트 세션"을 제외하는 단계가 없다.

- `src/main.rs`의 `collect_codex_sessions()`는 날짜 기준으로 rollout 파일을 전부 수집한다.
- `src/parser/codex.rs`의 `SessionMeta` 파싱은 `id`, `cwd`, `model_provider`만 구조화하고 나머지 메타데이터는 버린다.

결과적으로 서브에이전트가 별도 세션 파일로 저장되더라도, 현재 구현은 그것을 일반 사용자 세션과 구분하지 못한다.

## Validated Observations

이번 설계는 추정이 아니라 실제 로컬 Codex 세션 파일을 확인한 뒤 정리한 사실에 기반한다.

### 1. Worktree 경로만으로는 서브에이전트를 판별할 수 없다

`cwd`가 `~/.codex/worktrees/...`인 세션 2개를 직접 확인했을 때, 두 세션 모두 첫 실제 사용자 메시지가 정상적인 인터랙티브 요청이었다.

즉 다음 규칙은 잘못된 규칙이다.

- `cwd`가 `~/.codex/worktrees/...`이면 서브에이전트로 본다

이번 설계에서는 이 규칙을 사용하지 않는다.

### 2. 실제 서브에이전트 세션에는 hard signal이 존재한다

전체 `session_meta` 스키마를 확인했을 때 대부분의 세션은 일반 필드만 갖고 있었지만, 일부 세션에는 다음 필드가 추가로 존재했다.

- `agent_role`
- `agent_nickname`
- `source`가 문자열이 아니라 객체이며, 예: `{"subagent": "memory_consolidation"}`

실제로 확인된 예시는 다음과 같았다.

- `cwd = /Users/jinwoohan/.codex/memories`
- `agent_role = "memory builder"`
- `agent_nickname = "Morpheus"`
- `source = {"subagent": "memory_consolidation"}`

이 조합은 일반 인터랙티브 세션보다 훨씬 강한 서브에이전트 신호다.

## Goal

Codex 세션 파일 안에 이미 존재하는 hard signal만 사용해서 서브에이전트 세션을 기본 제외한다.

핵심 목표는 다음과 같다.

- 일반 인터랙티브 Codex 세션은 계속 포함하기
- worktree 기반 일반 세션을 잘못 제외하지 않기
- 세션 파일에서 명시적으로 확인되는 서브에이전트만 제외하기

## Non-Goals

- `cwd` 패턴만으로 서브에이전트를 추정하지 않는다
- 첫 프롬프트 문구, 말투, 작업 스타일 같은 휴리스틱을 쓰지 않는다
- Claude의 자동화 세션 필터와 동일한 CLI/config override까지 이번 1차 설계 범위에 포함하지 않는다
- 부모 세션과 자식 세션을 연결하는 계보 그래프를 만들지 않는다

## Proposed Solution

### 1. SessionMeta에 필요한 메타데이터를 보존한다

현재 `CodexEntry::SessionMeta`는 서브에이전트 판별에 필요한 메타데이터를 잃어버린다.

이를 최소 범위로 확장한다.

```rust
SessionMeta {
    timestamp: DateTime<Utc>,
    session_id: String,
    cwd: String,
    model_provider: String,
    subagent_source: Option<String>,
    agent_role: Option<String>,
    agent_nickname: Option<String>,
    text: String,
}
```

파싱 규칙은 다음과 같다.

- `source`가 문자열이면 기존처럼 일반 source로 간주하고 서브에이전트 판정에 사용하지 않는다
- `source`가 객체이고 `subagent` 문자열을 가지면 `subagent_source = Some(...)`
- `agent_role`와 `agent_nickname`는 문자열일 때만 채택한다

이 구조는 가장 작은 변경으로 현재 문제를 해결한다.

### 2. Codex 세션 분류를 추가한다

세션 단위로 다음 분류를 도입한다.

```rust
enum CodexSessionKind {
    Interactive,
    Subagent,
}
```

분류 규칙은 hard signal만 사용한다.

- `subagent_source.is_some()` 이면 `Subagent`
- 아니면 `agent_role.is_some()` 이면 `Subagent`
- 아니면 `agent_nickname.is_some()` 이면 `Subagent`
- 그 외는 `Interactive`

이 순서의 의미는 "확실한 메타데이터가 있을 때만 제외"다.

### 3. 수집 단계에서 Subagent를 기본 제외한다

`src/main.rs`의 `collect_codex_sessions()`에서:

1. 파일을 읽는다
2. 날짜 필터를 적용한다
3. 요약을 만든다
4. `SessionMeta`를 기반으로 세션 종류를 판별한다
5. `Subagent`이면 제외한다
6. `Interactive`만 기존 merge/dedupe 흐름으로 넘긴다

이때 merge key, dedupe, timestamp 정렬 로직은 그대로 유지한다.

즉 이번 변경의 핵심은 "수집 후보를 줄이는 필터 1개 추가"이지, 수집 파이프라인 전체 재설계가 아니다.

## Design Decisions

### Why not filter by worktree cwd?

실제 확인 결과 worktree cwd를 사용하는 일반 인터랙티브 세션이 존재했다.

따라서 worktree 기준 필터는 오탐을 만든다.

### Why not use prompt heuristics?

이번 문제는 이미 세션 파일 안에 메타데이터가 존재하는 경우가 확인되었다.

이럴 때는 말투나 패턴 추정보다 저장된 메타데이터를 우선 사용해야 한다.

### Why not add a general metadata map first?

가능은 하지만 지금 문제를 해결하는 데 필요 이상으로 범위를 키운다.

이번 변경은 Rust 학습자 친화성과 유지보수성을 위해 "필요한 필드만 추가 파싱"하는 쪽을 선택한다.

## Architecture Impact

영향 범위는 작다.

- `src/parser/codex.rs`
  - `SessionMeta` 필드 확장
  - `session_meta` 파싱 확장
  - 세션 종류 판별 helper 추가
- `src/main.rs`
  - `collect_codex_sessions()`에서 subagent 제외 로직 추가
- `README.md`
  - Codex subagent 기본 제외 동작 문서화

## Error Handling

- `source`가 문자열이 아니고 객체도 아니면 무시한다
- `source.subagent`가 문자열이 아니면 무시한다
- `agent_role`, `agent_nickname`가 비문자열이면 무시한다
- 메타데이터가 일부만 있어도 hard signal이 하나라도 있으면 `Subagent`로 본다

이 규칙은 로그 포맷 변화에 대해 안전한 기본값을 제공한다.

## Test Plan

### Parser

- unit test: `source`가 문자열일 때 `subagent_source`가 비어 있는지
- unit test: `source = {"subagent":"memory_consolidation"}`일 때 `subagent_source`가 채워지는지
- unit test: `agent_role`, `agent_nickname`가 있으면 `SessionMeta`에 보존되는지

### Classification

- unit test: `subagent_source`가 있으면 `Subagent`
- unit test: `agent_role`만 있어도 `Subagent`
- unit test: `agent_nickname`만 있어도 `Subagent`
- unit test: 세 필드가 모두 없으면 `Interactive`

### Collection

- unit test: 일반 인터랙티브 Codex 세션은 계속 수집되는지
- unit test: `source.subagent`가 있는 세션은 제외되는지
- unit test: `cwd`가 `~/.codex/worktrees/...`라도 hard signal이 없으면 포함되는지

### Verification

- `cargo build`
- `cargo clippy`
- `cargo test`

## Migration Notes

- 기존 캐시 포맷 변경은 필요 없다
- 기존 사용자 설정 변경은 필요 없다
- 동작 변화는 "일부 Codex subagent 세션이 기본 제외됨" 한 가지다

## Recommended First Implementation Slice

1. `SessionMeta`에 `subagent_source`, `agent_role`, `agent_nickname` 추가
2. `session_meta` 파싱 확장
3. 세션 종류 판별 helper 추가
4. `collect_codex_sessions()`에서 `Subagent` 기본 제외
5. 테스트 추가
6. README에 동작 문서화
