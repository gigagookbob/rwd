// parser 모듈은 로그 파일을 읽고 구조화된 데이터로 변환하는 역할을 합니다.
// 디렉토리 안에 mod.rs 파일을 두면 디렉토리 이름이 모듈 이름이 됩니다 (Rust Book Ch.7 참조).
// 예: src/parser/mod.rs → mod parser; 로 선언 가능

// claude 서브모듈을 공개(pub) 선언합니다.
// 이렇게 하면 parser::claude::LogEntry 형태로 접근할 수 있습니다.
pub mod claude;

// codex 서브모듈: OpenAI Codex CLI 세션 로그 파싱
// claude와 달리 pub use로 재공개하지 않음 — Codex 타입은 parser::codex:: 네임스페이스로 접근합니다.
// 두 파서의 타입 이름이 충돌하지 않도록 네임스페이스를 분리합니다.
pub mod codex;

// Box<dyn std::error::Error>는 "어떤 에러든 담을 수 있는 박스"입니다.
// dyn은 동적 디스패치를 의미합니다 — 런타임에 실제 에러 타입이 결정됩니다 (Rust Book Ch.17 참조).
// M5에서 thiserror 크레이트로 전용 에러 타입을 만들 예정이므로, 지금은 이 간단한 별칭을 사용합니다.
pub type ParseError = Box<dyn std::error::Error>;

// pub use로 자주 사용하는 항목을 상위 모듈에서 바로 접근할 수 있게 합니다 (Rust Book Ch.7.4 참조).
// 예: parser::LogEntry 로 바로 접근 가능 (parser::claude::LogEntry 대신)
pub use claude::{
    discover_log_dir, filter_entries_by_date, list_project_dirs, list_session_files,
    parse_jsonl_file, summarize_entries,
};
