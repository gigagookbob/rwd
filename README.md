# rwd (rewind)

CLI that turns your AI agent sessions into a daily journal. Extracts decisions, learnings, and corrections, then saves them as Obsidian Daily Notes.

## rwd vs Claude Code `/insights`

Claude Code has a built-in `/insights` command. rwd serves a different purpose.

`/insights` answers **"Am I using Claude efficiently?"** — it analyzes 30 days of tool usage patterns, token consumption, and friction points as an HTML dashboard. It's a coding efficiency report.

rwd answers **"What did I decide today, and why?"** — it extracts decisions, curiosities, and model corrections from all sessions and accumulates them as Obsidian Daily Notes. It's a developer journal.

| | `/insights` | `rwd` |
|--|-------------|-------|
| Focus | Tool usage patterns, efficiency | Decisions, learning, model corrections |
| Period | 30-day rolling | Daily |
| Output | HTML dashboard (one-off) | Obsidian Daily Notes (cumulative) |
| Analysis | Quantitative (tokens, tool counts, code volume) | Qualitative (why A over B, what was confusing) |
| Environment | Inside Claude Code session only | Standalone CLI, runs anywhere |

## Installation

### One-line install (macOS Apple Silicon / Linux x86_64)

```bash
curl -fsSL https://raw.githubusercontent.com/gigagookbob/rwd/main/install.sh | bash
```

### One-line install (Windows)

```powershell
irm https://raw.githubusercontent.com/gigagookbob/rwd/main/install.ps1 | iex
```

### Build from source

```bash
cargo install --git https://github.com/gigagookbob/rwd.git
```

### Prebuilt binary targets

- `aarch64-apple-darwin` (macOS Apple Silicon)
- `x86_64-unknown-linux-gnu` (Linux x86_64)
- `x86_64-pc-windows-msvc` (Windows x86_64)

> If macOS shows "unidentified developer" warning:
> ```bash
> xattr -d com.apple.quarantine /usr/local/bin/rwd
> ```

## Setup

```bash
rwd init
```

Sets up your LLM provider, credentials, output path, and language preference.
If you choose `codex`, `rwd` uses your existing `codex login` session (no API key required).
Obsidian vault is auto-detected if available.
Config is stored at `~/.config/rwd/config.toml`.

Auth methods by provider:
- `anthropic`, `openai`: API key auth
- `codex`: Codex login session auth (`codex login`)

### Change settings

```bash
rwd config output-path /path/to/vault    # Change output path
rwd config provider codex                # Change LLM provider
rwd config api-key sk-...                # Set API key for current provider (openai/anthropic)
rwd config openai-api-key sk-...         # Set OpenAI API key directly
rwd config anthropic-api-key sk-ant-...  # Set Anthropic API key directly
rwd config codex-model gpt-5.4           # Override Codex model (default: gpt-5.4)
rwd config codex-reasoning xhigh         # Override Codex reasoning (default: xhigh)
rwd config lang ko                       # Change output language (en/ko)
rwd auth status                          # Show provider auth method + credential state
```

### Sensitive data masking

When running `rwd today`, sensitive data in session logs (API keys, tokens, private IPs, etc.) is automatically masked before sending to the LLM. Enabled by default. To disable, add to `~/.config/rwd/config.toml`:

```toml
[redactor]
enabled = false
```

## Usage

```bash
rwd today              # Analyze today's AI sessions → save to Obsidian Daily Notes
rwd today --lang ko    # Override output language for this run
rwd summary            # Generate progress summary (Markdown) → save + copy to clipboard
rwd slack              # Generate Slack-ready message → copy to clipboard
rwd config             # Change settings (interactive menu)
rwd auth status        # Show current provider auth status
rwd update             # Update to the latest version
```

## Release policy (maintainers)

- Release is triggered automatically on `main` push only when these files changed:
  - `src/**`, `prompts/**`, `Cargo.toml`, `Cargo.lock`
- Release tag is derived from `Cargo.toml` package version (`vX.Y.Z`).
- If the tag already exists, release is skipped.
- Docs-only changes (for example `README.md`, `docs/**`) do not trigger release.
- Release notes are published in two layers: a short custom summary for `rwd`, then GitHub's full categorized changelog.
- Maintainer tone and workflow guide: [docs/RELEASES.md](docs/RELEASES.md)

## Uninstall

```bash
# Remove rwd binary
sh -c 'rm "$(command -v rwd)"'

# Also remove config/data:
rm -rf ~/.config/rwd ~/.rwd
```

## License

MIT
