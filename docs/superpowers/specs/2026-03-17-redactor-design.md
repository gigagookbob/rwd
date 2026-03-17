# Redactor: LLM 전송 전 민감 정보 마스킹

> GitHub Issue: #26
> Date: 2026-03-17
> Version: v0.5.0

## 배경

`rwd today` 실행 시 세션 로그 텍스트가 그대로 LLM API로 전송된다. API 키, 비밀번호, 내부 IP 등 민감 정보가 포함됐다면 그대로 외부로 유출될 수 있다.

## 결정 사항

| 항목 | 결정 | 근거 |
|------|------|------|
| 모듈 | `src/redactor/` (mod.rs + patterns.rs) | parser와 책임 분리 (SRP) |
| 파이프라인 위치 | `main.rs`에서 파싱 후, 분석 전 | 호출 흐름이 명시적으로 보임 |
| 접근법 | regex 기반, `PatternKind` enum | 단순함 우선, 향후 Aho-Corasick 교체 준비 |
| 추상화 | `Redactable` 트레이트 | 타입 무관 텍스트 접근 |
| 치환 형식 | `[REDACTED:TYPE]` | LLM 맥락 이해 + 디버깅 용이 |
| 내장 패턴 | 7개 | 주요 민감 정보 커버 |
| 설정 | `config.toml` `[redactor] enabled = true` | 기본 활성, 하위 호환 유지 |
| 터미널 출력 | 마스킹 건수 요약 한 줄 | 기존 출력 스타일과 일관 (이모지 없음) |
| 버전 | v0.5.0 | 새 기능 모듈 추가 → minor bump |

## 모듈 구조

```
src/redactor/
├── mod.rs       # 공개 API: redact_entries(), RedactResult
└── patterns.rs  # 내장 패턴 정의 (RedactorRule 목록)
```

### 핵심 타입

```rust
/// 패턴 종류 — 향후 FixedPrefix를 Aho-Corasick으로 교체 가능
enum PatternKind {
    FixedPrefix,  // "sk-", "ghp_" 등
    Regex,        // "PASSWORD=..." 등 복합 패턴
}

struct RedactorRule {
    name: &'static str,      // "API_KEY", "BEARER_TOKEN" 등
    kind: PatternKind,
    pattern: Regex,           // 컴파일된 정규식
}

struct RedactResult {
    pub total_count: usize,
    pub by_type: HashMap<String, usize>,  // {"API_KEY": 2, "BEARER_TOKEN": 1}
}
```

### Redactable 트레이트

```rust
trait Redactable {
    fn text_fields_mut(&mut self) -> Vec<&mut String>;
}
```

각 엔트리 타입(`LogEntry`, `CodexEntry`)이 이 트레이트를 구현한다. `redactor`는 타입에 무관하게 텍스트 필드를 순회하며 마스킹한다.

## 파이프라인 흐름

```
세션 로그 (JSONL)
    ↓
parser (파싱만)
    ↓ Vec<LogEntry>, Vec<CodexEntry>
redactor::redact_entries(&mut entries)    ← 새로 추가
    ↓ 마스킹된 entries + RedactResult
터미널 요약 출력 (건수 > 0일 때)
    ↓
analyzer (프롬프트 빌드 + LLM 호출)
    ↓
output (Markdown 렌더링 + Vault 저장)
```

`main.rs`에서의 호출:

```rust
let entries = collect_claude_entries()?;
let redact_result = redactor::redact_entries(&mut entries)?;
if redact_result.total_count > 0 {
    println!("민감 정보 {}건 마스킹됨 ({})", redact_result.total_count, format_summary(&redact_result));
}
let analysis = analyzer::analyze_entries(&entries).await?;
```

## 내장 탐지 패턴

| 이름 | 종류 | 패턴 | 매칭 대상 |
|------|------|------|----------|
| `API_KEY` | FixedPrefix | `sk-[a-zA-Z0-9]{20,}` | OpenAI, Anthropic 키 |
| `AWS_KEY` | FixedPrefix | `AKIA[0-9A-Z]{16}` | AWS Access Key ID |
| `GITHUB_TOKEN` | FixedPrefix | `gh[ps]_[a-zA-Z0-9]{36,}` | GitHub PAT |
| `SLACK_TOKEN` | FixedPrefix | `xoxb-[a-zA-Z0-9\-]+` | Slack Bot Token |
| `BEARER_TOKEN` | Regex | `Bearer\s+[a-zA-Z0-9\-._~+/]+=*` | Authorization 헤더 |
| `ENV_SECRET` | Regex | `(?i)(password\|secret\|token\|api_key)\s*=\s*\S+` | 환경변수 할당 |
| `PRIVATE_IP` | Regex | `\b(10\.\d+\.\d+\.\d+\|172\.(1[6-9]\|2\d\|3[01])\.\d+\.\d+\|192\.168\.\d+\.\d+)\b` | 사설 IP 주소 |

## config.toml 연동

```toml
[redactor]
enabled = true   # 기본값: true (섹션 생략 시에도 활성)
```

- `[redactor]` 섹션 없음 → 기본 활성
- `enabled = false` → 마스킹 스킵

기존 설정 구조체에 `redactor: Option<RedactorConfig>`로 추가하여 하위 호환성 유지.

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
- 이모지 없음, 기존 출력 스타일과 일관

## 후속 이슈

- **커스텀 패턴 지원**: config.toml에서 사용자 정의 정규식 패턴 추가
- **Aho-Corasick 최적화**: FixedPrefix 패턴을 Aho-Corasick으로 교체하여 성능 개선
