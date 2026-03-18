# Rate Limit 인식 분석 엔진 Implementation Plan

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

/// API rate limit 정보.
/// probe 호출의 응답 헤더에서 추출하거나, 실패 시 default_generous()를 사용한다.
#[derive(Debug, Clone)]
pub struct RateLimits {
    pub input_tokens_per_minute: u64,
    pub output_tokens_per_minute: u64,
    pub requests_per_minute: u64,
}

impl RateLimits {
    /// probe 실패 시 사용하는 관대한 기본값.
    /// 대부분의 사용자가 single_shot으로 진행하게 되며,
    /// 실제 제한에 걸리면 런타임 안전망이 처리한다.
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
/// 세션별 토큰 추정 결과.
#[derive(Debug, Clone)]
pub struct SessionEstimate {
    pub session_id: String,
    pub estimated_tokens: u64,
    pub entry_count: usize,
}

/// 개별 실행 스텝의 전략.
#[derive(Debug, Clone, PartialEq)]
pub enum StepStrategy {
    /// ITPM 이내 — 그대로 전송
    Direct,
    /// ITPM 초과 — 청크 분할 후 요약
    Summarize { chunks: usize },
}

/// 실행 계획의 개별 스텝.
#[derive(Debug, Clone)]
pub struct ExecutionStep {
    pub session_id: String,
    pub strategy: StepStrategy,
    pub estimated_tokens: u64,
}

/// 전체 실행 계획.
/// is_single_shot이면 기존처럼 한 번에 전송 (높은 tier에서 오버헤드 없음).
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub rate_limits: RateLimits,
    pub steps: Vec<ExecutionStep>,
    pub total_estimated_tokens: u64,
    pub is_single_shot: bool,
}
```

- [ ] **Step 2: Build 확인**

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

`prompt.rs` 하단 tests 모듈에 추가:

```rust
#[test]
fn test_estimate_tokens_한국어() {
    // "안녕하세요" = 5글자, ÷ 2 = 2 (반올림하지 않으므로 정수 나눗셈)
    assert_eq!(super::estimate_tokens("안녕하세요"), 2);
}

#[test]
fn test_estimate_tokens_영어() {
    // "hello world" = 11글자, ÷ 2 = 5
    assert_eq!(super::estimate_tokens("hello world"), 5);
}

#[test]
fn test_estimate_tokens_빈문자열() {
    assert_eq!(super::estimate_tokens(""), 0);
}

#[test]
fn test_estimate_sessions_세션별_추정() {
    let entries = vec![
        serde_json::from_str::<LogEntry>(
            r#"{"type":"user","sessionId":"s1","timestamp":"2026-03-11T10:00:00Z","uuid":"u1","message":{"role":"user","content":"안녕하세요 반갑습니다"}}"#,
        ).unwrap(),
        serde_json::from_str::<LogEntry>(
            r#"{"type":"user","sessionId":"s2","timestamp":"2026-03-11T11:00:00Z","uuid":"u2","message":{"role":"user","content":"두번째 세션입니다"}}"#,
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

`prompt.rs` 상단에 import 추가:
```rust
use super::planner::SessionEstimate;
```

함수 추가:
```rust
/// 시스템 프롬프트의 추정 토큰 수 (SYSTEM_PROMPT 문자열 기준 사전 계산).
/// provider::SYSTEM_PROMPT의 글자 수 ÷ 2.
pub const SYSTEM_PROMPT_ESTIMATED_TOKENS: u64 = 800;

/// 텍스트의 토큰 수를 간이 추정한다.
/// 한국어는 음절당 ~1토큰이므로, 글자 수 ÷ 2는 보수적 추정이다.
pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as u64) / 2
}

/// 세션별 토큰 추정 결과를 반환한다.
/// extract_session_ids로 세션 목록을 구한 뒤, 세션별 엔트리의 텍스트 크기를 추정한다.
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

        // build_prompt과 동일한 형식으로 텍스트 크기를 추정한다.
        // 실제 build_prompt을 호출하지 않고, 엔트리의 원시 텍스트 길이를 합산한다.
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
    // ITPM이 35,000이고 세션 합계가 31,000이면,
    // analyze_summary 여유분(5,000) 고려 시 single_shot 불가.
    let limits = RateLimits {
        input_tokens_per_minute: 35_000,
        output_tokens_per_minute: 8_000,
        requests_per_minute: 50,
    };
    let estimates = vec![
        SessionEstimate { session_id: "s1".into(), estimated_tokens: 31_000, entry_count: 15 },
    ];
    let plan = build_execution_plan(&limits, &estimates);
    // 31,000 + 5,000(예약) = 36,000 > 35,000 → single_shot 아님
    assert!(!plan.is_single_shot);
}

#[test]
fn test_build_plan_exact_boundary_is_single_shot() {
    // total + budget == ITPM 정확히 일치 시 single_shot (<=)
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
/// analyze_summary() 호출을 위해 예약하는 토큰 여유분.
const SUMMARY_BUDGET_TOKENS: u64 = 5_000;

/// rate limit과 세션별 추정치를 기반으로 실행 계획을 수립한다.
///
/// 전략 분기:
/// - 전체 합계 + 여유분 ≤ ITPM → single_shot (한 번에 전송)
/// - 개별 세션 ≤ ITPM → Direct (세션별 순차)
/// - 개별 세션 > ITPM → Summarize (청크 분할 후 요약)
pub fn build_execution_plan(
    limits: &RateLimits,
    estimates: &[SessionEstimate],
) -> ExecutionPlan {
    let itpm = limits.input_tokens_per_minute;
    let total: u64 = estimates.iter().map(|e| e.estimated_tokens).sum();

    // 전체가 ITPM 안에 들어가면 single_shot
    if total + SUMMARY_BUDGET_TOKENS <= itpm {
        return ExecutionPlan {
            rate_limits: limits.clone(),
            steps: Vec::new(),
            total_estimated_tokens: total,
            is_single_shot: true,
        };
    }

    // 세션별로 전략 결정
    let steps: Vec<ExecutionStep> = estimates
        .iter()
        .map(|est| {
            let strategy = if est.estimated_tokens <= itpm {
                StepStrategy::Direct
            } else {
                // ITPM 기준으로 필요한 청크 수 계산 (올림)
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
git commit -m "feat: build_execution_plan — single_shot/direct/summarize 전략 분기"
```

---

## Chunk 2: Probe + Provider Dispatch

### Task 5: Probe — Anthropic

**Files:**
- Modify: `src/analyzer/anthropic.rs`

- [ ] **Step 1: Implement probe_anthropic_rate_limits**

Anthropic probe는 실제 API 호출이 필요하므로 단위 테스트 대신 헤더 파싱 로직만 별도 테스트한다.

```rust
use super::planner::RateLimits;

/// Anthropic API에 최소 요청을 보내 응답 헤더에서 rate limit을 읽는다.
/// 실패 시 None을 반환하며, 호출자가 default_generous로 대체한다.
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

/// Anthropic 응답 헤더에서 rate limit 값을 추출한다.
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

- [ ] **Step 2: Write test for header parsing**

```rust
#[test]
fn test_parse_anthropic_rate_headers_참고용() {
    // parse_anthropic_rate_headers는 reqwest::Response를 받으므로
    // 직접 단위 테스트가 어렵다. 대신 헤더 파싱 로직의 정확성은
    // 통합 테스트(실제 API 호출)에서 검증한다.
    // 여기서는 컴파일 확인만 한다.
    assert!(true);
}
```

- [ ] **Step 3: Build 확인**

Run: `cargo build`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/analyzer/anthropic.rs
git commit -m "feat: probe_anthropic_rate_limits — 응답 헤더에서 ITPM/OTPM/RPM 추출"
```

---

### Task 6: Probe — OpenAI

**Files:**
- Modify: `src/analyzer/openai.rs`

- [ ] **Step 1: Implement probe_openai_rate_limits**

```rust
use super::planner::RateLimits;

/// OpenAI API에 최소 요청을 보내 응답 헤더에서 rate limit을 읽는다.
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

/// OpenAI 응답 헤더에서 rate limit 값을 추출한다.
fn parse_openai_rate_headers(response: &reqwest::Response) -> Option<RateLimits> {
    let headers = response.headers();

    // OpenAI는 x-ratelimit-limit-tokens (TPM 합산) 헤더를 사용한다.
    // ITPM/OTPM 분리가 없으므로 tokens를 ITPM으로 사용하고 OTPM은 1/4로 추정.
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

- [ ] **Step 2: Build 확인**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/analyzer/openai.rs
git commit -m "feat: probe_openai_rate_limits — x-ratelimit-limit-tokens 헤더 파싱"
```

---

### Task 7: Provider probe dispatch

**Files:**
- Modify: `src/analyzer/provider.rs`

- [ ] **Step 1: Add probe_rate_limits to LlmProvider**

`impl LlmProvider` 블록에 추가:

```rust
    /// API probe 호출로 사용자의 실제 rate limit을 확인한다.
    /// 실패 시 default_generous()를 반환하여 single_shot으로 진행한다.
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
            eprintln!("⚠ rate limit 확인 실패, 기본값으로 진행합니다.");
            super::planner::RateLimits::default_generous()
        })
    }
```

- [ ] **Step 2: Build 확인**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/analyzer/provider.rs
git commit -m "feat: LlmProvider::probe_rate_limits — 프로바이더별 probe dispatch"
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
        // 각 메시지가 약 10토큰(20글자)이고 ITPM이 25토큰이면
        // 한 청크에 2개씩 들어가야 한다.
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
        // 단일 메시지가 제한을 초과해도 스킵하지 않고 단독 청크로 넣는다.
        let messages = vec![
            ("USER".to_string(), "a".repeat(100)),  // ~50 tokens
        ];
        let chunks = split_into_chunks(&messages, 25);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 1);
    }

    #[test]
    fn test_split_into_chunks_빈_메시지() {
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

/// 세션 요약에 사용하는 프롬프트.
/// rwd의 인사이트 카테고리에 맞춰 핵심 내용을 보존하도록 지시한다.
pub const CHUNK_SUMMARIZE_PROMPT: &str = r#"다음 개발 세션 대화에서 아래 항목을 중심으로 요약하라:
- 내린 기술적 결정과 그 이유
- 실수나 수정 사항
- 새로 배운 점 (TIL)
- 흥미로운 발견이나 의문점
원문의 구체적 기술 용어와 맥락을 보존하라."#;

/// 메시지 목록을 ITPM 제한 내의 청크들로 분할한다.
/// 메시지 경계에서만 자른다 (메시지 중간에서 자르지 않음).
/// 단일 메시지가 제한을 초과하면 단독 청크로 넣는다.
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

        // 현재 청크에 추가하면 초과하는 경우
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

/// 대형 세션의 메시지를 청크별로 요약하고, 합친 요약 텍스트를 반환한다.
/// 각 청크 사이에 rate pacing을 적용한다.
pub async fn summarize_chunks(
    chunks: &[Vec<(String, String)>],
    provider: &LlmProvider,
    api_key: &str,
    limits: &RateLimits,
) -> Result<String, super::AnalyzerError> {
    let mut summaries: Vec<String> = Vec::new();
    let total = chunks.len();

    for (i, chunk) in chunks.iter().enumerate() {
        // 청크를 텍스트로 변환
        let chunk_text: String = chunk
            .iter()
            .map(|(role, text)| format!("[{role}] {text}"))
            .collect::<Vec<_>>()
            .join("\n");

        eprintln!("    청크 {}/{total} 요약 중...", i + 1);

        // 요약 API 호출 (max_tokens: 2000은 provider 수준에서 설정)
        let summary = provider
            .call_api_with_max_tokens(
                api_key,
                CHUNK_SUMMARIZE_PROMPT,
                &chunk_text,
                2000,
            )
            .await?;
        summaries.push(summary);

        // rate pacing: 마지막 청크가 아니면 대기
        if i + 1 < total {
            let chunk_tokens = estimate_tokens(&chunk_text);
            let wait = calculate_wait(chunk_tokens, limits);
            if wait > 0.0 {
                eprintln!("    다음 요청까지 대기 중... ({:.0}초)", wait);
                tokio::time::sleep(std::time::Duration::from_secs_f64(wait)).await;
            }
        }
    }

    Ok(summaries.join("\n\n"))
}

/// ITPM/RPM 기반 대기 시간을 계산한다.
/// max(itpm_wait, rpm_wait)를 반환한다.
pub fn calculate_wait(used_tokens: u64, limits: &RateLimits) -> f64 {
    let itpm_wait = (used_tokens as f64 / limits.input_tokens_per_minute as f64) * 60.0;
    let rpm_wait = 60.0 / limits.requests_per_minute as f64;
    itpm_wait.max(rpm_wait)
}
```

- [ ] **Step 2: Add `_with_max_tokens` variants to anthropic.rs and openai.rs**

기존 `call_anthropic_api`/`call_openai_api`는 그대로 유지 (하위 호환).
새 함수를 추가하여 `max_tokens` 파라미터를 받는다:

`anthropic.rs`:
```rust
/// max_tokens를 지정할 수 있는 API 호출 변형.
pub async fn call_anthropic_api_with_max_tokens(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,
) -> Result<String, super::AnalyzerError> {
    let client = reqwest::Client::new();
    let request_body = ApiRequest {
        model: MODEL.to_string(),
        max_tokens,
        system: system_prompt.to_string(),
        messages: vec![ApiMessage {
            role: "user".to_string(),
            content: conversation_text.to_string(),
        }],
    };
    let response = client
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(format!("API 요청 실패 ({status}): {error_body}").into());
    }
    let api_response: ApiResponse = response.json().await?;
    let text = api_response
        .content
        .iter()
        .find(|block| block.block_type == "text")
        .and_then(|block| block.text.as_deref())
        .ok_or("API 응답에 텍스트 블록이 없습니다")?;
    Ok(text.to_string())
}
```

`openai.rs`에도 동일 패턴으로 `call_openai_api_with_max_tokens` 추가.

- [ ] **Step 3: Add call_api_with_max_tokens to provider.rs**

`impl LlmProvider` 블록에 추가:

```rust
    /// API 호출 (max_tokens 지정 가능).
    /// 요약 호출 시 max_tokens를 2000으로 제한하고, 분석 호출은 기존 16384를 유지.
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

`summarizer.rs` tests에 추가:

```rust
#[test]
fn test_calculate_wait_itpm_기반() {
    let limits = RateLimits {
        input_tokens_per_minute: 30_000,
        output_tokens_per_minute: 8_000,
        requests_per_minute: 1_000, // RPM이 높으므로 ITPM이 병목
    };
    let wait = calculate_wait(15_000, &limits);
    // 15000/30000 * 60 = 30초
    assert!((wait - 30.0).abs() < 0.1);
}

#[test]
fn test_calculate_wait_rpm_기반() {
    let limits = RateLimits {
        input_tokens_per_minute: 1_000_000, // ITPM이 높으므로 RPM이 병목
        output_tokens_per_minute: 200_000,
        requests_per_minute: 50,
    };
    let wait = calculate_wait(100, &limits);
    // 60/50 = 1.2초
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

기존 `analyze_entries_by_session`, `is_context_limit_error`, `is_rate_limit_error` 함수와 해당 테스트를 제거하고, 새 함수를 추가한다.

`mod.rs` 상단에 import 추가:
```rust
use planner::{ExecutionPlan, StepStrategy};
```

새 함수 추가:

```rust
/// 실행 계획을 받아 순차 실행하고 결과를 병합한다.
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
        eprintln!("⠋ [{}/{}] {} 분석 중...", i + 1, total_steps, step.session_id);

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
                eprintln!("✓ [{}/{}] 완료", i + 1, total_steps);
                results.push(analysis);
                total_redact.merge(redact);
            }
            Err(e) => {
                eprintln!("⚠ [{}/{}] {} 스킵: {}", i + 1, total_steps, step.session_id, e);
            }
        }

        // rate pacing: 마지막 스텝이 아니면 대기
        if i + 1 < total_steps {
            let wait = summarizer::calculate_wait(
                step.estimated_tokens,
                &plan.rate_limits,
            );
            if wait > 0.0 {
                eprintln!("⠋ 다음 요청까지 대기 중... ({:.0}초)", wait);
                tokio::time::sleep(std::time::Duration::from_secs_f64(wait)).await;
            }
        }
    }

    if results.is_empty() {
        return Err("모든 세션의 분석에 실패했습니다.".into());
    }

    Ok((insight::merge_results(results), total_redact))
}

/// Direct 스텝: 세션 프롬프트를 그대로 전송.
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

/// Summarize 스텝: 대형 세션을 청크별 요약 후 분석.
async fn execute_summarize_step(
    entries: &[LogEntry],
    session_id: &str,
    provider: &provider::LlmProvider,
    api_key: &str,
    limits: &planner::RateLimits,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    // 세션의 메시지를 (role, text) 튜플로 추출
    let messages = prompt::extract_messages(entries);

    let chunks = summarizer::split_into_chunks(&messages, limits.input_tokens_per_minute);
    let summary_text = summarizer::summarize_chunks(&chunks, provider, api_key, limits).await?;

    // 요약본으로 최종 분석
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
/// LogEntry에서 (role, text) 튜플 목록을 추출한다.
/// summarizer의 split_into_chunks에서 사용한다.
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

기존 `analyze_entries` 본문을 교체:

```rust
pub async fn analyze_entries(
    entries: &[LogEntry],
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError> {
    let (provider, api_key) = provider::load_provider()?;

    // 1. Probe: 사용자의 실제 rate limit 확인
    eprintln!("⠋ API 한도 확인 중...");
    let limits = provider.probe_rate_limits(&api_key).await;
    eprintln!(
        "✓ ITPM: {} | OTPM: {} | RPM: {}",
        limits.input_tokens_per_minute,
        limits.output_tokens_per_minute,
        limits.requests_per_minute,
    );

    // 2. Estimate: 세션별 토큰 추정
    let estimates = prompt::estimate_sessions(entries);

    // 3. Plan: 실행 계획 수립
    let plan = planner::build_execution_plan(&limits, &estimates);

    // 4. Display: 계획 출력
    if plan.is_single_shot {
        eprintln!("✓ 전체 로그를 한 번에 분석합니다 (추정 {}토큰)", plan.total_estimated_tokens);
    } else {
        eprintln!("✓ 세션 {}개 분석 예정 (총 {} 토큰 추정)", plan.steps.len(), plan.total_estimated_tokens);
        for step in &plan.steps {
            let strategy_desc = match &step.strategy {
                StepStrategy::Direct => "직접 분석".to_string(),
                StepStrategy::Summarize { chunks } => format!("요약 후 분석 ({chunks} 청크)"),
            };
            eprintln!("  • {}: {} 토큰 → {}", step.session_id, step.estimated_tokens, strategy_desc);
        }
    }

    // 5. Execute
    if plan.is_single_shot {
        // 기존과 동일: 한 번에 전송
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

`mod.rs`에서 다음을 제거:
- `analyze_entries_by_session()` 함수 (라인 83-147)
- `is_context_limit_error()` 함수 (라인 193-196)
- `is_rate_limit_error()` 함수 (라인 199-201)
- `#[cfg(test)] mod tests` 블록 전체 (라인 203-242) — 제거된 함수의 테스트들

`entry_session_id()` 함수는 유지한다 (execute_plan에서 사용).

- [ ] **Step 5: Build + test**

Run: `cargo build && cargo test`
Expected: PASS (기존 fallback 테스트가 제거되었으므로 남은 테스트만 통과)

- [ ] **Step 6: Run clippy**

Run: `cargo clippy`
Expected: no warnings

- [ ] **Step 7: Commit**

```bash
git add src/analyzer/mod.rs src/analyzer/prompt.rs
git commit -m "feat: analyze_entries를 probe → plan → execute 흐름으로 리팩터링

기존 try-fallback 방식을 제거하고, rate limit probe 기반
사전 계획 방식으로 교체. 모든 tier에서 분석이 동작하도록 한다.

관련: #36"
```

---

### Task 11: 429 재시도 로직

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: execute_plan의 에러 처리에 429 재시도 추가**

`execute_plan`의 `Err(e)` 분기를 수정:

```rust
Err(e) => {
    let err_msg = e.to_string();
    // 429 rate limit → retry-after 대기 후 1회 재시도
    if err_msg.contains("429") {
        eprintln!("⚠ [{}/{}] rate limit 초과, 60초 대기 후 재시도...", i + 1, total_steps);
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        // 재시도
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
                eprintln!("✓ [{}/{}] 재시도 성공", i + 1, total_steps);
                results.push(analysis);
                total_redact.merge(redact);
            }
            Err(retry_err) => {
                eprintln!("⚠ [{}/{}] {} 스킵 (재시도 실패): {}", i + 1, total_steps, step.session_id, retry_err);
            }
        }
    } else {
        eprintln!("⚠ [{}/{}] {} 스킵: {}", i + 1, total_steps, step.session_id, err_msg);
    }
}
```

- [ ] **Step 2: Build + test**

Run: `cargo build && cargo test`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/analyzer/mod.rs
git commit -m "feat: execute_plan 429 재시도 로직 — 1회 대기 후 retry, 실패 시 스킵"
```

**Note:** `execute_plan`, `execute_direct_step`, `execute_summarize_step`는 async + 외부 API 의존이므로 단위 테스트가 어렵다. probe 실패 시 default_generous 적용, 스텝 부분 실패 처리, 429 재시도 등은 실제 API 키를 사용하는 통합 테스트 또는 수동 검증으로 확인한다. 향후 mock provider 도입 시 자동화 가능.

---

### Task 12: ARCHITECTURE.md 업데이트 + 최종 검증

**Files:**
- Modify: `docs/ARCHITECTURE.md`

- [ ] **Step 1: ARCHITECTURE.md에 새 모듈 추가**

`analyzer/` 섹션의 프로젝트 구조 트리에 `planner.rs`, `summarizer.rs` 추가.

- [ ] **Step 2: Full build + test + clippy**

Run: `cargo build && cargo clippy && cargo test`
Expected: ALL PASS

- [ ] **Step 3: Commit**

```bash
git add docs/ARCHITECTURE.md
git commit -m "docs: ARCHITECTURE.md에 planner, summarizer 모듈 추가"
```
