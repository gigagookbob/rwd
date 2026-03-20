// Reads log files and transforms them into structured data.

pub mod claude;

// Codex submodule: OpenAI Codex CLI session log parsing.
// Types are accessed via parser::codex:: namespace to avoid name conflicts with Claude types.
pub mod codex;

// TODO: Replace with a dedicated error type via thiserror.
pub type ParseError = Box<dyn std::error::Error>;

pub use claude::{
    discover_log_dir, filter_entries_by_date, list_project_dirs, list_session_files,
    parse_jsonl_file, summarize_entries,
};
