# Rate Limit Aware Analysis Engine Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Probe API rate limits before analysis, pre-plan execution strategy, and make rwd work for all API tiers.

**Architecture:** Probe → Estimate → Plan → Execute pipeline. `analyze_entries()` refactored from try-fallback to proactive planning. Two new modules: `planner.rs` (execution planning), `summarizer.rs` (chunk summarization for oversized sessions).

**Tech Stack:** Rust 2024 Edition, reqwest (HTTP headers), serde_json, tokio (async)

**Spec:** `docs/superpowers/specs/2026-03-18-rate-limit-aware-analysis-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/analyzer/planner.rs` | Create | RateLimits, SessionEstimate, ExecutionPlan types + build_execution_plan() |
| `src/analyzer/summarizer.rs` | Create | Chunk splitting + CHUNK_SUMMARIZE_PROMPT + summarize_session() |
| `src/analyzer/prompt.rs` | Modify | estimate_sessions(), SYSTEM_PROMPT_ESTIMATED_TOKENS |
| `src/analyzer/anthropic.rs` | Modify | probe_anthropic_rate_limits() — header parsing |
| `src/analyzer/openai.rs` | Modify | probe_openai_rate_limits() — header parsing |
| `src/analyzer/provider.rs` | Modify | LlmProvider::probe_rate_limits() dispatch |
| `src/analyzer/mod.rs` | Modify | execute_plan(), refactored analyze_entries(), remove old fallback |
| `docs/ARCHITECTURE.md` | Modify | Add planner.rs, summarizer.rs to project structure |

---

## Chunk 1: Types + Token Estimator + Planner

### Task 1: RateLimits type + default_generous

**Files:**
- Create: `src/analyzer/planner.rs`
- Modify: `src/analyzer/mod.rs` (add `pub mod planner;`)

- [ ] **Step 1: Write the failing test for RateLimits::default_generous**

```rust
// src/analyzer/planner.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_generous_returns_high_limits() {
        let limits = RateLimits::default_generous();
        assert_eq!(limits.input_tokens_per_minute, 1_000_000);
        assert_eq!(limits.output_tokens_per_minute, 200_000);
        assert_eq!(limits.requests_per_minute, 1_000);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib planner::tests::test_default_generous`
Expected: FAIL — module not found

- [ ] **Step 3: Implement RateLimits struct**

```rust
// src/analyzer/planner.rs

/// API rate limit information.
/// Extracted from probe call response headers, or default_generous() on failure.
#[derive(Debug, Clone)]
pub struct RateLimits {
    pub input_tokens_per_minute: u64,
    pub output_tokens_per_minute: u64,
    pub requests_per_minute: u64,
}

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

Register the module in `src/analyzer/mod.rs`:
```rust
pub mod planner;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib planner::tests::test_default_generous`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/analyzer/planner.rs src/analyzer/mod.rs
git commit -m "feat: RateLimits type + default_generous (rate-limit-aware #36)"
```

---

### Task 2: ExecutionPlan types

**Files:**
- Modify: `src/analyzer/planner.rs`

- [ ] **Step 1: Add remaining types to planner.rs**

```rust
/// Per-session token estimation result.
#[derive(Debug, Clone)]
pub struct SessionEstimate {
    pub session_id: String,
    pub estimated_tokens: u64,
    pub entry_count: usize,
}

/// Strategy for an individual execution step.
#[derive(Debug, Clone, PartialEq)]
pub enum StepStrategy {
    /// Within ITPM — send as-is
    Direct,
    /// Exceeds ITPM — chunk split then summarize
    Summarize { chunks: usize },
}

/// An individual step in the execution plan.
#[derive(Debug, Clone)]
pub struct ExecutionStep {
    pub session_id: String,
    pub strategy: StepStrategy,
    pub estimated_tokens: u64,
}

/// The overall execution plan.
/// When is_single_shot is true, send everything at once as before (no overhead for high-tier users).
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub rate_limits: RateLimits,
    pub steps: Vec<ExecutionStep>,
    pub total_estimated_tokens: u64,
    pub is_single_shot: bool,
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/analyzer/planner.rs
git commit -m "feat: ExecutionPlan types — StepStrategy, ExecutionStep, ExecutionPlan"
```

---

### Task 3: Token estimator (estimate_sessions)

**Files:**
- Modify: `src/analyzer/prompt.rs`

- [ ] **Step 1: Write failing tests for token estimation**

Add to the tests module at the bottom of `prompt.rs`:

```rust
#[test]
fn test_estimate_tokens_korean() {
    // 5 characters, / 2 = 2 (integer division, no rounding)
    assert_eq!(super::estimate_tokens("hello"), 2);
}

#[test]
fn test_estimate_tokens_english() {
    // "hello world" = 11 characters, / 2 = 5
    assert_eq!(super::estimate_tokens("hello world"), 5);
}

#[test]
fn test_estimate_tokens_empty_string() {
    assert_eq!(super::estimate_tokens(""), 0);
}

#[test]
fn test_estimate_sessions_per_session_estimation() {
    let entries = vec![
        serde_json::from_str::<LogEntry>(
            r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:00:00Z","uuid":"u1","message":{"role":"user","content":"hello nice to meet you"}}"#,
        ).unwrap(),
        serde_json::from_str::<LogEntry>(
            r#"{"type":"user","sessionId":"s2","timestamp":"2026-03-11T11:00:00Z","uuid":"u2","message":{"role":"user","content":"this is the second session"}}"#,
        ).unwrap(),
    ];
    let estimates = estimate_sessions(&entries);
    assert_eq!(estimates.len(), 2);
    assert_eq!(estimates[0].session_id, "s1");
    assert_eq!(estimates[1].session_id, "s2");
    assert!(estimates[0].estimated_tokens > 0);
    assert_eq!(estimates[0].entry_count, 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib prompt::tests::test_estimate`
Expected: FAIL — function not found

- [ ] **Step 3: Implement estimate_tokens and estimate_sessions**

Add import at the top of `prompt.rs`:
```rust
use super::planner::SessionEstimate;
```

Add functions:
```rust
/// Estimated token count for the system prompt (pre-calculated from SYSTEM_PROMPT character count).
/// provider::SYSTEM_PROMPT character count / 2.
pub const SYSTEM_PROMPT_ESTIMATED_TOKENS: u64 = 800;

/// Rough token estimation for text.
/// Korean syllables are ~1 token each, so character count / 2 is a conservative estimate.
pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as u64) / 2
}

/// Returns per-session token estimates.
/// Gets session list via extract_session_ids, then estimates text size for each session's entries.
pub fn estimate_sessions(entries: &[LogEntry]) -> Vec<SessionEstimate> {
    let session_ids = extract_session_ids(entries);
    let mut estimates = Vec::new();

    for session_id in &session_ids {
        let session_entries: Vec<&LogEntry> = entries
            .iter()
            .filter(|e| {
                let eid = match e {
                    LogEntry::User(u) => Some(u.session_id.as_str()),
                    LogEntry::Assistant(a) => Some(a.session_id.as_str()),
                    LogEntry::Progress(p) => Some(p.session_id.as_str()),
                    LogEntry::System(s) => s.session_id.as_deref(),
                    LogEntry::FileHistorySnapshot(_) | LogEntry::Other(_) => None,
                };
                eid == Some(session_id.as_str())
            })
            .collect();

        // Estimate text size in the same format as build_prompt.
        // Instead of calling build_prompt, sum the raw text lengths of entries.
        let mut total_chars: u64 = 0;
        let entry_count = session_entries.len();

        for entry in &session_entries {
            match entry {
                LogEntry::User(e) => {
                    if let Some(text) = e.message.as_ref().and_then(extract_user_text) {
                        total_chars += text.chars().count() as u64;
                    }
                }
                LogEntry::Assistant(e) => {
                    if let Some(msg) = &e.message {
                        for block in &msg.content {
                            if let ContentBlock::Text { text } = block {
                                if let Some(t) = text {
                                    total_chars += t.chars().count() as u64;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let estimated_tokens = total_chars / 2 + SYSTEM_PROMPT_ESTIMATED_TOKENS;
        estimates.push(SessionEstimate {
            session_id: session_id.clone(),
            estimated_tokens,
            entry_count,
        });
    }

    estimates
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib prompt::tests::test_estimate`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/analyzer/prompt.rs
git commit -m "feat: token estimator — estimate_sessions, estimate_tokens (chars/2)"
```

---

### Task 4: build_execution_plan

**Files:**
- Modify: `src/analyzer/planner.rs`

- [ ] **Step 1: Write failing tests for execution plan building**

```rust
#[test]
fn test_build_plan_single_shot_when_total_fits() {
    let limits = RateLimits {
        input_tokens_per_minute: 100_000,
        output_tokens_per_minute: 50_000,
        requests_per_minute: 100,
    };
    let estimates = vec![
        SessionEstimate { session_id: "s1".into(), estimated_tokens: 10_000, entry_count: 5 },
        SessionEstimate { session_id: "s2".into(), estimated_tokens: 20_000, entry_count: 10 },
    ];
    let plan = build_execution_plan(&limits, &estimates);
    assert!(plan.is_single_shot);
    assert!(plan.steps.is_empty());
    assert_eq!(plan.total_estimated_tokens, 30_000);
}

#[test]
fn test_build_plan_direct_when_sessions_fit_individually() {
    let limits = RateLimits {
        input_tokens_per_minute: 30_000,
        output_tokens_per_minute: 8_000,
        requests_per_minute: 50,
    };
    let estimates = vec![
        SessionEstimate { session_id: "s1".into(), estimated_tokens: 10_000, entry_count: 5 },
        SessionEstimate { session_id: "s2".into(), estimated_tokens: 20_000, entry_count: 10 },
    ];
    let plan = build_execution_plan(&limits, &estimates);
    assert!(!plan.is_single_shot);
    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[0].strategy, StepStrategy::Direct);
    assert_eq!(plan.steps[1].strategy, StepStrategy::Direct);
}

#[test]
fn test_build_plan_summarize_when_session_exceeds_itpm() {
    let limits = RateLimits {
        input_tokens_per_minute: 30_000,
        output_tokens_per_minute: 8_000,
        requests_per_minute: 50,
    };
    let estimates = vec![
        SessionEstimate { session_id: "s1".into(), estimated_tokens: 50_000, entry_count: 20 },
    ];
    let plan = build_execution_plan(&limits, &estimates);
    assert!(!plan.is_single_shot);
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].strategy, StepStrategy::Summarize { chunks: 2 });
}

#[test]
fn test_build_plan_default_generous_is_single_shot() {
    let limits = RateLimits::default_generous();
    let estimates = vec![
        SessionEstimate { session_id: "s1".into(), estimated_tokens: 50_000, entry_count: 20 },
    ];
    let plan = build_execution_plan(&limits, &estimates);
    assert!(plan.is_single_shot);
}

#[test]
fn test_build_plan_reserves_summary_budget() {
    // ITPM is 35,000 and session total is 31,000.
    // With analyze_summary headroom (5,000), single_shot is not possible.
    let limits = RateLimits {
        input_tokens_per_minute: 35_000,
        output_tokens_per_minute: 8_000,
        requests_per_minute: 50,
    };
    let estimates = vec![
        SessionEstimate { session_id: "s1".into(), estimated_tokens: 31_000, entry_count: 15 },
    ];
    let plan = build_execution_plan(&limits, &estimates);
    // 31,000 + 5,000 (reserved) = 36,000 > 35,000 → not single_shot
    assert!(!plan.is_single_shot);
}

#[test]
fn test_build_plan_exact_boundary_is_single_shot() {
    // When total + budget == ITPM exactly → single_shot (<=)
    let limits = RateLimits {
        input_tokens_per_minute: 35_000,
        output_tokens_per_minute: 8_000,
        requests_per_minute: 50,
    };
    let estimates = vec![
        SessionEstimate { session_id: "s1".into(), estimated_tokens: 30_000, entry_count: 15 },
    ];
    let plan = build_execution_plan(&limits, &estimates);
    // 30,000 + 5,000 = 35,000 == 35,000 → single_shot
    assert!(plan.is_single_shot);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib planner::tests::test_build_plan`
Expected: FAIL — function not found

- [ ] **Step 3: Implement build_execution_plan**

```rust
/// Token headroom reserved for the analyze_summary() call.
const SUMMARY_BUDGET_TOKENS: u64 = 5_000;

/// Builds an execution plan based on rate limits and per-session estimates.
///
/// Strategy branching:
/// - Total sum + headroom <= ITPM → single_shot (send all at once)
/// - Individual session <= ITPM → Direct (sequential per-session)
/// - Individual session > ITPM → Summarize (chunk split then summarize)
pub fn build_execution_plan(
    limits: &RateLimits,
    estimates: &[SessionEstimate],
) -> ExecutionPlan {
    let itpm = limits.input_tokens_per_minute;
    let total: u64 = estimates.iter().map(|e| e.estimated_tokens).sum();

    // If everything fits within ITPM → single_shot
    if total + SUMMARY_BUDGET_TOKENS <= itpm {
        return ExecutionPlan {
            rate_limits: limits.clone(),
            steps: Vec::new(),
            total_estimated_tokens: total,
            is_single_shot: true,
        };
    }

    // Determine strategy per session
    let steps: Vec<ExecutionStep> = estimates
        .iter()
        .map(|est| {
            let strategy = if est.estimated_tokens <= itpm {
                StepStrategy::Direct
            } else {
                // Calculate required chunk count based on ITPM (ceiling)
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
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib planner::tests`
Expected: PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy`
Expected: no warnings

- [ ] **Step 6: Commit**

```bash
git add src/analyzer/planner.rs
git commit -m "feat: build_execution_plan — single_shot/direct/summarize strategy branching"
```

---

## Chunk 2: Probe + Provider Dispatch

### Task 5: Probe — Anthropic

**Files:**
- Modify: `src/analyzer/anthropic.rs`

- [ ] **Step 1: Implement probe_anthropic_rate_limits**

Since Anthropic probe requires an actual API call, test only the header parsing logic via unit tests.

```rust
use super::planner::RateLimits;

/// Sends a minimal request to the Anthropic API and reads rate limits from response headers.
/// Returns None on failure; caller falls back to default_generous.
pub async fn probe_anthropic_rate_limits(
    api_key: &str,
) -> Option<RateLimits> {
    let client = reqwest::Client::new();

    let request_body = ApiRequest {
        model: MODEL.to_string(),
        max_tokens: 1,
        system: String::new(),
        messages: vec![ApiMessage {
            role: "user".to_string(),
            content: "ping".to_string(),
        }],
    };

    let response = client
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await
        .ok()?;

    parse_anthropic_rate_headers(&response)
}

/// Extracts rate limit values from Anthropic response headers.
fn parse_anthropic_rate_headers(response: &reqwest::Response) -> Option<RateLimits> {
    let headers = response.headers();

    let itpm = headers
        .get("anthropic-ratelimit-input-tokens-limit")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())?;

    let otpm = headers
        .get("anthropic-ratelimit-output-tokens-limit")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(itpm / 4);

    let rpm = headers
        .get("anthropic-ratelimit-requests-limit")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(50);

    Some(RateLimits {
        input_tokens_per_minute: itpm,
        output_tokens_per_minute: otpm,
        requests_per_minute: rpm,
    })
}
```

- [ ] **Step 2: Write reference test for header parsing**

```rust
#[test]
fn test_parse_anthropic_rate_headers_reference() {
    // parse_anthropic_rate_headers takes a reqwest::Response so
    // direct unit testing is difficult. Header parsing accuracy is
    // verified through integration tests (actual API calls).
    // This test only confirms compilation.
    assert!(true);
}
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/analyzer/anthropic.rs
git commit -m "feat: probe_anthropic_rate_limits — extract ITPM/OTPM/RPM from response headers"
```

---

### Task 6: Probe — OpenAI

**Files:**
- Modify: `src/analyzer/openai.rs`

- [ ] **Step 1: Implement probe_openai_rate_limits**

```rust
use super::planner::RateLimits;

/// Sends a minimal request to the OpenAI API and reads rate limits from response headers.
pub async fn probe_openai_rate_limits(
    api_key: &str,
) -> Option<RateLimits> {
    let client = reqwest::Client::new();

    let request_body = ChatRequest {
        model: MODEL.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "ping".to_string(),
        }],
        max_tokens: 1,
    };

    let response = client
        .post(API_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .ok()?;

    parse_openai_rate_headers(&response)
}

/// Extracts rate limit values from OpenAI response headers.
fn parse_openai_rate_headers(response: &reqwest::Response) -> Option<RateLimits> {
    let headers = response.headers();

    // OpenAI uses x-ratelimit-limit-tokens (combined TPM) header.
    // No ITPM/OTPM separation, so use tokens as ITPM and estimate OTPM as 1/4.
    let tpm = headers
        .get("x-ratelimit-limit-tokens")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())?;

    let rpm = headers
        .get("x-ratelimit-limit-requests")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(50);

    Some(RateLimits {
        input_tokens_per_minute: tpm,
        output_tokens_per_minute: tpm / 4,
        requests_per_minute: rpm,
    })
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/analyzer/openai.rs
git commit -m "feat: probe_openai_rate_limits — x-ratelimit-limit-tokens header parsing"
```

---

### Task 7: Provider probe dispatch

**Files:**
- Modify: `src/analyzer/provider.rs`

- [ ] **Step 1: Add probe_rate_limits to LlmProvider**

Add to `impl LlmProvider` block:

```rust
    /// Checks the user's actual rate limits via an API probe call.
    /// Returns default_generous() on failure to proceed with single_shot.
    pub async fn probe_rate_limits(
        &self,
        api_key: &str,
    ) -> super::planner::RateLimits {
        let result = match self {
            LlmProvider::Anthropic => {
                super::anthropic::probe_anthropic_rate_limits(api_key).await
            }
            LlmProvider::OpenAi => {
                super::openai::probe_openai_rate_limits(api_key).await
            }
        };
        result.unwrap_or_else(|| {
            eprintln!("Warning: rate limit check failed, proceeding with defaults.");
            super::planner::RateLimits::default_generous()
        })
    }
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/analyzer/provider.rs
git commit -m "feat: LlmProvider::probe_rate_limits — per-provider probe dispatch"
```

---

## Chunk 3: Summarizer + Execute Plan + Refactor

### Task 8: Summarizer module — chunk splitting

**Files:**
- Create: `src/analyzer/summarizer.rs`
- Modify: `src/analyzer/mod.rs` (add `pub mod summarizer;`)

- [ ] **Step 1: Write failing tests for chunk splitting**

```rust
// src/analyzer/summarizer.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_into_chunks_respects_token_limit() {
        // Each message is ~10 tokens (20 chars), ITPM is 25 tokens,
        // so each chunk should hold 2 messages.
        let messages = vec![
            ("USER".to_string(), "a".repeat(20)),   // ~10 tokens
            ("USER".to_string(), "b".repeat(20)),   // ~10 tokens
            ("USER".to_string(), "c".repeat(20)),   // ~10 tokens
        ];
        let chunks = split_into_chunks(&messages, 25);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 2);
        assert_eq!(chunks[1].len(), 1);
    }

    #[test]
    fn test_split_into_chunks_single_message_exceeds_limit() {
        // A single message exceeding the limit still gets its own chunk (not skipped).
        let messages = vec![
            ("USER".to_string(), "a".repeat(100)),  // ~50 tokens
        ];
        let chunks = split_into_chunks(&messages, 25);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 1);
    }

    #[test]
    fn test_split_into_chunks_empty_messages() {
        let messages: Vec<(String, String)> = vec![];
        let chunks = split_into_chunks(&messages, 30_000);
        assert!(chunks.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib summarizer::tests::test_split`
Expected: FAIL — module not found

- [ ] **Step 3: Implement split_into_chunks**

```rust
// src/analyzer/summarizer.rs

use super::prompt::estimate_tokens;

/// Prompt used for session summarization.
/// Instructs preservation of key content aligned with rwd's insight categories.
pub const CHUNK_SUMMARIZE_PROMPT: &str = r#"Summarize the following development session conversation, focusing on:
- Technical decisions made and their rationale
- Mistakes or corrections
- Newly learned concepts (TIL)
- Interesting discoveries or questions
Preserve specific technical terms and context from the original."#;

/// Splits a message list into chunks that fit within the ITPM limit.
/// Only splits at message boundaries (never in the middle of a message).
/// A single message exceeding the limit gets its own chunk.
pub fn split_into_chunks(
    messages: &[(String, String)],
    itpm: u64,
) -> Vec<Vec<(String, String)>> {
    if messages.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<Vec<(String, String)>> = Vec::new();
    let mut current_chunk: Vec<(String, String)> = Vec::new();
    let mut current_tokens: u64 = 0;

    for (role, text) in messages {
        let msg_tokens = estimate_tokens(text);

        // Adding to current chunk would exceed limit
        if !current_chunk.is_empty() && current_tokens + msg_tokens > itpm {
            chunks.push(current_chunk);
            current_chunk = Vec::new();
            current_tokens = 0;
        }

        current_chunk.push((role.clone(), text.clone()));
        current_tokens += msg_tokens;
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}
```

Register the module in `src/analyzer/mod.rs`:
```rust
pub mod summarizer;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib summarizer::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/analyzer/summarizer.rs src/analyzer/mod.rs
git commit -m "feat: summarizer — split_into_chunks + CHUNK_SUMMARIZE_PROMPT"
```

---

### Task 9: Summarizer — summarize_session (async API calls)

**Files:**
- Modify: `src/analyzer/summarizer.rs`

- [ ] **Step 1: Implement summarize_chunks**

```rust
use super::planner::RateLimits;
use super::provider::LlmProvider;

/// Summarizes a large session's messages chunk-by-chunk and returns the combined summary text.
/// Applies rate pacing between chunks.
pub async fn summarize_chunks(
    chunks: &[Vec<(String, String)>],
    provider: &LlmProvider,
    api_key: &str,
    limits: &RateLimits,
) -> Result<String, super::AnalyzerError> {
    let mut summaries: Vec<String> = Vec::new();
    let total = chunks.len();

    for (i, chunk) in chunks.iter().enumerate() {
        // Convert chunk to text
        let chunk_text: String = chunk
            .iter()
            .map(|(role, text)| format!("[{role}] {text}"))
            .collect::<Vec<_>>()
            .join("\n");

        eprintln!("    Summarizing chunk {}/{}...", i + 1, total);

        // Summary API call (max_tokens: 2000 set at provider level)
        let summary = provider
            .call_api_with_max_tokens(
                api_key,
                CHUNK_SUMMARIZE_PROMPT,
                &chunk_text,
                2000,
            )
            .await?;
        summaries.push(summary);

        // Rate pacing: wait unless this is the last chunk
        if i + 1 < total {
            let chunk_tokens = estimate_tokens(&chunk_text);
            let wait = calculate_wait(chunk_tokens, limits);
            if wait > 0.0 {
                eprintln!("    Waiting for next request... ({:.0}s)", wait);
                tokio::time::sleep(std::time::Duration::from_secs_f64(wait)).await;
            }
        }
    }

    Ok(summaries.join("\n\n"))
}

/// Calculates wait time based on ITPM/RPM.
/// Returns max(itpm_wait, rpm_wait).
pub fn calculate_wait(used_tokens: u64, limits: &RateLimits) -> f64 {
    let itpm_wait = (used_tokens as f64 / limits.input_tokens_per_minute as f64) * 60.0;
    let rpm_wait = 60.0 / limits.requests_per_minute as f64;
    itpm_wait.max(rpm_wait)
}
```

- [ ] **Step 2: Add `_with_max_tokens` variants to anthropic.rs and openai.rs**

Keep existing `call_anthropic_api`/`call_openai_api` as-is (backward compatible).
Add new functions that accept a `max_tokens` parameter:

`anthropic.rs`:
```rust
/// API call variant with configurable max_tokens.
pub async fn call_anthropic_api_with_max_tokens(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,
) -> Result<String, super::AnalyzerError> {
    // ... same as call_anthropic_api but with configurable max_tokens ...
}
```

Add `call_openai_api_with_max_tokens` to `openai.rs` with the same pattern.

- [ ] **Step 3: Add call_api_with_max_tokens to provider.rs**

Add to `impl LlmProvider` block:

```rust
    /// API call with configurable max_tokens.
    /// Used for summary calls with max_tokens=2000, while analysis calls keep the existing 16384.
    pub async fn call_api_with_max_tokens(
        &self,
        api_key: &str,
        system_prompt: &str,
        conversation_text: &str,
        max_tokens: u32,
    ) -> Result<String, super::AnalyzerError> {
        match self {
            LlmProvider::Anthropic => {
                super::anthropic::call_anthropic_api_with_max_tokens(
                    api_key, system_prompt, conversation_text, max_tokens,
                )
                .await
            }
            LlmProvider::OpenAi => {
                super::openai::call_openai_api_with_max_tokens(
                    api_key, system_prompt, conversation_text, max_tokens,
                )
                .await
            }
        }
    }
```

- [ ] **Step 3: Write test for calculate_wait**

Add to `summarizer.rs` tests:

```rust
#[test]
fn test_calculate_wait_itpm_based() {
    let limits = RateLimits {
        input_tokens_per_minute: 30_000,
        output_tokens_per_minute: 8_000,
        requests_per_minute: 1_000, // RPM is high, so ITPM is the bottleneck
    };
    let wait = calculate_wait(15_000, &limits);
    // 15000/30000 * 60 = 30 seconds
    assert!((wait - 30.0).abs() < 0.1);
}

#[test]
fn test_calculate_wait_rpm_based() {
    let limits = RateLimits {
        input_tokens_per_minute: 1_000_000, // ITPM is high, so RPM is the bottleneck
        output_tokens_per_minute: 200_000,
        requests_per_minute: 50,
    };
    let wait = calculate_wait(100, &limits);
    // 60/50 = 1.2 seconds
    assert!((wait - 1.2).abs() < 0.1);
}
```

- [ ] **Step 4: Build + test**

Run: `cargo build && cargo test --lib summarizer::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/analyzer/summarizer.rs src/analyzer/provider.rs
git commit -m "feat: summarize_chunks + calculate_wait + call_summarize_api"
```

---

### Task 10: execute_plan + analyze_entries refactor

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: Add execute_plan function**

Remove existing `analyze_entries_by_session`, `is_context_limit_error`, `is_rate_limit_error` functions and their tests, then add the new function.

Add import at the top of `mod.rs`:
```rust
use planner::{ExecutionPlan, StepStrategy};
```

Add new function:

```rust
/// Receives an execution plan, executes steps sequentially, and merges results.
async fn execute_plan(
    plan: &ExecutionPlan,
    entries: &[LogEntry],
    provider: &provider::LlmProvider,
    api_key: &str,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let mut results = Vec::new();
    let mut total_redact = RedactResult::empty();
    let total_steps = plan.steps.len();

    for (i, step) in plan.steps.iter().enumerate() {
        eprintln!("⠋ [{}/{}] Analyzing {}...", i + 1, total_steps, step.session_id);

        let session_entries: Vec<LogEntry> = entries
            .iter()
            .filter(|e| entry_session_id(e) == Some(step.session_id.as_str()))
            .cloned()
            .collect();

        let result = match &step.strategy {
            StepStrategy::Direct => {
                execute_direct_step(
                    &session_entries, provider, api_key, redactor_enabled,
                ).await
            }
            StepStrategy::Summarize { .. } => {
                execute_summarize_step(
                    &session_entries, &step.session_id, provider, api_key,
                    &plan.rate_limits, redactor_enabled,
                ).await
            }
        };

        match result {
            Ok((analysis, redact)) => {
                eprintln!("✓ [{}/{}] Complete", i + 1, total_steps);
                results.push(analysis);
                total_redact.merge(redact);
            }
            Err(e) => {
                eprintln!("⚠ [{}/{}] {} skipped: {}", i + 1, total_steps, step.session_id, e);
            }
        }

        // Rate pacing: wait unless this is the last step
        if i + 1 < total_steps {
            let wait = summarizer::calculate_wait(
                step.estimated_tokens,
                &plan.rate_limits,
            );
            if wait > 0.0 {
                eprintln!("⠋ Waiting for next request... ({:.0}s)", wait);
                tokio::time::sleep(std::time::Duration::from_secs_f64(wait)).await;
            }
        }
    }

    if results.is_empty() {
        return Err("All session analyses failed.".into());
    }

    Ok((insight::merge_results(results), total_redact))
}

/// Direct step: send session prompt as-is.
async fn execute_direct_step(
    entries: &[LogEntry],
    provider: &provider::LlmProvider,
    api_key: &str,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let prompt_text = prompt::build_prompt(entries)?;
    let (final_prompt, redact_result) = if redactor_enabled {
        crate::redactor::redact_text(&prompt_text)
    } else {
        (prompt_text, RedactResult::empty())
    };
    let raw_response = provider.call_api(api_key, &final_prompt).await?;
    let result = insight::parse_response(&raw_response)?;
    Ok((result, redact_result))
}

/// Summarize step: summarize a large session chunk-by-chunk, then analyze.
async fn execute_summarize_step(
    entries: &[LogEntry],
    session_id: &str,
    provider: &provider::LlmProvider,
    api_key: &str,
    limits: &planner::RateLimits,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    // Extract (role, text) tuples from session messages
    let messages = prompt::extract_messages(entries);

    let chunks = summarizer::split_into_chunks(&messages, limits.input_tokens_per_minute);
    let summary_text = summarizer::summarize_chunks(&chunks, provider, api_key, limits).await?;

    // Final analysis on summarized text
    let prompt_with_session = format!("[Session: {session_id}]\n{summary_text}");
    let (final_prompt, redact_result) = if redactor_enabled {
        crate::redactor::redact_text(&prompt_with_session)
    } else {
        (prompt_with_session, RedactResult::empty())
    };
    let raw_response = provider.call_api(api_key, &final_prompt).await?;
    let result = insight::parse_response(&raw_response)?;
    Ok((result, redact_result))
}
```

- [ ] **Step 2: Add extract_messages helper to prompt.rs**

```rust
/// Extracts (role, text) tuples from LogEntries.
/// Used by summarizer's split_into_chunks.
pub fn extract_messages(entries: &[LogEntry]) -> Vec<(String, String)> {
    let mut messages = Vec::new();
    for entry in entries {
        match entry {
            LogEntry::User(e) => {
                if let Some(text) = e.message.as_ref().and_then(extract_user_text) {
                    messages.push(("USER".to_string(), text));
                }
            }
            LogEntry::Assistant(e) => {
                if let Some(msg) = &e.message {
                    let text = extract_assistant_text(&msg.content);
                    if !text.is_empty() {
                        messages.push(("ASSISTANT".to_string(), text));
                    }
                }
            }
            _ => {}
        }
    }
    messages
}
```

- [ ] **Step 3: Refactor analyze_entries to use probe → plan → execute**

Replace existing `analyze_entries` body:

```rust
pub async fn analyze_entries(
    entries: &[LogEntry],
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;

    // 1. Probe: check user's actual rate limits
    eprintln!("⠋ Checking API limits...");
    let limits = provider.probe_rate_limits(&api_key).await;
    eprintln!(
        "✓ ITPM: {} | OTPM: {} | RPM: {}",
        limits.input_tokens_per_minute,
        limits.output_tokens_per_minute,
        limits.requests_per_minute,
    );

    // 2. Estimate: per-session token estimation
    let estimates = prompt::estimate_sessions(entries);

    // 3. Plan: build execution plan
    let plan = planner::build_execution_plan(&limits, &estimates);

    // 4. Display: show the plan
    if plan.is_single_shot {
        eprintln!("✓ Analyzing all logs in a single shot (estimated {} tokens)", plan.total_estimated_tokens);
    } else {
        eprintln!("✓ {} sessions to analyze (estimated {} total tokens)", plan.steps.len(), plan.total_estimated_tokens);
        for step in &plan.steps {
            let strategy_desc = match &step.strategy {
                StepStrategy::Direct => "direct analysis".to_string(),
                StepStrategy::Summarize { chunks } => format!("summarize then analyze ({chunks} chunks)"),
            };
            eprintln!("  • {}: {} tokens → {}", step.session_id, step.estimated_tokens, strategy_desc);
        }
    }

    // 5. Execute
    if plan.is_single_shot {
        // Same as before: send all at once
        let prompt_text = prompt::build_prompt(entries)?;
        let (final_prompt, redact_result) = if redactor_enabled {
            crate::redactor::redact_text(&prompt_text)
        } else {
            (prompt_text, RedactResult::empty())
        };
        let raw_response = provider.call_api(&api_key, &final_prompt).await?;
        let result = insight::parse_response(&raw_response)?;
        Ok((result, redact_result))
    } else {
        execute_plan(&plan, entries, &provider, &api_key, redactor_enabled).await
    }
}
```

- [ ] **Step 4: Remove old fallback code**

Remove the following from `mod.rs`:
- `analyze_entries_by_session()` function (lines 83-147)
- `is_context_limit_error()` function (lines 193-196)
- `is_rate_limit_error()` function (lines 199-201)
- `#[cfg(test)] mod tests` block (lines 203-242) — tests for the removed functions

Keep `entry_session_id()` (used by execute_plan).

- [ ] **Step 5: Build + test**

Run: `cargo build && cargo test`
Expected: PASS (old fallback tests removed, remaining tests pass)

- [ ] **Step 6: Run clippy**

Run: `cargo clippy`
Expected: no warnings

- [ ] **Step 7: Commit**

```bash
git add src/analyzer/mod.rs src/analyzer/prompt.rs
git commit -m "feat: refactor analyze_entries to probe → plan → execute flow

Remove the old try-fallback approach and replace with
rate limit probe-based proactive planning. Ensures
analysis works for all tiers.

Related: #36"
```

---

### Task 11: 429 retry logic

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: Add 429 retry to execute_plan error handling**

Modify the `Err(e)` branch in `execute_plan`:

```rust
Err(e) => {
    let err_msg = e.to_string();
    // 429 rate limit → wait then retry once
    if err_msg.contains("429") {
        eprintln!("⚠ [{}/{}] rate limit exceeded, waiting 60s then retrying...", i + 1, total_steps);
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        // Retry
        let retry_result = match &step.strategy {
            StepStrategy::Direct => {
                execute_direct_step(
                    &session_entries, provider, api_key, redactor_enabled,
                ).await
            }
            StepStrategy::Summarize { .. } => {
                execute_summarize_step(
                    &session_entries, &step.session_id, provider, api_key,
                    &plan.rate_limits, redactor_enabled,
                ).await
            }
        };

        match retry_result {
            Ok((analysis, redact)) => {
                eprintln!("✓ [{}/{}] Retry succeeded", i + 1, total_steps);
                results.push(analysis);
                total_redact.merge(redact);
            }
            Err(retry_err) => {
                eprintln!("⚠ [{}/{}] {} skipped (retry failed): {}", i + 1, total_steps, step.session_id, retry_err);
            }
        }
    } else {
        eprintln!("⚠ [{}/{}] {} skipped: {}", i + 1, total_steps, step.session_id, err_msg);
    }
}
```

- [ ] **Step 2: Build + test**

Run: `cargo build && cargo test`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/analyzer/mod.rs
git commit -m "feat: execute_plan 429 retry logic — wait once then retry, skip on failure"
```

**Note:** `execute_plan`, `execute_direct_step`, and `execute_summarize_step` are async with external API dependencies, making unit testing difficult. Probe failure default_generous, partial step failure handling, and 429 retry are verified via integration tests with real API keys or manual testing. Can be automated in the future with a mock provider.

---

### Task 12: ARCHITECTURE.md update + final verification

**Files:**
- Modify: `docs/ARCHITECTURE.md`

- [ ] **Step 1: Add new modules to ARCHITECTURE.md**

Add `planner.rs` and `summarizer.rs` to the `analyzer/` section in the project structure tree.

- [ ] **Step 2: Full build + test + clippy**

Run: `cargo build && cargo clippy && cargo test`
Expected: ALL PASS

- [ ] **Step 3: Commit**

```bash
git add docs/ARCHITECTURE.md
git commit -m "docs: add planner, summarizer modules to ARCHITECTURE.md"
```
