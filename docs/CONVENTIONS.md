# Coding Conventions — rwd

## Naming

| Target           | Convention              | Example                 |
| ---------------- | ----------------------- | ----------------------- |
| Variables/funcs  | `snake_case`            | `parse_log_file`        |
| Structs/enums    | `PascalCase`            | `SessionLog`, `LogType` |
| Constants        | `SCREAMING_SNAKE_CASE`  | `MAX_RETRY_COUNT`       |
| Modules/files    | `snake_case`            | `log_parser.rs`         |

## Formatting & Linting

- `cargo fmt` — auto-format all code
- `cargo clippy` — maintain 0 warnings

## Error Handling

- No `unwrap()` or `expect()` in production code (tests are OK)
- Use `Result<T, E>` with `?` operator as default
- Error messages should be user-friendly

```
Bad:  Error: parse failed
Good: Error: Failed to parse session log at ~/.claude/projects/xxx.jsonl — invalid JSON at line 42
```

## Comments

- Write comments that explain "why", not "what"
- Let the code express what it does
- Keep comments concise

## Testing

- Include unit tests in each module
- Test function names follow `test_behavior_condition_expected` pattern

```rust
#[test]
fn test_parse_valid_jsonl_returns_session_entries() { ... }
```
