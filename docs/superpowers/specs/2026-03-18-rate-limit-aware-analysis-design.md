# Design: Rate Limit 인식 분석 엔진

## 배경

이전 설계(token-limit-fallback)는 "시도 → 실패 → fallback" 방식이었다. 그러나 실제 tier별 ITPM 차이가 극단적이다:

| Tier | Claude Sonnet ITPM | 비고 |
|------|-------------------|------|
| Tier 1 | 30,000 | 개인 사용자 대부분 |
| Tier 4 | 2,000,000 | 67배 차이 |

Tier 1 사용자는 429 에러만 받고 사실상 rwd를 사용할 수 없었다. "에러 대신 동작"이 이번 설계의 핵심이다.

## 이전 설계와의 관계

`2026-03-18-token-limit-fallback-design.md`의 try-fallback 방식을 **완전히 교체**한다. 기존의 400/429 에러 판별 로직, `analyze_entries_by_session()` fallback 함수를 제거하고, probe 기반 사전 계획 방식으로 대체한다.

## 설계 원칙

- 모든 tier에서 동작해야 한다
- 높은 tier 사용자에게 불필요한 오버헤드를 주지 않는다
- 사용자에게 진행 상황을 투명하게 보여준다

## 아키텍처 개요

```
analyze_entries(entries)
  → probe_rate_limits(provider, api_key) → RateLimits
  → estimate_sessions(entries) → Vec<SessionEstimate>
  → build_execution_plan(rate_limits, estimates) → ExecutionPlan
  → display_plan(plan)
  → execute_plan(plan, provider, api_key)
      → is_single_shot: 한 번에 전송 (기존과 동일)
      → 아닐 경우: 스텝별 순차 실행
          → Direct: 세션 프롬프트 → API 호출
          → Summarize: 청크 분할 → 요약 → 합치기 → 분석
          → 스텝 간 rate pacing
  → merge_results → 최종 결과
```

## 섹션 1: Probe 모듈

### 목적

API 호출 전에 사용자의 실제 rate limit을 파악한다.

### 동작

- 최소한의 메시지("ping")로 API 호출
- 응답 헤더에서 rate limit 정보 추출:
  - Claude: `anthropic-ratelimit-input-tokens-limit`
  - OpenAI: `x-ratelimit-limit-tokens`
- 결과를 `RateLimits` 구조체로 반환

### 타입

```rust
pub struct RateLimits {
    pub input_tokens_per_minute: u64,
    pub output_tokens_per_minute: u64,
    pub requests_per_minute: u64,
}
```

### 비용

입력 ~10토큰 + 출력 ~10토큰. Claude Sonnet 기준 $0.0001 이하.

### 변경 범위

- `analyzer/anthropic.rs`: `probe_rate_limits()` 함수 추가. 기존 `call_api`와 별도로 응답 헤더를 파싱하는 저수준 함수 필요.
- `analyzer/openai.rs`: 동일 구조의 `probe_rate_limits()` 함수 추가.
- `analyzer/provider.rs`: `LlmProvider`에 `probe_rate_limits()` 디스패치 메서드 추가.

## 섹션 2: 토큰 추정기

### 목적

API 호출 없이 로컬에서 프롬프트 토큰 수를 사전 추정한다.

### 방식

- 정확한 tokenizer 대신 글자 수 기반 간이 추정
- 비율: `글자 수 ÷ 3` (한국어/영어 혼합 텍스트에서 보수적)
- 시스템 프롬프트 토큰도 합산 (고정 문자열이므로 상수로 미리 계산)

### 타입

```rust
pub struct SessionEstimate {
    pub session_id: String,
    pub estimated_tokens: u64,
    pub entry_count: usize,
}
```

### 인터페이스

```rust
/// 세션별 토큰 추정
pub fn estimate_sessions(entries: &[LogEntry]) -> Vec<SessionEstimate>
```

### 변경 범위

- `analyzer/prompt.rs`: 추정 함수 추가. 기존 `build_prompt`과 `extract_session_ids`를 활용.

## 섹션 3: 실행 계획 수립

### 목적

ITPM과 세션별 추정 토큰을 비교해서 전체 실행 전략을 결정한다.

### 전략 분기

```
세션 추정 토큰 vs ITPM
  ├─ 전체 합계 ≤ ITPM → is_single_shot: true (한 번에 전송)
  ├─ 개별 세션 ≤ ITPM → Direct 스텝 (세션별 순차 전송)
  └─ 개별 세션 > ITPM → Summarize 스텝 (청크 분할 → 요약 → 분석)
```

### 타입

```rust
pub enum StepStrategy {
    Direct,
    Summarize { chunks: u64 },
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

### 핵심 로직

`is_single_shot`이면 기존처럼 한 번에 보냄. 높은 tier 사용자는 오버헤드 없음.

### 변경 범위

- 새 모듈 `analyzer/planner.rs`

## 섹션 4: 요약 전략

### 목적

ITPM을 초과하는 대형 세션을 요약해서 분석 가능한 크기로 축소한다.

### 흐름

```
대형 세션 (50K 토큰)
  → ITPM(30K) 기준으로 청크 분할 (2개)
  → 각 청크에 요약 프롬프트 적용
  → 요약 결과 합치기
  → 합쳐진 요약으로 최종 분석
```

### 요약 프롬프트

rwd의 인사이트 카테고리에 맞춤 설계:

```
"다음 개발 세션 대화에서 아래 항목을 중심으로 요약하라:
- 내린 기술적 결정과 그 이유
- 실수나 수정 사항
- 새로 배운 점 (TIL)
- 흥미로운 발견이나 의문점
원문의 구체적 기술 용어와 맥락을 보존하라."
```

### 청크 분할 단위

대화 메시지(turn) 경계에서 자른다. 메시지 중간에서 자르지 않는다.

### Rate pacing

청크 간 요약 호출 사이에 ITPM 리필 대기. 대기 시간: `(사용한 토큰 / ITPM) × 60초`

### 모델

요약과 분석 모두 사용자가 설정한 동일 모델을 사용한다. 경량 모델 옵션은 향후 성능 비교 후 고려.

### 변경 범위

- 새 모듈 `analyzer/summarizer.rs`
- `analyzer/provider.rs`: 요약 프롬프트 상수(`SUMMARIZE_PROMPT`) 추가

## 섹션 5: 실행 엔진 + UX 출력

### 목적

ExecutionPlan을 받아 순차 실행하고 진행 상황을 실시간 표시한다.

### 인터페이스

```rust
pub async fn execute_plan(
    plan: &ExecutionPlan,
    provider: &LlmProvider,
    api_key: &str,
    redactor_enabled: bool,
) -> Result<(AnalysisResult, RedactResult), AnalyzerError>
```

### 실행 흐름

1. `is_single_shot` → 기존과 동일하게 한 번에 호출
2. 아닐 경우 → 스텝별 순차 실행:
   - `Direct`: 해당 세션 프롬프트 생성 → API 호출
   - `Summarize`: 청크 분할 → 각 청크 요약 (대기 포함) → 합쳐서 분석
3. 모든 스텝 결과를 `merge_results`로 병합

### 토큰 예산 관리

- 각 호출 후 사용한 토큰 차감
- 잔여 예산 < 다음 호출 추정 토큰 → 리필 대기
- 대기 시간: `(부족분 / ITPM) × 60초`

### UX 출력

```
⠋ API 한도 확인 중...
✓ ITPM: 30,000 | OTPM: 8,000 | RPM: 50

✓ 세션 3개 분석 예정 (총 85,000 토큰 추정)
  • session_abc123: 12,000 토큰 → 직접 분석
  • session_def456: 48,000 토큰 → 요약 후 분석 (2 청크)
  • session_ghi789: 25,000 토큰 → 직접 분석

⠋ [1/3] session_abc123 분석 중...
✓ [1/3] 완료
⠋ 다음 요청까지 대기 중... (24초)
⠋ [2/3] session_def456 요약 중... (청크 1/2)
✓ [2/3] 요약 청크 1/2 완료
⠋ 다음 요청까지 대기 중... (58초)
⠋ [2/3] session_def456 요약 중... (청크 2/2)
✓ [2/3] 요약 완료, 분석 중...
✓ [2/3] 완료
⠋ [3/3] session_ghi789 분석 중...
✓ [3/3] 완료

✓ 전체 분석 완료 (3분 12초)
```

### 변경 범위

- `analyzer/mod.rs`: 기존 `analyze_entries()` 리팩터링. try-fallback 로직을 probe → plan → execute 흐름으로 교체. `analyze_entries_by_session()`, `is_context_limit_error()`, `is_rate_limit_error()` 제거.

## 제거 대상

기존 fallback 관련 코드를 제거한다:

- `analyzer/mod.rs`: `analyze_entries_by_session()` 함수
- `analyzer/mod.rs`: `is_context_limit_error()`, `is_rate_limit_error()` 함수 및 테스트
- `analyzer/mod.rs`: 400/429 에러 분기 로직

## 새 모듈 요약

| 모듈 | 책임 |
|------|------|
| `analyzer/planner.rs` (신규) | 실행 계획 수립 — RateLimits + SessionEstimate → ExecutionPlan |
| `analyzer/summarizer.rs` (신규) | 대형 세션 청크 분할 및 요약 |

## 변경 모듈 요약

| 모듈 | 변경 내용 |
|------|---------|
| `analyzer/mod.rs` | analyze_entries 리팩터링, fallback 제거, execute_plan 도입 |
| `analyzer/anthropic.rs` | probe_rate_limits() 추가, 응답 헤더 파싱 |
| `analyzer/openai.rs` | probe_rate_limits() 추가, 응답 헤더 파싱 |
| `analyzer/provider.rs` | probe 디스패치, SUMMARIZE_PROMPT 상수 추가 |
| `analyzer/prompt.rs` | estimate_sessions() 추가 |

## 테스트

- `planner.rs`: 전략 분기 테스트 (single_shot, direct, summarize)
- `prompt.rs`: 토큰 추정 테스트
- `summarizer.rs`: 청크 분할 경계 테스트 (메시지 단위)
- `mod.rs`: execute_plan 통합 테스트 (mock provider)

## 관련 이슈

- GitHub Issue #36 (closed): 대용량 로그 분석 시 LLM 토큰 제한 처리
