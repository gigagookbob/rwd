# Service Intro & GitHub About — Design Spec

## Goal

Reframe rwd's public-facing copy (GitHub About + README intro) to reflect its core character: **rewinding your day with AI agents**, rather than being a technical log analysis tool.

## Key Decisions

- Drop "coding" from the identity — Claude Code and Codex are AI agents; "coding agent" is an unnecessary narrowing
- Lead with value ("rewind your day"), follow with what you get
- Keep English throughout

## Changes

### GitHub About (one-line)

**Before:**
> CLI tool that analyzes AI coding session logs and extracts daily development insights

**After:**
> Rewind your day with AI agents. Daily notes, insights, and more

### README intro (first paragraph)

**Before:**
> CLI tool that analyzes AI coding session logs, extracts daily development insights, and saves them as Markdown to your Obsidian vault.

**After:**
> CLI that turns your AI agent sessions into a daily journal. Extracts decisions, learnings, and corrections, then saves them as Obsidian Daily Notes.

## Out of Scope

- README comparison table, installation, usage sections — unchanged
- Cargo.toml description — unchanged (separate task if needed)
- Landing page or marketing copy — not planned
