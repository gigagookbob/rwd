// Reads log files and transforms them into structured data.

pub mod claude;
pub mod roots;

// Codex submodule: OpenAI Codex CLI session log parsing.
// Types are accessed via parser::codex:: namespace to avoid name conflicts with Claude types.
pub mod codex;

// TODO: Replace with a dedicated error type via thiserror.
pub type ParseError = Box<dyn std::error::Error>;

pub use claude::{
    dedupe_entries as dedupe_claude_entries, discover_claude_log_roots, filter_entries_by_date,
    list_project_dirs_in_root, list_session_files, parse_jsonl_file, summarize_entries,
};
