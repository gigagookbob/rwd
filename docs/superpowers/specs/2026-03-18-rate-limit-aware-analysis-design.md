# Design: Rate Limit Aware Analysis Engine

## Background

The previous design (token-limit-fallback) used a "try → fail → fallback" approach. However, the actual ITPM differences between tiers are extreme:

| Tier | Claude Sonnet ITPM | Note |
|------|--------------------|------|
| Tier 1 | 30,000 | Most individual users |
| Tier 4 | 2,000,000 | 67x difference |

Tier 1 users were effectively unable to use rwd, receiving only 429 errors. "Working instead of erroring" is the core principle of this design.

## Relationship to Previous Design

This **completely replaces** the try-fallback approach from `2026-03-18-token-limit-fallback-design.md`. The existing 400/429 error detection logic and `analyze_entries_by_session()` fallback function are removed and replaced with a probe-based proactive planning approach.

## Scope

- **Included:** `analyze_entries()` (Claude Code log analysis) — primary target of this design
- **Excluded:** `analyze_codex_entries()` — already has a per-session analysis structure, so excluded from current scope. Can be unified into the same pipeline in the future.
- **Excluded:** `analyze_summary()` — separate API call but with low token usage (summarizes analysis results). However, the execution plan reserves at least 5,000 tokens of headroom.

## Design Principles

- Must work on all tiers
- Must not impose unnecessary overhead on high-tier users
- Must show progress transparently to the user

## Architecture Overview

```
analyze_entries(entries, redactor_enabled)
  → probe_rate_limits(provider, api_key) → RateLimits (default_generous on failure)
  → estimate_sessions(entries) → Vec<SessionEstimate>
  → build_execution_plan(rate_limits, estimates) → ExecutionPlan
  → display_plan(plan)
  → execute_plan(plan, provider, api_key, redactor_enabled)
      → is_single_shot: send all at once (same as before)
      → otherwise: sequential step execution
          → Direct: session prompt → API call
          → Summarize: chunk split → summarize → merge → analyze
          → rate pacing between steps
  → merge_results → final result
```

## Section 1: Probe Module

### Purpose

Determine the user's actual rate limits before making API calls.

### Behavior

- Make an API call with a minimal message ("ping")
- Extract rate limit info from response headers:
  - Claude: `anthropic-ratelimit-input-tokens-limit`
  - OpenAI: `x-ratelimit-limit-tokens`
- Return results as a `RateLimits` struct

### Types

```rust
pub struct RateLimits {
    pub input_tokens_per_minute: u64,
    pub output_tokens_per_minute: u64,
    pub requests_per_minute: u64,
}
```

### Cost

~10 input tokens + ~10 output tokens. Less than $0.0001 on Claude Sonnet.

### Probe Failure

Scenarios where probe can fail: network errors, 401 (invalid key), 5xx, missing headers (proxy/custom gateway).

**Recovery strategy:** proceed with defaults on probe failure.

```rust
impl RateLimits {
    /// Generous defaults used when probe fails.
    /// Most users will proceed with single_shot,
    /// and the runtime safety net handles actual limits.
    pub fn default_generous() -> Self {
        Self {
            input_tokens_per_minute: 1_000_000,
            output_tokens_per_minute: 200_000,
            requests_per_minute: 1_000,
        }
    }
}
```

### Scope of Changes

- `analyzer/anthropic.rs`: Add `probe_rate_limits()` function. Requires a low-level function to parse response headers, separate from existing `call_api`.
- `analyzer/openai.rs`: Add `probe_rate_limits()` function with the same structure.
- `analyzer/provider.rs`: Add `probe_rate_limits()` dispatch method to `LlmProvider`.

## Section 2: Token Estimator

### Purpose

Estimate prompt token counts locally without making API calls.

### Method

- Use character count instead of a precise tokenizer
- Ratio: `character count / 2` (Korean syllables are ~1 token each, so this is a conservative estimate for mixed Korean/English text)
- Include system prompt tokens: `const SYSTEM_PROMPT_ESTIMATED_TOKENS: u64` (defined in `prompt.rs`, pre-calculated since the string is fixed)

### Types

```rust
pub struct SessionEstimate {
    pub session_id: String,
    pub estimated_tokens: u64,
    pub entry_count: usize,
}
```

### Interface

```rust
/// Per-session token estimation
pub fn estimate_sessions(entries: &[LogEntry]) -> Vec<SessionEstimate>
```

### Scope of Changes

- `analyzer/prompt.rs`: Add estimation function. Leverages existing `build_prompt` and `extract_session_ids`.

## Section 3: Execution Plan Building

### Purpose

Compare ITPM against per-session estimated tokens to determine the overall execution strategy.

### Strategy Branching

```
Session estimated tokens vs ITPM
  ├─ Total sum ≤ ITPM → is_single_shot: true (send all at once)
  ├─ Individual session ≤ ITPM → Direct step (sequential per-session)
  └─ Individual session > ITPM → Summarize step (chunk split → summarize → analyze)
```

### Types

```rust
pub enum StepStrategy {
    Direct,
    Summarize { chunks: usize },
}

pub struct ExecutionStep {
    pub session_id: String,
    pub strategy: StepStrategy,
    pub estimated_tokens: u64,
}

pub struct ExecutionPlan {
    pub rate_limits: RateLimits,
    pub steps: Vec<ExecutionStep>,
    pub total_estimated_tokens: u64,
    pub is_single_shot: bool,
}
```

### Core Logic

If `is_single_shot`, send everything at once as before. No overhead for high-tier users.

### Scope of Changes

- New module `analyzer/planner.rs`

## Section 4: Summarization Strategy

### Purpose

Reduce oversized sessions that exceed ITPM by summarizing them into analyzable sizes.

### Flow

```
Large session (50K tokens)
  → Split into chunks based on ITPM (30K) → 2 chunks
  → Apply summarization prompt to each chunk
  → Merge summary results
  → Final analysis on the merged summary
```

### Summarization Prompt

Custom-designed for rwd's insight categories:

```
"Summarize the following development session conversation, focusing on:
- Technical decisions made and their rationale
- Mistakes or corrections
- Newly learned concepts (TIL)
- Interesting discoveries or questions
Preserve specific technical terms and context from the original."
```

### Chunk Splitting Unit

Split at conversation message (turn) boundaries. Never split in the middle of a message.

### Summary Output Size Limit

Set `max_tokens: 2000` on the summarization prompt so each chunk's summary stays under 2000 tokens. Even with N chunks merged, the total is limited to `N × 2000` tokens, reducing the chance of the final analysis prompt exceeding ITPM.

### Rate Pacing

Wait for token refill between chunk summarization calls. Wait time: `max(itpm_wait, rpm_wait)`
- `itpm_wait`: `(tokens_used / ITPM) × 60 seconds`
- `rpm_wait`: `60 / RPM` seconds (minimum request interval)

### Model

Both summarization and analysis use the same model configured by the user. A lightweight model option will be considered after future performance comparisons.

### Scope of Changes

- New module `analyzer/summarizer.rs`: includes chunk splitting, summarization calls, and `CHUNK_SUMMARIZE_PROMPT` constant
- Separate from existing `provider.rs`'s `SUMMARY_PROMPT` (for progress summaries). Named `CHUNK_SUMMARIZE_PROMPT` to avoid naming conflicts.

## Section 5: Execution Engine + UX Output

### Purpose

Receive an ExecutionPlan, execute steps sequentially, and display real-time progress.

### Interface

```rust
pub async fn execute_plan(
    plan: &ExecutionPlan,
    provider: &LlmProvider,
    api_key: &str,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError>
```

### Execution Flow

1. `is_single_shot` → call once as before
2. Otherwise → execute steps sequentially:
   - `Direct`: build session prompt → API call
   - `Summarize`: chunk split → summarize each chunk (with waits) → merge → analyze
3. Merge all step results with `merge_results`

### Token Budget Management

- Deduct used tokens after each call
- Remaining budget < next call's estimated tokens → wait for refill
- Wait time: `max(itpm_wait, rpm_wait)` (see Section 4 Rate pacing)
- Reserve headroom after the last step for the `analyze_summary()` call

### Step Failure Handling

When an individual step fails with a non-rate-limit error (network error, invalid response, etc.):
- Skip that session and print a warning
- Continue with remaining steps (partial success allowed)
- On rate limit error (429): wait for the retry-after header duration, then retry once. If retry also fails, skip that session.
  - Anthropic: standard `retry-after` header
  - OpenAI: `retry-after` or `x-ratelimit-reset-tokens` header

### Redaction Timing

In the Summarize flow, redaction is applied **just before each chunk's API call**. Chunk splitting is performed on the original text, and each split chunk is redacted before being sent to the API. This preserves token estimation accuracy.

### UX Output

```
⠋ Checking API limits...
✓ ITPM: 30,000 | OTPM: 8,000 | RPM: 50

✓ 3 sessions to analyze (estimated 85,000 total tokens)
  • session_abc123: 12,000 tokens → direct analysis
  • session_def456: 48,000 tokens → summarize then analyze (2 chunks)
  • session_ghi789: 25,000 tokens → direct analysis

⠋ [1/3] Analyzing session_abc123...
✓ [1/3] Complete
⠋ Waiting for next request... (24s)
⠋ [2/3] Summarizing session_def456... (chunk 1/2)
✓ [2/3] Summary chunk 1/2 complete
⠋ Waiting for next request... (58s)
⠋ [2/3] Summarizing session_def456... (chunk 2/2)
✓ [2/3] Summary complete, analyzing...
✓ [2/3] Complete
⠋ [3/3] Analyzing session_ghi789...
✓ [3/3] Complete

✓ Full analysis complete (3m 12s)
```

### Scope of Changes

- `analyzer/mod.rs`: Refactor existing `analyze_entries()`. Replace try-fallback logic with probe → plan → execute flow. Remove `analyze_entries_by_session()`, `is_context_limit_error()`, `is_rate_limit_error()`.

## Removal Targets

Remove existing fallback-related code:

- `analyzer/mod.rs`: `analyze_entries_by_session()` function
- `analyzer/mod.rs`: `is_context_limit_error()`, `is_rate_limit_error()` functions and tests
- `analyzer/mod.rs`: 400/429 error branching logic

**Kept:** `entry_session_id()` helper function — used for per-session filtering in the new execute_plan. Kept alongside `prompt.rs`'s `extract_session_ids()`.

**Kept:** `merge_results()` (insight.rs) — used as-is with no signature changes.

## New Module Summary

| Module | Responsibility |
|--------|---------------|
| `analyzer/planner.rs` (new) | Execution plan building — RateLimits + SessionEstimate → ExecutionPlan |
| `analyzer/summarizer.rs` (new) | Large session chunk splitting and summarization |

## Changed Module Summary

| Module | Changes |
|--------|---------|
| `analyzer/mod.rs` | Refactor analyze_entries, remove fallback, introduce execute_plan |
| `analyzer/anthropic.rs` | Add probe_rate_limits(), response header parsing |
| `analyzer/openai.rs` | Add probe_rate_limits(), response header parsing |
| `analyzer/provider.rs` | Add probe dispatch method |
| `analyzer/prompt.rs` | Add estimate_sessions() |

## Tests

- `planner.rs`: strategy branching tests (single_shot, direct, summarize)
- `planner.rs`: ITPM boundary value tests (estimated tokens == ITPM)
- `prompt.rs`: token estimation tests (Korean, English, mixed)
- `prompt.rs`: empty session (0 entries) handling
- `summarizer.rs`: chunk splitting boundary tests (message units)
- `summarizer.rs`: edge case where a single message exceeds ITPM
- `mod.rs`: verify default_generous applied on probe failure
- `planner.rs`: verify default_generous plan results in is_single_shot=true
- `mod.rs`: verify remaining steps continue on partial step failure
- `mod.rs`: verify skip-and-continue on 429 retry failure

## Implementation Notes

- The project structure tree in `docs/ARCHITECTURE.md` needs to be updated to reflect new modules (`planner.rs`, `summarizer.rs`)

## Related Issues

- GitHub Issue #36 (closed): Handling LLM token limits during large log analysis
