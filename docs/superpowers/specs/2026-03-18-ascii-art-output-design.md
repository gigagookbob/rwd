# Design: rwd today ASCII Art Output

## Purpose

Improve the terminal output of `rwd today` with a classic block ASCII logo + Unicode box-drawing table.

## Design

### Changed File

`src/main.rs` — output formatting changes only. No logic changes.

### Logo Banner (Cyan)

```
  ██████  ██     ██ ██████
  ██   ██ ██     ██ ██   ██
  ██████  ██  █  ██ ██   ██
  ██   ██ ██ ███ ██ ██   ██
  ██   ██  ███ ███  ██████
    rewind your day  v0.5.1
```

### Info Box (Unicode Box Drawing)

```
  ┌──────────────────────────────────┐
  │  2026-03-18 09:51                │
  ├──────────────────────────────────┤
  │  Claude Code                     │
  │  08:51 ~ 09:51                   │
  │  Sessions 8  in 63,832,010  out 122,882│
  ├──────────────────────────────────┤
  │  Codex                           │
  │  09:16 ~ 09:51                   │
  │  Sessions 1                      │
  └──────────────────────────────────┘
```

### Colors (ANSI, safe for both light/dark themes)

| Element | Color |
|---------|-------|
| Logo | Cyan |
| "Claude Code" | Bright Blue |
| "Codex" | Yellow |
| Box lines / general text | Default (reset) |
| Provider info | Magenta |
| Version/tagline | Dim (gray) |

### What Stays Unchanged

- Insight output (`print_insights`) — unchanged
- Cache status messages — unchanged
- Scanning line — removed (the box already contains all the info)
- Business logic — only output formatting changes

### Implementation Approach

- Use ANSI escape codes directly (`\x1b[36m` etc.)
- No additional crates — the color count is small and simple enough to handle directly
- Box width is fixed (not dynamically adjusted based on content length)
