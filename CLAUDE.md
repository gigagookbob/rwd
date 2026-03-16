# AGENTS.md — rwd (rewind)

AI Agent 세션 로그를 분석하여 일일 개발 인사이트를 추출하고, Obsidian vault에 Markdown으로 저장하는 Rust CLI 도구.

## Technical Constraints

- Language: Rust (2024 Edition), Stable 1.94.0+
- 공식 문서 기준: https://doc.rust-lang.org/book/ (2024 Edition)
- 아키텍처 및 프로젝트 구조: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- 개발 마일스톤: [docs/MILESTONES.md](docs/MILESTONES.md)

## Learning Context

이 프로젝트의 개발자는 Rust 학습자이다. 코드 작성과 동시에 Rust를 배우는 것이 프로젝트의 목적 중 하나이다.
학습 관련 상세 지침: [docs/LEARNING_GUIDE.md](docs/LEARNING_GUIDE.md)

## MUST DO

- 코드 작성 후 반드시 `cargo build`, `cargo clippy`, `cargo test`로 검증할 것
- 새로운 Rust 개념 사용 시, 해당 개념이 무엇이고 왜 필요한지 설명할 것
- 에러 처리는 `unwrap()` 대신 `Result`와 `?` 연산자를 사용할 것
- 설계 결정 시 대안과 선택 이유를 함께 설명할 것
- Context7 MCP를 사용하여 크레이트 API를 참조한 뒤 코드를 작성할 것
- 함수/모듈 단위로 작은 단계씩 구현할 것
- 코딩 컨벤션 준수: [docs/CONVENTIONS.md](docs/CONVENTIONS.md)

## MUST NOT DO

- 설명 없이 고급 패턴을 사용하지 말 것 (복잡한 트레이트 바운드, 매크로 정의, 라이프타임 어노테이션 등)
- `unsafe` 블록을 사용하지 말 것
- 한 번에 100줄 이상의 코드를 생성하지 말 것
- deprecated된 API나 이전 에디션(2021 이하)의 패턴을 사용하지 말 것
- 공식 문서에서 확인되지 않은 내용을 사실처럼 설명하지 말 것
- 학습자가 이해하지 못한 채 넘어가도록 방치하지 말 것
