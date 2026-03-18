# 동적 max_tokens Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** LLM 출력 토큰을 세션 수에 비례하여 동적으로 계산하고, 모델 한도 초과 시 multi-step으로 자동 전환한다.

**Architecture:** planner에 출력 토큰 추정 로직을 추가하여 single-shot 판단 시 출력 제한도 함께 고려한다. call_api의 하드코딩 max_tokens를 제거하고 동적 값을 전달한다.

**Tech Stack:** Rust, 기존 planner/provider/anthropic/openai 모듈

**Spec:** `docs/superpowers/specs/2026-03-18-dynamic-max-tokens-design.md`

---

## File Map

| 파일 | 역할 | 변경 |
|------|------|------|
| `src/analyzer/planner.rs` | 실행 계획 수립 | 출력 추정 함수 + single-shot 조건 확장 + `recommended_max_tokens` 필드 |
| `src/analyzer/provider.rs` | 프로바이더 추상화 | `max_output_tokens()` 메서드 + `call_api()` 시그니처 변경 |
| `src/analyzer/anthropic.rs` | Anthropic API | `call_anthropic_api()`에 `max_tokens` 파라미터 추가 |
| `src/analyzer/openai.rs` | OpenAI API | `call_openai_api()`에 `max_tokens` 파라미터 추가 |
| `src/analyzer/mod.rs` | 분석 오케스트레이션 | 동적 max_tokens를 planner에서 받아 call_api에 전달 |

---

## Chunk 1: 플래너 출력 추정 + 조건 확장

### Task 1: planner.rs — 출력 추정 상수 및 함수

**Files:**
- Modify: `src/analyzer/planner.rs`

- [ ] **Step 1: 테스트 작성 — 출력 초과 시 multi-step 전환**

`planner.rs`의 `#[cfg(test)] mod tests` 안에 추가:

```rust
#[test]
fn test_single_shot_blocked_by_output_limit() {
    // 입력은 ITPM 이내지만 세션 22개 → 출력 추정 42900 > 32000
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
    // 2세션 × 1500 × 1.3 = 3900 < 32000 → single-shot
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
    // 10세션 × 1500 × 1.3 = 19500
    assert_eq!(plan.recommended_max_tokens, 19_500);
}
```

- [ ] **Step 2: 테스트 실행 — 실패 확인**

Run: `cargo test --lib planner::tests -- --nocapture 2>&1 | tail -10`
Expected: 컴파일 에러 (build_execution_plan 시그니처 변경 필요)

- [ ] **Step 3: planner.rs 수정 — 출력 추정 + 조건 확장**

`planner.rs`에 상수 추가:

```rust
/// 세션당 예상 출력 토큰.
/// 16세션 분석 시 16384 토큰 초과 실측 → 세션당 ~1000+, 보수적 1500.
const OUTPUT_TOKENS_PER_SESSION: u64 = 1_500;

/// 출력 여유 비율 (30%).
const OUTPUT_MARGIN: f64 = 1.3;
```

`ExecutionPlan`에 필드 추가:

```rust
pub struct ExecutionPlan {
    pub rate_limits: RateLimits,
    pub steps: Vec<ExecutionStep>,
    pub total_estimated_tokens: u64,
    pub is_single_shot: bool,
    pub recommended_max_tokens: u64,  // 추가
}
```

`build_execution_plan` 시그니처에 `model_max_output: u64` 파라미터 추가:

```rust
pub fn build_execution_plan(
    limits: &RateLimits,
    estimates: &[SessionEstimate],
    model_max_output: u64,
) -> ExecutionPlan {
    let itpm = limits.input_tokens_per_minute;
    let total: u64 = estimates.iter().map(|e| e.estimated_tokens).sum();
    let num_sessions = estimates.len() as u64;

    // 출력 토큰 추정
    let estimated_output = (num_sessions * OUTPUT_TOKENS_PER_SESSION) as f64 * OUTPUT_MARGIN;
    let recommended_max_tokens = (estimated_output as u64).min(model_max_output);

    // single-shot 조건: 입력 AND 출력 모두 한도 이내
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

    // 세션별로 전략 결정 (기존 로직 유지)
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

- [ ] **Step 4: 기존 테스트 수정 — 3번째 인자 추가**

기존 6개 테스트의 `build_execution_plan(&limits, &estimates)` 호출에 `, 32_000` 추가.

- [ ] **Step 5: 테스트 실행 — 통과 확인**

Run: `cargo test --lib planner::tests -- --nocapture 2>&1 | tail -10`
Expected: 모든 테스트 PASS

- [ ] **Step 6: 커밋**

```bash
git add src/analyzer/planner.rs
git commit -m "feat: planner에 출력 토큰 추정 + single-shot 출력 제한 조건 추가"
```

---

## Chunk 2: provider + API 함수 시그니처 변경

### Task 2: provider.rs — max_output_tokens + call_api 변경

**Files:**
- Modify: `src/analyzer/provider.rs`

- [ ] **Step 1: `max_output_tokens()` 메서드 추가**

`impl LlmProvider` 블록에 추가:

```rust
/// 모델별 최대 출력 토큰.
/// 모델 스펙이며 tier/사용자와 무관한 상수.
pub fn max_output_tokens(&self) -> u64 {
    match self {
        LlmProvider::Anthropic => 32_000,
        LlmProvider::OpenAi => 16_384,
    }
}
```

- [ ] **Step 2: `call_api()` 시그니처에 `max_tokens` 추가**

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

`call_summary_api`도 동일하게 `max_tokens: u32` 파라미터 추가 (기본값 16384 사용).

- [ ] **Step 3: cargo check — 컴파일 에러 확인 (anthropic/openai 미수정 상태)**

Run: `cargo check 2>&1 | tail -5`
Expected: anthropic.rs/openai.rs에서 인자 수 불일치 에러

### Task 3: anthropic.rs — max_tokens 파라미터화

**Files:**
- Modify: `src/analyzer/anthropic.rs`

- [ ] **Step 1: `call_anthropic_api()` 시그니처 변경**

```rust
pub async fn call_anthropic_api(
    api_key: &str,
    system_prompt: &str,
    conversation_text: &str,
    max_tokens: u32,  // 추가
) -> Result<String, super::AnalyzerError> {
```

함수 본문의 `max_tokens: 16384`를 `max_tokens`로 변경:

```rust
let request_body = ApiRequest {
    model: MODEL.to_string(),
    max_tokens,  // 하드코딩 제거
    system: system_prompt.to_string(),
    // ...
};
```

### Task 4: openai.rs — max_tokens 파라미터화

**Files:**
- Modify: `src/analyzer/openai.rs`

- [ ] **Step 1: `call_openai_api()` 시그니처 변경**

anthropic.rs와 동일한 패턴. `max_tokens: 16384` → `max_tokens` 파라미터.

- [ ] **Step 2: cargo check — 컴파일 통과 확인**

Run: `cargo check 2>&1 | tail -5`
Expected: `mod.rs`에서 call_api 호출부 인자 수 불일치 에러 (아직 미수정)

- [ ] **Step 3: 커밋**

```bash
git add src/analyzer/provider.rs src/analyzer/anthropic.rs src/analyzer/openai.rs
git commit -m "feat: call_api에 동적 max_tokens 파라미터 추가"
```

---

## Chunk 3: mod.rs — 동적 max_tokens 전달 + 통합

### Task 5: mod.rs — 호출부 수정

**Files:**
- Modify: `src/analyzer/mod.rs`

- [ ] **Step 1: `analyze_entries()`에서 planner 호출 수정**

`build_execution_plan` 호출에 `model_max_output` 전달:

```rust
let plan = planner::build_execution_plan(
    &limits,
    &estimates,
    provider.max_output_tokens(),
);
```

- [ ] **Step 2: single-shot `call_api()` 호출에 동적 max_tokens 전달**

```rust
let raw_response = provider.call_api(
    &api_key,
    &final_prompt,
    plan.recommended_max_tokens as u32,
).await?;
```

- [ ] **Step 3: `analyze_codex_entries()`의 `call_api()` 호출 수정**

Codex는 세션 1개씩 호출하므로 고정값 사용:

```rust
let max_tokens = (1500_f64 * 1.3) as u32; // 1세션 = 1950
let raw_response = provider.call_api(&api_key, &final_prompt, max_tokens).await?;
```

- [ ] **Step 4: `execute_plan()`의 `call_api()` 호출 수정**

multi-step에서는 세션 1개씩이므로 동일하게 1세션 기준:

```rust
// execute_direct_step, execute_summarize_step 내부의 call_api 호출에
// max_tokens: (1500_f64 * 1.3) as u32 전달
```

- [ ] **Step 5: `call_summary_api()` 호출부 수정**

summary는 짧으므로 16384 고정:

```rust
provider.call_summary_api(&api_key, session_summaries, 16384).await?
```

- [ ] **Step 6: cargo clippy + cargo test — 전체 통과 확인**

Run: `cargo clippy 2>&1 | tail -5 && cargo test 2>&1 | tail -5`
Expected: 경고 없음, 91+ 테스트 PASS

- [ ] **Step 7: 커밋**

```bash
git add src/analyzer/mod.rs
git commit -m "feat: 동적 max_tokens를 planner에서 계산하여 API 호출에 전달"
```

---

## Chunk 4: 통합 테스트 + 정리

### Task 6: 수동 검증

- [ ] **Step 1: `cargo run -- today` 실행**

16개 이상 세션이 있는 날에 실행하여 JSON 파싱 에러가 발생하지 않는지 확인.

- [ ] **Step 2: plan 출력에 추정 출력 토큰 표시 (선택)**

현재 입력 토큰만 표시하는데, 출력 추정도 표시하면 디버깅에 도움됨:

```
✓ 전체 로그를 한 번에 분석합니다 (추정 입력 318247토큰, 출력 max 19500토큰)
```

- [ ] **Step 3: 최종 커밋 + PR**

```bash
git push origin dev
```
