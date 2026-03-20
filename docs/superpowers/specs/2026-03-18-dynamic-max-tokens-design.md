# Dynamic max_tokens + Automatic Splitting on Output Overflow

## Problem

- When analyzing 16 sessions in a single shot with `rwd today`, the LLM response JSON gets truncated at `max_tokens: 16384`
- Truncated JSON → parse failure → entire analysis aborted
- `claude-opus-4-6`'s actual max output is 32,000, but only half was being used

## Design

### 1. Per-Model max output Constants

```rust
// provider.rs
impl LlmProvider {
    fn max_output_tokens(&self) -> u64 {
        match self {
            LlmProvider::Anthropic => 32_000,
            LlmProvider::OpenAi => 16_384,
        }
    }
}
```

### 2. Output Token Estimation

Estimate output size based on session count.

```
OUTPUT_TOKENS_PER_SESSION = 1500
estimated_output = num_sessions × 1500 × 1.3 (30% headroom)
```

Rationale for 1500: analyzing 16 sessions exceeded 16384 tokens → ~1000+ per session. Conservatively set to 1500.

### 3. Dynamic max_tokens Calculation

```
dynamic_max_tokens = min(estimated_output, model_max_output)
```

Pass the dynamic value to `call_api()` instead of the hardcoded 16384.

### 4. Planner Extension — Switch to Multi-Step on Output Overflow

Add an output condition to `build_execution_plan()`'s single-shot decision:

```rust
// Before
if total_input + SUMMARY_BUDGET <= itpm { single_shot }

// After
if total_input + SUMMARY_BUDGET <= itpm
   && estimated_output <= model_max_output { single_shot }
```

When output is exceeded, split sessions into groups and process them through the existing multi-step pipeline.

Group size: `model_max_output / (OUTPUT_TOKENS_PER_SESSION × 1.3)` sessions per group.

### 5. call_api Signature Change

```rust
// Before: max_tokens hardcoded
pub async fn call_api(&self, api_key: &str, prompt: &str) -> Result<String, Error>

// After: max_tokens parameter added
pub async fn call_api(&self, api_key: &str, prompt: &str, max_tokens: u32) -> Result<String, Error>
```

### Changed Files

| File | Change |
|------|--------|
| `planner.rs` | Output estimation + add output limit to single-shot condition + `num_sessions` parameter |
| `provider.rs` | `max_output_tokens()` method, `call_api()` signature change |
| `anthropic.rs` | Add `max_tokens` parameter to `call_anthropic_api()` |
| `openai.rs` | Add `max_tokens` parameter to `call_openai_api()` |
| `mod.rs` | Dynamic max_tokens calculation + update call_api call sites |

### Tests

- `test_single_shot_blocked_by_output_limit` — input fits within ITPM but too many sessions cause output overflow → multi-step
- `test_dynamic_max_tokens_calculation` — verify max_tokens calculation based on session count
- `test_single_shot_with_few_sessions` — few sessions → single-shot as before
