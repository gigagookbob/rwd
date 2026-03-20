# TIL Section Improvement Design

## Goal

Deprecate the shallow TIL items derived from `curiosities`/`corrections`, and instead have the LLM directly extract "what the user actually learned." Each item consists of a title (one line) + context description (1-2 lines) + session ID.

## Target Audience

My future self. For those moments three months from now when I wonder "why did I do it this way?"

## Data Structure

Add a `til` field to the existing `SessionInsight`:

```rust
pub struct TilItem {
    pub title: String,      // One-line title (what was learned)
    pub detail: String,     // 1-2 line context (why, how it was applied)
    pub session_id: String, // For tracing back to the original session
}
```

`AnalysisResult.sessions[].til: Vec<TilItem>`

## LLM Prompt Changes

Add a `til` field to the system prompt JSON schema:

```json
"til": [
  {
    "title": "One-line summary of what was learned",
    "detail": "Why it was needed and how it was applied, in 1-2 lines"
  }
]
```

Add TIL extraction guidelines to the rules:
- Do NOT derive from curiosities or corrections — directly extract what the user **actually learned** from the conversation
- Include only learning that is meaningful in the context of this session, not general knowledge
- Return an empty array if nothing was learned

## Markdown Output

Combine all session items in a `## TIL` section at the bottom. Each item includes a session ID as an HTML comment.

```markdown
## TIL

### serde's tag attribute doesn't work with nested JSON
Codex JSONL has the type field in two places, so serde tag couldn't parse it in one pass.
Worked around it with two-stage parsing (loose → structured).
<!-- session: d31e7507 -->

### chrono Local vs Utc
Using just date_naive() on DateTime<Utc> compares against UTC, which causes early-morning KST sessions to be missed.
Must convert with with_timezone(&Local) before comparing.
<!-- session: 342dfbf0 -->
```

## Impact Scope

| File | Change |
|------|--------|
| `analyzer/insight.rs` | Add `TilItem` struct, add `til` field to `SessionInsight` |
| `analyzer/provider.rs` | Add `til` schema + extraction rules to system prompt |
| `output/markdown.rs` | Modify `render_til_section` — title + description + session ID comment |

## Removed Logic

Remove the logic in `output/markdown.rs`'s `render_session()` that pushed `curiosities`/`corrections` into `til_items`. TIL items now come exclusively from the `SessionInsight.til` field. `curiosities` and `corrections` remain in their respective session sections as before.
