# Coding Conventions — rwd

## 네이밍

| 대상           | 규칙                    | 예시                    |
| -------------- | ----------------------- | ----------------------- |
| 변수/함수      | `snake_case`            | `parse_log_file`        |
| 구조체/열거형  | `PascalCase`            | `SessionLog`, `LogType` |
| 상수           | `SCREAMING_SNAKE_CASE`  | `MAX_RETRY_COUNT`       |
| 모듈/파일      | `snake_case`            | `log_parser.rs`         |

## 포매팅 및 린트

- `cargo fmt` — 모든 코드에 자동 포매팅 적용
- `cargo clippy` — warning 0개 유지

## 에러 처리

- `unwrap()`, `expect()` 사용 금지 (테스트 코드 제외)
- `Result<T, E>`와 `?` 연산자를 기본으로 사용
- 에러 메시지는 사용자 친화적으로 작성

```
Bad:  Error: parse failed
Good: Error: Failed to parse session log at ~/.claude/projects/xxx.jsonl — invalid JSON at line 42
```

## 주석

- "왜(why)"를 설명하는 주석을 작성할 것
- "무엇(what)"은 코드 자체가 표현하도록 할 것
- 새로운 Rust 개념이 사용된 곳에는 학습 참조 주석을 남길 것

```rust
// Rust의 ownership 규칙에 의해 여기서 값이 이동(move)됩니다 (Rust Book Ch.4)
let parsed = parse_session(raw_data);
```

## 테스트

- 각 모듈에 기본 단위 테스트를 포함할 것
- 테스트 함수명은 `test_동작_조건_기대결과` 형식

```rust
#[test]
fn test_parse_valid_jsonl_returns_session_entries() { ... }
```
