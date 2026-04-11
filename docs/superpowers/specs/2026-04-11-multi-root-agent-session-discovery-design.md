# Multi-Root Agent Session Discovery

## Problem

현재 `rwd`는 Claude Code와 Codex 로그를 각각 "하나의 루트 경로"에서만 찾는다.

- Claude Code: `~/.claude/projects`가 존재하면 그 경로만 사용하고, 없을 때만 WSL의 `/mnt/c/Users/.../.claude/projects` fallback을 본다.
- Codex: `~/.codex/sessions`가 존재하면 그 경로만 사용하고, 없을 때만 WSL의 `/mnt/c/Users/.../.codex/sessions` fallback을 본다.

이 구조는 하이브리드 환경에서 실제 작업 일부를 누락시킨다.

### 실제로 문제가 되는 시나리오

1. WSL CLI로 작업한 Codex 세션은 `/home/jinwoo/.codex/sessions`에 쌓임
2. Codex Desktop App 세션은 `/mnt/c/Users/qew85/.codex/sessions`에 쌓임
3. 두 경로가 모두 존재하면 현재 구현은 `/home/...`만 읽고 `/mnt/...`는 무시함
4. 결과적으로 `rwd today`가 오늘의 실제 작업 일부를 놓침

Claude Code도 같은 문제가 생길 수 있다.

1. WSL 안에서 Claude CLI를 사용하면 `/home/jinwoo/.claude/projects`
2. Windows 쪽 Claude 환경을 WSL에서 읽으면 `/mnt/c/Users/.../.claude/projects`
3. 두 경로가 동시에 존재할 수 있음

## Goal

Claude Code와 Codex 모두 "단일 루트 선택"이 아니라 "존재하는 모든 유효 루트를 수집"하도록 바꾼다.

핵심 목표는 다음 두 가지다.

- 하이브리드 환경에서도 로그 누락 없이 분석하기
- 같은 세션/같은 엔트리가 여러 루트에 중복으로 존재해도 중복 분석하지 않기

## Non-Goals

- Codex Desktop App의 `CODEX_HOME` 자체를 옮기거나 자동 마이그레이션하지 않는다
- 사용자 파일을 `/mnt`와 `/home` 사이에서 자동 복사/동기화하지 않는다
- 이번 변경에서 경로 정책을 강제하지 않는다

`rwd`의 역할은 "현재 존재하는 로그를 최대한 정확하게 발견하고 합쳐서 분석"하는 데 한정한다.

## Proposed Solution

### 1. Single-root API를 Multi-root API로 교체

현재의:

- `parser::discover_log_dir() -> Result<PathBuf, _>`
- `parser::codex::discover_codex_sessions_dir() -> Result<PathBuf, _>`

를 다음 개념으로 바꾼다.

- `discover_claude_log_roots(...) -> Vec<PathBuf>`
- `discover_codex_session_roots(...) -> Vec<PathBuf>`

반환값은 "우선순위가 반영된, dedupe된 루트 목록"이다.

### 2. Root resolution order

각 provider의 루트는 아래 순서로 수집한다.

1. 명시적 config override
2. provider-specific env-derived root
3. native home root
4. WSL에서 보이는 Windows home roots

Codex 예시:

1. `config.input.codex_roots`
2. `CODEX_HOME/sessions` if `CODEX_HOME` is set
3. `~/.codex/sessions`
4. `/mnt/c/Users/*/.codex/sessions` candidates in WSL

Claude 예시:

1. `config.input.claude_roots`
2. `~/.claude/projects`
3. `/mnt/c/Users/*/.claude/projects` candidates in WSL

### 3. Config shape

새 설정 섹션을 추가한다.

```toml
[input]
codex_roots = [
  "/home/jinwoo/.codex/sessions",
  "/mnt/c/Users/qew85/.codex/sessions",
]
claude_roots = [
  "/home/jinwoo/.claude/projects",
  "/mnt/c/Users/qew85/.claude/projects",
]
```

설정이 없으면 자동 탐색을 사용한다.
설정이 있으면 그 경로를 최우선으로 사용하되, 존재하는 경로만 채택한다.

### 4. Merge strategy

루트를 여러 개 읽더라도 "같은 데이터를 두 번 분석"하면 안 된다.

#### Codex merge key

- 1차 key: `session_id`
- fallback key: `(root_path, rollout_filename)`

같은 `session_id`를 가진 세션이 여러 루트에서 발견되면 엔트리를 합친다.

엔트리 dedupe는 fingerprint 기반으로 한다.

- `SessionMeta`: `(timestamp, session_id, cwd, model_provider)`
- `UserMessage`: `(timestamp, text)`
- `AssistantMessage`: `(timestamp, text)`
- `FunctionCall`: `(timestamp, name)`
- `Other`: skip or keep only once with coarse fingerprint

합쳐진 엔트리는 timestamp 기준으로 정렬한 뒤 기존 분석 파이프라인으로 넘긴다.

#### Claude merge key

Claude는 entry-level dedupe가 더 안전하다.

- `User`: `(session_id, uuid)`
- `Assistant`: `(session_id, uuid)`
- `Progress`: `(session_id, timestamp)`
- `System`: `(session_id_or_empty, timestamp)`
- `FileHistorySnapshot`: `(message_id_or_empty)`

여러 project root에서 읽은 오늘 엔트리를 하나의 벡터로 합친 뒤 fingerprint로 dedupe한다.
이후 기존 `summarize_entries()`와 분석 파이프라인을 그대로 사용한다.

### 5. Shared discovery helper

Claude와 Codex의 WSL/Windows 경로 탐색 로직은 거의 동일하다.
중복 코드를 줄이기 위해 공용 helper를 도입한다.

예상 위치:

- 새 모듈 `src/parser/roots.rs`

역할:

- `is_wsl_environment()`
- `wsl_windows_home_candidates()`
- `dedupe_existing_paths(paths: Vec<PathBuf>) -> Vec<PathBuf>`

이 helper를 Claude/Codex parser 양쪽에서 재사용한다.

## Architecture Impact

### Affected files

- `src/config.rs`
- `src/parser/mod.rs`
- `src/parser/claude.rs`
- `src/parser/codex.rs`
- `src/main.rs`
- 가능하면 새 파일 `src/parser/roots.rs`

### Core flow changes

Before:

```text
discover one root -> read files from one root -> analyze
```

After:

```text
discover multiple roots -> read files from all roots -> dedupe/merge -> analyze
```

## Design Decisions

### Why not branch by "app vs CLI"?

그 기준은 겉으로는 직관적이지만 실제 저장 위치를 보장하지 않는다.

- Codex Desktop App도 WSL 백엔드를 사용할 수 있다
- CLI도 `CODEX_HOME`에 따라 `/mnt`를 쓸 수 있다

따라서 제품 코드는 "사용자 종류"가 아니라 "실제 로그 루트"를 기준으로 동작해야 한다.

### Why config + env + fallback order?

- Config가 가장 명시적이라 재현 가능성이 높다
- `CODEX_HOME`은 Codex에서 사실상 공식 홈 지시자 역할을 한다
- 홈 경로 fallback은 설정이 없는 사용자를 위한 안전망이다

### Why merge instead of choosing one root?

이 문제의 본질은 "어느 경로가 진실 원본인가"보다 "오늘 실제 작업이 여러 루트에 분산될 수 있다"는 점이다.

루트 하나만 선택하면 반드시 누락 케이스가 생긴다.

## Test Plan

### Root discovery

- unit test: config root와 auto-discovered root가 함께 존재할 때 우선순위가 유지되는지
- unit test: `/home`와 `/mnt`가 모두 존재하면 둘 다 반환되는지
- unit test: 중복 경로가 dedupe되는지

### Codex merge

- unit test: `/home`와 `/mnt`에서 서로 다른 session_id 두 개가 들어오면 둘 다 수집되는지
- unit test: 같은 session_id가 두 루트에 중복 존재하면 하나의 세션으로 merge되는지
- unit test: merged entries가 timestamp 순서로 정렬되는지

### Claude merge

- unit test: 두 root에서 같은 `(session_id, uuid)` 엔트리가 들어오면 한 번만 남는지
- unit test: 서로 다른 root의 서로 다른 session_id 엔트리는 모두 보존되는지

### Integration

- `rwd today`가 `/home` only, `/mnt` only, `/home + /mnt` 세 경우 모두 정상 동작하는지
- `cargo build`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

## Migration Notes

- 기존 config 파일에는 `[input]` 섹션이 없으므로 `Option<InputConfig>`로 추가한다
- 섹션이 없으면 기존 사용자도 자동 탐색으로 계속 동작해야 한다
- cache key 구조 변경은 이번 설계 범위에 포함하지 않는다

## Recommended First Implementation Slice

한 번에 전부 바꾸기보다 아래 순서가 안전하다.

1. `Config`에 optional `[input]` 섹션 추가
2. 공용 root discovery helper 추가
3. Codex multi-root 수집 + dedupe/merge 구현
4. Claude multi-root 수집 + dedupe 구현
5. verbose 출력에 실제 사용한 루트 목록 추가

