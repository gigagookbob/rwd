# AGENTS.md — rwd (rewind)

CLI tool that analyzes AI coding session logs and extracts daily development insights, saving them as Markdown to an Obsidian vault.

## Technical Constraints

- Language: Rust (2024 Edition), Stable 1.94.0+
- Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Milestones: [docs/MILESTONES.md](docs/MILESTONES.md)
- Conventions: [docs/CONVENTIONS.md](docs/CONVENTIONS.md)

## MUST DO

- Cross-platform: 파일 I/O, 프로세스, 터미널 입력, config 경로 변경 시 Windows/macOS/Linux 동작 차이 확인
- Validate with `cargo build`, `cargo clippy`, `cargo test` after changes
- Use `Result` and `?` operator for error handling (no `unwrap()` in non-test code)
- Use Context7 MCP to reference crate APIs before writing code
- Explain design decisions with alternatives and rationale
- Implement in small increments (function/module level)

## MUST NOT DO

- No `unsafe` blocks
- No deprecated APIs or pre-2024 edition patterns
- Do not present unverified information as fact
