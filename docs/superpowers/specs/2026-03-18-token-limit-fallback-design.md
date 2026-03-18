# Design: Claude Code 분석 토큰 제한 fallback

## 목적

Claude Code 로그 분석 시 LLM API의 토큰 제한에 걸리면, 에러 종류에 따라 적절히 처리한다:
- 400 (컨텍스트 초과): 세션별 개별 분석으로 자동 전환
- 429 (TPM 초과): 친절한 에러 메시지로 해결 방법 안내

## 현재 상태

- `analyze_entries()`가 하루치 Claude Code 엔트리를 한 번에 LLM에 전달
- OpenAI TPM 제한(30K/분) 초과 시 429 에러로 분석 실패
- Codex는 이미 세션별 개별 분석 (`analyze_codex_entries()` per session)

## 설계

### 에러별 처리 전략

#### 400 (컨텍스트 윈도우 초과) → 세션별 분할 fallback

fallback 로직은 `analyze_entries()` 내부에서 처리한다. 이미 로드한 `(provider, api_key)`를 재사용하여 세션별 API 호출을 수행한다.

```
analyze_entries(전체 엔트리)
  → build_prompt(전체) → API 호출
  → 성공: 기존과 동일
  → 400(토큰 관련) 에러:
    → "세션별 분석으로 전환합니다" 안내 출력
    → extract_session_ids()로 세션 ID 목록 추출
    → 세션마다:
      → 원본 entries에서 해당 session_id로 필터 + clone → Vec<LogEntry>
      → build_prompt(&session_entries) → API 호출 (기존 함수 그대로 사용)
      → redact 처리
      → 개별 세션에서도 토큰 에러 발생 시: 해당 세션 스킵 + 경고 출력
      → "세션 3/8 분석 중..." 진행 표시
    → 각 AnalysisResult.sessions + RedactResult를 병합
```

#### 429 (TPM 초과) → 에러 메시지 개선

429는 분할로 해결되지 않음 (분할해도 분당 총 토큰이 동일하거나 초과). 대신 친절한 에러 메시지를 출력:

```
프롬프트가 너무 커서 OpenAI 토큰 제한을 초과했습니다.
해결 방법:
  • rwd config provider anthropic  (Anthropic으로 전환)
  • OpenAI 플랜 업그레이드       (TPM 한도 증가)
```

### 에러 판별

현재 `anthropic.rs`와 `openai.rs`에서 에러 시 다른 형식의 메시지를 반환한다:
- `anthropic.rs`: `"API 요청 실패 ({status}): {error_body}"`
- `openai.rs`: `"OpenAI API 요청 실패 ({status}): {error_body}"`

에러 메시지 문자열 파싱으로 판별:
- "429" 포함 → TPM 에러
- "400" 포함 + ("token" 또는 "context" 포함) → 컨텍스트 에러

**위험 사항**: 에러 메시지 형식에 의존하므로, 형식 변경 시 fallback이 동작하지 않을 수 있다. M5의 에러 타입 강화(thiserror) 시 구조화된 에러로 전환 예정.

### 변경 파일

#### 1. `src/analyzer/mod.rs` — `analyze_entries()` fallback 로직

- API 호출 실패 시 에러 메시지 검사
- 400 토큰 에러 → 세션별 분할 분석 + 결과/RedactResult 병합
- 429 에러 → 친절한 에러 메시지 반환
- 기타 에러 → 기존처럼 전파
- `is_context_limit_error()`, `is_rate_limit_error()` 판별 함수 추가

#### 2. `src/analyzer/prompt.rs` — 세션 ID 추출 함수 추가

- `extract_session_ids(entries: &[LogEntry]) -> Vec<String>` 추가
- 세션별 분할 시 원본 entries를 session_id로 filter + clone하여 `Vec<LogEntry>`로 만든 뒤 기존 `build_prompt()`에 전달
- `build_prompt()` 시그니처 변경 없음

#### 3. `src/analyzer/insight.rs` — 결과 병합 함수 추가

- `merge_results(results: Vec<AnalysisResult>) -> AnalysisResult`
- 각 결과의 `sessions` Vec을 하나로 합침
- 모듈 역할: "파싱 및 결과 조합"

### 변경하지 않는 것

- `anthropic.rs` / `openai.rs` — 에러 응답 형식 변경 없음
- `build_prompt()` 시그니처 — 변경 없음
- Codex 분석 — 이미 세션별 분석
- `provider.rs` — 프로바이더별 토큰 제한 설정 없음
- 토큰 수 사전 추정 — 불필요 (시도 후 fallback)

### 테스트

- `is_context_limit_error()` / `is_rate_limit_error()` 에러 판별 테스트
- `merge_results()` 병합 로직 테스트
- `extract_session_ids()` 세션 ID 추출 테스트
