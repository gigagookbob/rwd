# Dynamic max_tokens Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dynamically calculate LLM output tokens proportional to the session count, and automatically switch to multi-step when the model limit is exceeded.

**Architecture:** Add output token estimation logic to the planner so that the single-shot decision also considers output limits. Remove the hardcoded max_tokens from call_api and pass a dynamic value instead.

**Tech Stack:** Rust, existing planner/provider/anthropic/openai modules

**Spec:** `docs/superpowers/specs/2026-03-18-dynamic-max-tokens-design.md`

---

## File Map

| File | Role | Change |
|------|------|--------|
| `src/analyzer/planner.rs` | Execution plan building | Output estimation function + extend single-shot condition + `recommended_max_tokens` field |
| `src/analyzer/provider.rs` | Provider abstraction | `max_output_tokens()` method + `call_api()` signature change |
| `src/analyzer/anthropic.rs` | Anthropic API | Add `max_tokens` parameter to `call_anthropic_api()` |
| `src/analyzer/openai.rs` | OpenAI API | Add `max_tokens` parameter to `call_openai_api()` |
| `src/analyzer/mod.rs` | Analysis orchestration | Get dynamic max_tokens from planner and pass to call_api |

---

## Chunk 1: Planner Output Estimation + Condition Extension

### Task 1: planner.rs — output estimation constants and function

**Files:**
- Modify: `src/analyzer/planner.rs`

- [ ] **Step 1: Write tests — multi-step switch on output overflow**

Add to `#[cfg(test)] mod tests` in `planner.rs`:

```rust
#[test]
fn test_single_shot_blocked_by_output_limit() {
    // Input fits within ITPM but 22 sessions → estimated output 42900 > 32000
    let limits = RateLimits::default_generous();
    let estimates: Vec<SessionEstimate> = (0..22)
        .map(|i| SessionEstimate {
            session_id: format!("s{i}"),
            estimated_tokens: 10_000,
        })
        .collect();
    let plan = build_execution_plan(&limits, &estimates, 32_000);
    assert!(!plan.is_single_shot);
    assert!(!plan.steps.is_empty());
}

#[test]
fn test_single_shot_allowed_when_output_fits() {
    let limits = RateLimits::default_generous();
    let estimates = vec![
        SessionEstimate { session_id: "s1".into(), estimated_tokens: 50_000 },
        SessionEstimate { session_id: "s2".into(), estimated_tokens: 30_000 },
    ];
    // 2 sessions × 1500 × 1.3 = 3900 < 32000 → single-shot
    let plan = build_execution_plan(&limits, &estimates, 32_000);
    assert!(plan.is_single_shot);
}

#[test]
fn test_recommended_max_tokens_scales_with_sessions() {
    let limits = RateLimits::default_generous();
    let estimates: Vec<SessionEstimate> = (0..10)
        .map(|i| SessionEstimate {
            session_id: format!("s{i}"),
            estimated_tokens: 5_000,
        })
        .collect();
    let plan = build_execution_plan(&limits, &estimates, 32_000);
    // 10 sessions × 1500 × 1.3 = 19500
    assert_eq!(plan.recommended_max_tokens, 19_500);
}
```

- [ ] **Step 2: Run tests — verify failure**

Run: `cargo test --lib planner::tests -- --nocapture 2>&1 | tail -10`
Expected: compile error (build_execution_plan signature change needed)

- [ ] **Step 3: Modify planner.rs — output estimation + condition extension**

Add constants to `planner.rs`:

```rust
/// Expected output tokens per session.
/// Measured: 16 sessions exceeded 16384 tokens → ~1000+ per session, conservatively set to 1500.
const OUTPUT_TOKENS_PER_SESSION: u64 = 1_500;

/// Output headroom ratio (30%).
const OUTPUT_MARGIN: f64 = 1.3;
```

Add field to `ExecutionPlan`:

```rust
pub struct ExecutionPlan {
    pub rate_limits: RateLimits,
    pub steps: Vec<ExecutionStep>,
    pub total_estimated_tokens: u64,
    pub is_single_shot: bool,
    pub recommended_max_tokens: u64,  // added
}
```

Add `model_max_output: u64` parameter to `build_execution_plan` signature:

```rust
pub fn build_execution_plan(
    limits: &RateLimits,
    estimates: &[SessionEstimate],
    model_max_output: u64,
) -> ExecutionPlan {
    let itpm = limits.input_tokens_per_minute;
    let total: u64 = estimates.iter().map(|e| e.estimated_tokens).sum();
    let num_sessions = estimates.len() as u64;

    // Estimate output tokens
    let estimated_output = (num_sessions * OUTPUT_TOKENS_PER_SESSION) as f64 * OUTPUT_MARGIN;
    let recommended_max_tokens = (estimated_output as u64).min(model_max_output);

    // Single-shot condition: both input AND output must fit within limits
    if total + SUMMARY_BUDGET_TOKENS <= itpm
        && (estimated_output as u64) <= model_max_output
    {
        return ExecutionPlan {
            rate_limits: limits.clone(),
            steps: Vec::new(),
            total_estimated_tokens: total,
            is_single_shot: true,
            recommended_max_tokens,
        };
    }

    // Determine strategy per session (existing logic preserved)
    let steps: Vec<ExecutionStep> = estimates
        .iter()
        .map(|est| {
            let strategy = if est.estimated_tokens <= itpm {
                StepStrategy::Direct
            } else {
                let chunks = ((est.estimated_tokens as f64) / (itpm as f64)).ceil() as usize;
                StepStrategy::Summarize { chunks: chunks.max(2) }
            };
            ExecutionStep {
                session_id: est.session_id.clone(),
                strategy,
                estimated_tokens: est.estimated_tokens,
            }
        })
        .collect();

    ExecutionPlan {
        rate_limits: limits.clone(),
        steps,
        total_estimated_tokens: total,
        is_single_shot: false,
        recommended_max_tokens,
    }
}
```

- [ ] **Step 4: Update existing tests — add 3rd argument**

Add `, 32_000` to all existing 6 tests' `build_execution_plan(&limits, &estimates)` calls.

- [ ] **Step 5: Run tests — verify pass**

Run: `cargo test --lib planner::tests -- --nocapture 2>&1 | tail -10`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/analyzer/planner.rs
git commit -m "feat: add output token estimation + output limit condition to planner"
```

---

## Chunk 2: Provider + API Function Signature Changes

### Task 2: provider.rs — max_output_tokens + call_api change

**Files:**
- Modify: `src/analyzer/provider.rs`

- [ ] **Step 1: Add `max_output_tokens()` method**

Add to `impl LlmProvider` block:

```rust
/// Per-model maximum output tokens.
/// Model spec — constant regardless of tier/user.
pub fn max_output_tokens(&self) -> u64 {
    match self {
        LlmProvider::Anthropic => 32_000,
        LlmProvider::OpenAi => 16_384,
    }
}
```

- [ ] **Step 2: Add `max_tokens` to `call_api()` signature**

```rust
pub async fn call_api(
    &self,
    api_key: &str,
    conversation_text: &str,
    max_tokens: u32,
) -> Result<String, super::AnalyzerError> {
    match self {
        LlmProvider::Anthropic => {
            super::anthropic::call_anthropic_api(
                api_key, SYSTEM_PROMPT, conversation_text, max_tokens,
            ).await
        }
        LlmProvider::OpenAi => {
            super::openai::call_openai_api(
                api_key, SYSTEM_PROMPT, conversation_text, max_tokens,
            ).await
        }
    }
}
```

Also add `max_tokens: u32` parameter to `call_summary_api` (default to 16384).

- [ ] **Step 3: cargo check — verify compile errors (anthropic/openai not yet updated)**

Run: `cargo check 2>&1 | tail -5`
Expected: argument count mismatch errors in anthropic.rs/openai.rs

### Task 3: anthropic.rs — parameterize max_tokens

**Files:**
- Modify: `src/analyzer/anthropic.rs`

- [ ] **Step 1: Change `call_anthropic_api()` signature**

```rust
pub async fn call_anthropic_api(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,  // added
) -> Result<String, super::AnalyzerError> {
```

Replace the hardcoded `max_tokens: 16384` in the function body with `max_tokens`:

```rust
let request_body = ApiRequest {
    model: MODEL.to_string(),
    max_tokens,  // hardcoded value removed
    system: system_prompt.to_string(),
    // ...
};
```

### Task 4: openai.rs — parameterize max_tokens

**Files:**
- Modify: `src/analyzer/openai.rs`

- [ ] **Step 1: Change `call_openai_api()` signature**

Same pattern as anthropic.rs. `max_tokens: 16384` → `max_tokens` parameter.

- [ ] **Step 2: cargo check — verify compilation passes**

Run: `cargo check 2>&1 | tail -5`
Expected: argument count mismatch error in `mod.rs` (call_api call sites not yet updated)

- [ ] **Step 3: Commit**

```bash
git add src/analyzer/provider.rs src/analyzer/anthropic.rs src/analyzer/openai.rs
git commit -m "feat: add dynamic max_tokens parameter to call_api"
```

---

## Chunk 3: mod.rs — Dynamic max_tokens Passthrough + Integration

### Task 5: mod.rs — update call sites

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: Pass `model_max_output` to planner call in `analyze_entries()`**

```rust
let plan = planner::build_execution_plan(
    &limits,
    &estimates,
    provider.max_output_tokens(),
);
```

- [ ] **Step 2: Pass dynamic max_tokens to single-shot `call_api()` call**

```rust
let raw_response = provider.call_api(
    &api_key,
    &final_prompt,
    plan.recommended_max_tokens as u32,
).await?;
```

- [ ] **Step 3: Update `analyze_codex_entries()` `call_api()` call**

Codex calls one session at a time, so use a fixed value:

```rust
let max_tokens = (1500_f64 * 1.3) as u32; // 1 session = 1950
let raw_response = provider.call_api(&api_key, &final_prompt, max_tokens).await?;
```

- [ ] **Step 4: Update `execute_plan()` `call_api()` calls**

Multi-step processes one session at a time, so use the same 1-session value:

```rust
// Pass max_tokens: (1500_f64 * 1.3) as u32 to call_api calls in
// execute_direct_step and execute_summarize_step
```

- [ ] **Step 5: Update `call_summary_api()` call site**

Summary is short, so use a fixed 16384:

```rust
provider.call_summary_api(&api_key, session_summaries, 16384).await?
```

- [ ] **Step 6: Run cargo clippy + cargo test — verify all pass**

Run: `cargo clippy 2>&1 | tail -5 && cargo test 2>&1 | tail -5`
Expected: no warnings, 91+ tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/analyzer/mod.rs
git commit -m "feat: calculate dynamic max_tokens in planner and pass to API calls"
```

---

## Chunk 4: Integration Test + Cleanup

### Task 6: Manual verification

- [ ] **Step 1: Run `cargo run -- today`**

Run on a day with 16+ sessions to verify no JSON parsing errors occur.

- [ ] **Step 2: Display estimated output tokens in plan output (optional)**

Currently only input tokens are shown. Adding output estimation helps with debugging:

```
✓ Analyzing all logs in a single shot (estimated input 318247 tokens, output max 19500 tokens)
```

- [ ] **Step 3: Final commit + PR**

```bash
git push origin dev
```
