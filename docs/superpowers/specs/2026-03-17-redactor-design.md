# Redactor: LLM 전송 전 민감 정보 마스킹

> GitHub Issue: #26
> Date: 2026-03-17
> Version: v0.5.0

## 배경

`rwd today` 실행 시 세션 로그 텍스트가 그대로 LLM API로 전송된다. API 키, 비밀번호, 내부 IP 등 민감 정보가 포함됐다면 그대로 외부로 유출될 수 있다.

## 결정 사항

| 항목 | 결정 | 근거 |
|------|------|------|
| 모듈 | `src/redactor/` (mod.rs + patterns.rs) | parser/analyzer와 책임 분리 (SRP) |
| 파이프라인 위치 | `analyzer/mod.rs` 내부, `build_prompt()` 후 `call_api()` 전 | 프롬프트 텍스트가 LLM으로 나가는 유일한 경로 |
| 접근법 | regex 기반, `PatternKind` enum | 단순함 우선, 향후 Aho-Corasick 교체 준비 |
| API | `redact_text(&str) -> (String, RedactResult)` | 단순 문자열 변환, infallible |
| 치환 형식 | `[REDACTED:TYPE]` | LLM 맥락 이해 + 디버깅 용이 |
| 내장 패턴 | 8개 | 주요 민감 정보 커버 |
| 설정 | `config.toml` `[redactor] enabled = true` | 기본 활성, 하위 호환 유지 |
| 터미널 출력 | 마스킹 건수 요약 한 줄 | 기존 출력 스타일과 일관 (이모지 없음) |
| 버전 | v0.5.0 | 새 기능 모듈 추가 → minor bump |
| 새 의존성 | `regex` 크레이트 | Rust 생태계 표준 정규식 라이브러리 |

## 모듈 구조

```
src/redactor/
├── mod.rs       # 공개 API: redact_text(), RedactResult
└── patterns.rs  # 내장 패턴 정의 (RedactorRule 목록, LazyLock 초기화)
```

### 핵심 타입

```rust
/// 패턴 종류 — 향후 FixedPrefix를 Aho-Corasick으로 교체 가능
/// 현재는 양쪽 모두 Regex로 동작하며, kind는 메타데이터 역할만 함
enum PatternKind {
    FixedPrefix,  // "sk-", "ghp_" 등 (고정 접두사 기반)
    Regex,        // "PASSWORD=..." 등 복합 패턴
}

struct RedactorRule {
    name: &'static str,      // "API_KEY", "BEARER_TOKEN" 등
    kind: PatternKind,
    pattern: Regex,           // 컴파일된 정규식
}

/// 마스킹 결과 요약
struct RedactResult {
    pub total_count: usize,
    pub by_type: BTreeMap<String, usize>,  // 정렬된 출력 보장
}
```

### 공개 API

```rust
/// 텍스트에서 민감 정보를 탐지하고 [REDACTED:TYPE]으로 치환한다.
/// 패턴은 LazyLock으로 초기화되므로 이 함수는 infallible이다.
pub fn redact_text(text: &str) -> (String, RedactResult)
```

- 입력: 프롬프트 텍스트 (build_prompt / build_codex_prompt의 반환값)
- 출력: 마스킹된 텍스트 + 통계
- 에러 없음: 패턴은 `LazyLock`으로 프로그램 시작 시 컴파일, 실패하면 panic (프로그래밍 에러)

## 파이프라인 흐름

```
세션 로그 (JSONL)
    ↓
parser (파싱)
    ↓ Vec<LogEntry>, Vec<(Summary, Vec<CodexEntry>)>
analyzer
    ├─ build_prompt() / build_codex_prompt()
    ├─ redactor::redact_text(&prompt)    ← 새로 추가
    ├─ call_api(&redacted_prompt)
    └─ parse_response()
    ↓ (AnalysisResult, RedactResult)
main.rs: 터미널 요약 출력
    ↓
output (Markdown 렌더링 + Vault 저장)
```

원본 텍스트가 외부로 전송되는 경로는 LLM API 호출뿐이다:
- 캐시: AnalysisResult (처리된 인사이트)를 저장, 원본 텍스트 아님
- Markdown: AnalysisResult에서 렌더링, 원본 텍스트 아님

따라서 프롬프트 텍스트 마스킹으로 모든 외부 유출 경로를 차단할 수 있다.

### analyzer/mod.rs 호출 예시

**Claude Code 경로:**

```rust
pub async fn analyze_entries(entries: &[LogEntry], redactor_enabled: bool)
    -> Result<(AnalysisResult, RedactResult), AnalyzerError>
{
    let prompt = prompt::build_prompt(entries)?;
    let (redacted_prompt, redact_result) = if redactor_enabled {
        redactor::redact_text(&prompt)
    } else {
        (prompt, RedactResult::empty())
    };
    let response = provider.call_api(&api_key, &redacted_prompt).await?;
    let result = insight::parse_response(&response)?;
    Ok((result, redact_result))
}
```

**Codex 경로:**

```rust
pub async fn analyze_codex_entries(entries: &[CodexEntry], session_id: &str, redactor_enabled: bool)
    -> Result<(AnalysisResult, RedactResult), AnalyzerError>
{
    let prompt = prompt::build_codex_prompt(entries, session_id)?;
    let (redacted_prompt, redact_result) = if redactor_enabled {
        redactor::redact_text(&prompt)
    } else {
        (prompt, RedactResult::empty())
    };
    let response = provider.call_api(&api_key, &redacted_prompt).await?;
    let result = insight::parse_response(&response)?;
    Ok((result, redact_result))
}
```

**main.rs에서 결과 출력:**

```rust
// analyze 호출 후 RedactResult를 합산하여 출력
if total_redact_result.total_count > 0 {
    println!("민감 정보 {}건 마스킹됨 ({})",
        total_redact_result.total_count,
        format_redact_summary(&total_redact_result));
}
```

## 내장 탐지 패턴

| 이름 | 종류 | 패턴 | 매칭 대상 |
|------|------|------|----------|
| `API_KEY` | FixedPrefix | `\bsk-[a-zA-Z0-9]{20,}\b` | OpenAI, Anthropic 키 |
| `AWS_KEY` | FixedPrefix | `\bAKIA[0-9A-Z]{16}\b` | AWS Access Key ID |
| `GITHUB_TOKEN` | FixedPrefix | `\bgh[ps]_[a-zA-Z0-9]{36,}\b` | GitHub PAT |
| `SLACK_TOKEN` | FixedPrefix | `\bxox[bpsa]-[a-zA-Z0-9\-]+\b` | Slack Token (bot/user/session/app) |
| `BEARER_TOKEN` | Regex | `Bearer\s+[a-zA-Z0-9\-._~+/]+=*` | Authorization 헤더 |
| `ENV_SECRET` | Regex | `(?i)(password\|secret\|api_key)\s*=\s*["'][^"']+["']` | 환경변수 할당 (따옴표로 감싼 값) |
| `PRIVATE_IP` | Regex | `\b(10\.\d+\.\d+\.\d+\|172\.(1[6-9]\|2\d\|3[01])\.\d+\.\d+\|192\.168\.\d+\.\d+)\b` | 사설 IP 주소 |
| `PRIVATE_KEY` | Regex | `-----BEGIN[A-Z ]*PRIVATE KEY-----` | PEM 개인키 블록 시작 |

변경 사항 (리뷰 반영):
- 모든 FixedPrefix 패턴에 `\b` 워드 바운더리 추가 (false positive 감소)
- `SLACK_TOKEN`: `xoxb-` → `xox[bpsa]-` (모든 Slack 토큰 타입 커버)
- `ENV_SECRET`: `\S+` → `["'][^"']+["']` (따옴표 감싼 값만 매칭, 코드 토론 false positive 감소)
- `PRIVATE_KEY` 패턴 추가 (PEM 키 블록)

### 알려진 제한 사항

- 멀티라인 시크릿 (여러 줄에 걸친 키)은 v0.5.0에서 미지원. 패턴은 한 줄 단위로 매칭.
- `ENV_SECRET`은 따옴표 없는 할당(`PASSWORD=mypass123`)은 매칭하지 않음 (false positive 최소화 우선)

## config.toml 연동

```toml
[redactor]
enabled = true   # 기본값: true (섹션 생략 시에도 활성)
```

- `[redactor]` 섹션 없음 → `Option<RedactorConfig>` = `None` → 기본 활성(`enabled = true`)
- `enabled = false` → 마스킹 스킵

기존 설정 구조체에 `redactor: Option<RedactorConfig>`로 추가. `RedactorConfig`는 `Serialize + Deserialize` derive. 하위 호환성 유지.

## 터미널 출력

```
=== rwd today (2026-03-17 14:30) ===

Claude Code
총 세션: 3
민감 정보 5건 마스킹됨 (API_KEY: 3, BEARER_TOKEN: 1, ENV_SECRET: 1)

Claude API로 인사이트 분석 중...
```

- 마스킹 0건 → 해당 줄 미출력
- `redactor.enabled = false` → 해당 줄 미출력
- `BTreeMap` 사용으로 타입명 알파벳순 정렬 보장
- 이모지 없음, 기존 출력 스타일과 일관

## 후속 이슈

- **커스텀 패턴 지원**: config.toml에서 사용자 정의 정규식 패턴 추가
- **Aho-Corasick 최적화**: FixedPrefix 패턴을 Aho-Corasick으로 교체하여 성능 개선
