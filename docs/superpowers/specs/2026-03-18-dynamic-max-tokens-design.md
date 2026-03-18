# 동적 max_tokens + 출력 초과 시 자동 분할

## 문제

- `rwd today`에서 세션 16개를 single-shot으로 분석 시 LLM 응답 JSON이 `max_tokens: 16384`에서 잘림
- 잘린 JSON → 파싱 실패 → 전체 분석 중단
- `claude-opus-4-6`의 실제 max output은 32,000인데 절반만 사용 중이었음

## 설계

### 1. 모델별 max output 상수

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

### 2. 출력 토큰 추정

세션 수 기반으로 출력 크기를 추정한다.

```
OUTPUT_TOKENS_PER_SESSION = 1500
estimated_output = num_sessions × 1500 × 1.3 (30% 여유)
```

1500 근거: 오늘 16세션 분석 시 16384 토큰 초과 → 세션당 ~1000+. 보수적으로 1500.

### 3. 동적 max_tokens 계산

```
dynamic_max_tokens = min(estimated_output, model_max_output)
```

`call_api()`에 하드코딩된 16384 대신 동적 값을 전달.

### 4. 플래너 확장 — 출력 초과 시 multi-step 전환

`build_execution_plan()`의 single-shot 판단 조건에 출력 조건 추가:

```rust
// 기존
if total_input + SUMMARY_BUDGET <= itpm { single_shot }

// 변경
if total_input + SUMMARY_BUDGET <= itpm
   && estimated_output <= model_max_output { single_shot }
```

출력이 초과하면 세션을 그룹으로 나눠 기존 multi-step 파이프라인으로 처리.

그룹 크기: `model_max_output / (OUTPUT_TOKENS_PER_SESSION × 1.3)` 세션씩 묶음.

### 5. call_api 시그니처 변경

```rust
// 기존: max_tokens 하드코딩
pub async fn call_api(&self, api_key: &str, prompt: &str) -> Result<String, Error>

// 변경: max_tokens 파라미터 추가
pub async fn call_api(&self, api_key: &str, prompt: &str, max_tokens: u32) -> Result<String, Error>
```

### 변경 파일

| 파일 | 변경 |
|------|------|
| `planner.rs` | 출력 추정 + single-shot 조건에 출력 제한 추가 + `num_sessions` 파라미터 |
| `provider.rs` | `max_output_tokens()` 메서드, `call_api()` 시그니처 변경 |
| `anthropic.rs` | `call_anthropic_api()`에 `max_tokens` 파라미터 추가 |
| `openai.rs` | `call_openai_api()`에 `max_tokens` 파라미터 추가 |
| `mod.rs` | 동적 max_tokens 계산 + call_api 호출부 수정 |

### 테스트

- `test_single_shot_blocked_by_output_limit` — 입력은 ITPM 이내지만 세션이 많아 출력 초과 → multi-step
- `test_dynamic_max_tokens_calculation` — 세션 수에 따른 max_tokens 계산 검증
- `test_single_shot_with_few_sessions` — 세션 적으면 기존처럼 single-shot
