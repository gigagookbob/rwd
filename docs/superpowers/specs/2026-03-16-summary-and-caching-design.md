# rwd summary + today Caching Design

## Goals

1. Add caching to `rwd today` — skip LLM calls when the entry count hasn't changed
2. `rwd summary` subcommand — generate short progress summaries for development updates (terminal + Markdown + clipboard)

## 1. today Caching

### Cache File

`~/.rwd/cache/today-{YYYY-MM-DD}.json`:

```json
{
  "date": "2026-03-16",
  "claude_entry_count": 680,
  "codex_session_count": 0,
  "analysis": { ... }
}
```

### Logic

1. After collecting entries, check the cache file
2. If the entry count is the same, use the cached analysis and skip the LLM call
3. If the entry count differs, re-analyze and update the cache

## 2. rwd summary

### Flow

1. Check today's cache — if missing, run today first
2. Make a separate LLM call (summary-specific prompt) based on the cached analysis results
3. Terminal output + append a `## Development Progress` section to the Daily Markdown + copy to clipboard

### Prompt

- Bullet list per project, each item as a free-form sentence
- Understandable by both developers and non-developers
- Minimize technical jargon, focus on "what was accomplished"

### Output Example

```
## Development Progress

• doridori-app: Resolved all Kakao/Naver/Google social login Android errors and completed testing on the staging environment.
• doridori-app: Completed chatbot page UI and API design, then started Data Layer implementation.
• rwd: Added a Codex session parser to extend log analysis beyond Claude Code to include Codex logs.
```

### Clipboard

macOS: `pbcopy`, Linux: `xclip`

## Impact Scope

| File | Change |
|------|--------|
| `src/cli.rs` | Add `Summary` subcommand |
| `src/main.rs` | `run_summary()` + `run_today()` caching logic |
| `src/analyzer/mod.rs` | Add `analyze_summary()` function |
| `src/analyzer/provider.rs` | Add summary-specific system prompt |
| `src/output/markdown.rs` | Render development progress section |
| New: `src/cache.rs` | Cache read/write |
