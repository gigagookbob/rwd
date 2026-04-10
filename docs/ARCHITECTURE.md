# Architecture — rwd

## Core Flow

```
rwd today:       CLI entry → log file discovery → JSONL parsing & structuring → LLM provider call (API or Codex CLI) → Markdown generation → Obsidian Vault save
rwd today -b:    CLI entry → spawn background worker → exit immediately → OS notification on completion
rwd summary:     CLI entry → cache load → collect work_summaries → LLM API (SUMMARY_PROMPT) → Obsidian save + clipboard copy
rwd slack:       CLI entry → cache load → collect work_summaries → LLM API (SLACK_PROMPT) → clipboard copy
```

## Extracted Insights

- User's decision branches (why A over B)
- Things the user was curious or confused about
- Model errors corrected by the user
- Context switches between sessions (which project, which task)

## Two-Stage Processing

1. **Parsing stage (rule-based)**: Transforms log files into structured data
   - Who said what (user / assistant)
   - Tool invocations
   - Error occurrences
   - Undo/correction patterns

2. **Analysis stage (LLM-based)**: Sends structured data to LLM for insight extraction
   - Uses structured data (not raw logs) to minimize information loss

## Input Sources

### Claude Code

- Log location: `~/.claude/projects/` subdirectory JSONL files
- Format: Each line is an independent JSON object

### Codex

- Log location: `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`
- Format: Each line is `{"timestamp", "type", "payload"}` JSON object
- Entry types: session_meta, response_item, event_msg, turn_context
- Parser: Two-stage conversion (CodexRawEntry → CodexEntry)

## Project Structure

```
rwd/
├── Cargo.toml
├── CLAUDE.md
├── prompts/             # LLM prompt templates (EN/KO)
├── docs/
│   ├── ARCHITECTURE.md  # This document
│   ├── MILESTONES.md
│   └── CONVENTIONS.md
├── src/
│   ├── main.rs          # CLI entry point
│   ├── cli.rs           # clap-based CLI definitions
│   ├── messages.rs      # Centralized user-facing strings
│   ├── config.rs        # Config file management
│   ├── parser/          # Log parsing modules
│   │   ├── mod.rs
│   │   ├── claude.rs    # Claude Code log parser
│   │   └── codex.rs     # Codex log parser
│   ├── analyzer/        # Structured data → LLM insight extraction
│   │   ├── mod.rs       # Orchestrator
│   │   ├── anthropic.rs # Anthropic Claude API client
│   │   ├── codex_exec.rs# Codex CLI (codex exec) provider backend
│   │   ├── insight.rs   # Response parsing
│   │   ├── openai.rs    # OpenAI API client
│   │   ├── planner.rs   # Execution plan generation
│   │   ├── prompt.rs    # Prompt construction
│   │   ├── provider.rs  # LlmProvider enum, provider selection
│   │   └── summarizer.rs# Chunk summarization
│   ├── output/          # Markdown generation and file saving
│   │   ├── mod.rs
│   │   └── markdown.rs
│   ├── redactor/        # Sensitive data masking
│   │   ├── mod.rs       # Public API: redact_text(), RedactResult
│   │   └── patterns.rs  # Built-in patterns (LazyLock initialized)
│   ├── cache.rs         # Analysis result caching
│   └── update.rs        # Self-update via GitHub Releases
└── tests/
```

## Core Dependencies

| Crate        | Purpose              |
| ------------ | -------------------- |
| `clap`       | CLI parsing          |
| `regex`      | Regex matching       |
| `serde`      | Serialization        |
| `serde_json` | JSON/JSONL parsing   |
| `reqwest`    | HTTP client          |
| `tokio`      | Async runtime        |
| `chrono`     | Date/time handling   |
| `dialoguer`  | Interactive prompts  |
| `notify-rust`| OS notifications     |
