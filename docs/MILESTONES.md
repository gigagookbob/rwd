# Development Milestones — rwd

Each milestone is independently buildable and testable.

## M1: CLI Skeleton

- Define basic command structure with clap
- Verify `rwd today`, `rwd --help` work

## M2: Log File Discovery & Parsing

- Discover Claude Code log files (JSONL)
- Deserialize with serde
- Error handling for invalid log lines

## M3: LLM API Integration

- Send structured data to Claude API
- Receive and parse insight responses
- API key management (env vars or config file)

## M4: Markdown Generation & Saving

- Convert insights to template-based Markdown
- Save date-based files to Obsidian vault path

## M5: Polish & Extensions

- ~~Sensitive data masking~~ (v0.5.0 — `redactor` module, 8 built-in patterns, config toggle)
- ~~`rwd summary` progress summaries~~ (v0.8.0 — caching, Markdown summary, clipboard copy)
- ~~`rwd slack` share message~~ (v0.9.0 — non-developer friendly Slack message, cache freshness notice)
- i18n English-first conversion (v0.10.0 — externalized prompts, `--lang` flag, EN/KO support)
