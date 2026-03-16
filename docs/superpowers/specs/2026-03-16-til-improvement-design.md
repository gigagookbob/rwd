# TIL 섹션 개선 설계

## 목표

현재 `curiosities`/`corrections`에서 파생하는 얕은 TIL을 폐기하고, LLM이 "사용자가 실제로 배운 것"을 직접 추출하도록 변경한다. 각 항목은 제목(한 줄) + 맥락 설명(1-2줄) + 세션 ID로 구성된다.

## 대상 독자

미래의 나. 3개월 뒤 "이거 왜 이렇게 했더라?" 할 때 찾아보는 용도.

## 데이터 구조

기존 `SessionInsight`에 `til` 필드 추가:

```rust
pub struct TilItem {
    pub title: String,      // 한 줄 제목 (배운 것)
    pub detail: String,     // 1-2줄 맥락 설명 (왜, 어떻게 적용)
    pub session_id: String, // 원본 세션 추적용
}
```

`AnalysisResult.sessions[].til: Vec<TilItem>`

## LLM 프롬프트 변경

시스템 프롬프트 JSON 스키마에 `til` 필드 추가:

```json
"til": [
  {
    "title": "배운 것을 한 줄로 (한국어)",
    "detail": "왜 이게 필요했고 어떻게 적용했는지 1-2줄 (한국어)"
  }
]
```

Rules에 TIL 추출 지침 추가:
- curiosities나 corrections에서 파생하지 말고, 대화에서 사용자가 **실제로 배운 것**을 직접 추출
- 일반 상식이 아닌, 이 세션의 맥락에서 유의미한 학습만 포함
- 배운 게 없으면 빈 배열

## Markdown 출력

하단 `## TIL` 섹션에 모든 세션의 항목을 합침. 각 항목에 세션 ID HTML 주석 포함.

```markdown
## TIL

### serde의 tag 속성은 중첩 JSON에서 안 먹힌다
Codex JSONL이 type 필드가 두 곳에 있어서 serde tag로 한번에 파싱이 안 됐다.
2단계 파싱(loose → structured)으로 우회.
<!-- session: d31e7507 -->

### chrono Local vs Utc
DateTime<Utc>에 date_naive()만 쓰면 UTC 기준이라 KST 새벽 세션이 누락된다.
with_timezone(&Local) 변환 후 비교해야 한다.
<!-- session: 342dfbf0 -->
```

## 영향 범위

| 파일 | 변경 |
|------|------|
| `analyzer/insight.rs` | `TilItem` 구조체 추가, `SessionInsight`에 `til` 필드 추가 |
| `analyzer/provider.rs` | 시스템 프롬프트에 `til` 스키마 + 추출 규칙 추가 |
| `output/markdown.rs` | `render_til_section` 수정 — 제목+설명+세션ID 주석 |

## 기존 로직 제거

`output/markdown.rs`의 `render_session()`에서 `curiosities`/`corrections`를 `til_items`에 push하던 로직 제거. TIL은 오직 `SessionInsight.til` 필드에서만 가져옴. `curiosities`와 `corrections`는 각 세션 섹션에 그대로 유지.
