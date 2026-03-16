# rwd summary + today 캐싱 설계

## 목표

1. `rwd today`에 캐싱 추가 — 엔트리 수 변경 없으면 LLM 호출 스킵
2. `rwd summary` 서브커맨드 — 개발진척사항용 짧은 요약 생성 (터미널 + Markdown + 클립보드)

## 1. today 캐싱

### 캐시 파일

`~/.rwd/cache/today-{YYYY-MM-DD}.json`:

```json
{
  "date": "2026-03-16",
  "claude_entry_count": 680,
  "codex_session_count": 0,
  "analysis": { ... }
}
```

### 로직

1. 엔트리 수집 후 캐시 파일 확인
2. 엔트리 수가 동일하면 캐시된 analysis 사용, LLM 호출 스킵
3. 엔트리 수가 다르면 재분석 후 캐시 갱신

## 2. rwd summary

### 흐름

1. 오늘의 캐시 확인 → 없으면 today 먼저 실행
2. 캐시된 분석 결과를 기반으로 별도 LLM 호출 (요약 전용 프롬프트)
3. 터미널 출력 + Daily Markdown에 `## 개발 진척사항` 섹션 추가 + 클립보드 복사

### 프롬프트

- 프로젝트별 불릿 리스트, 각 항목은 자유 문장
- 개발자/비개발자 모두 이해 가능
- 기술 용어 최소화, "뭘 했는지"에 집중

### 출력 예시

```
## 개발 진척사항

• doridori-app: 카카오/네이버/구글 소셜 로그인 Android 오류를 모두 해결하고 staging 환경에서 테스트 완료.
• doridori-app: 챗봇 페이지 UI 및 API 설계 완료 후 Data Layer 구현 착수.
• rwd: Codex 세션 파서를 추가하여 Claude Code 외에 Codex 로그도 분석 가능하도록 확장.
```

### 클립보드

macOS: `pbcopy`, Linux: `xclip`

## 영향 범위

| 파일 | 변경 |
|------|------|
| `src/cli.rs` | `Summary` 서브커맨드 추가 |
| `src/main.rs` | `run_summary()` + `run_today()` 캐싱 로직 |
| `src/analyzer/mod.rs` | `analyze_summary()` 함수 추가 |
| `src/analyzer/provider.rs` | 요약 전용 시스템 프롬프트 추가 |
| `src/output/markdown.rs` | 개발 진척사항 섹션 렌더링 |
| 신규: `src/cache.rs` | 캐시 읽기/쓰기 |
