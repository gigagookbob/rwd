# rwd Landing Page — Design Spec

## Overview

rwd의 제품 소개 + 다운로드 유도를 위한 정적 랜딩 페이지. superset.sh 스타일의 미니멀 모던 다크 테마.

## Decisions

| 항목 | 결정 | 이유 |
|------|------|------|
| 목적 | 제품 소개 + 다운로드 유도 | docs 사이트는 별도로 필요 시 추후 추가 |
| 핵심 메시지 | AI 세션 → 개발 일지 자동화 + /insights 대비 차별점 | rwd의 유니크한 포지셔닝 |
| 비주얼 톤 | 미니멀 모던 (흑백 + zinc) | superset.sh 스타일, 콘텐츠 중심 |
| 기술 스택 | 순수 HTML/CSS/JS | 프레임워크 불필요, 의존성 제로 |
| 배포 | Vercel | 커스텀 도메인 + HTTPS 자동 |
| 언어 | 영어 기본 + 한국어 토글 | 글로벌 + 국내 개발자 모두 커버 |
| 도메인 | 추후 결정 | Vercel 기본 도메인으로 시작 |
| 레포지토리 | 별도 레포 | rwd CLI 레포와 분리 관리 |

## Visual Design

### Color System

```
--bg-primary:    #09090b   (페이지 배경)
--bg-secondary:  #18181b   (카드/터미널 배경)
--border:        #27272a   (구분선, 카드 테두리)
--text-primary:  #fafafa   (헤드라인, 강조 텍스트)
--text-secondary:#a1a1aa   (본문)
--text-muted:    #71717a   (보조 텍스트)
--text-dim:      #52525b   (비활성 텍스트)
--accent-green:  #22c55e   (성공/완료 표시)
--cta-bg:        #fafafa   (Primary CTA 배경)
--cta-text:      #09090b   (Primary CTA 텍스트)
```

### Typography

- 헤드라인: system sans-serif (`-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`), 700 weight, tight letter-spacing
- 본문: system sans-serif, 400 weight
- 코드/터미널: `"SF Mono", "Fira Code", "Cascadia Code", monospace`

### Responsive Breakpoints

- Mobile: < 640px (단일 컬럼, 축소된 패딩)
- Desktop: >= 640px (기본 레이아웃)

## Page Structure

### 1. Navigation

```
[rwd logo]  [GitHub] [Docs]                    [EN/KO toggle] [⭐ Star on GitHub]
─────────────────────────────────────────────────────────────────────────────────
```

- 고정(sticky) 상단 네비게이션
- 로고: "rwd" 텍스트 (font-weight: 700)
- 좌측: GitHub, Docs 링크
- 우측: 언어 토글 (EN/KO), GitHub Star 버튼
- 모바일: 햄버거 메뉴 또는 최소화된 레이아웃

### 2. Hero Section

- 중앙 정렬
- 헤드라인: "Your AI sessions, journaled." (36px, bold)
- 서브카피: "Extracts decisions, learnings, and model corrections from your AI coding sessions. Saves them as Obsidian Daily Notes." (15px, muted)
- CTA: [Install] (primary, white bg) + [View on GitHub] (secondary, border only)
- max-width: ~560px

### 3. Terminal Demo

- 히어로 바로 아래
- macOS 스타일 터미널 윈도우 (빨/노/초 dots + "Terminal" 타이틀)
- `rwd today` 실행 과정을 CSS 타이핑 애니메이션으로 순차 표시:
  1. `$ rwd today`
  2. `⠋ Discovering sessions...`
  3. `  Found 3 Claude Code sessions`
  4. `⠋ Analyzing with Claude API...`
  5. `  ┌ Decisions ─────────────────────`
  6. `  │ Chose Axum over Actix-web for`
  7. `  │ its simpler middleware API`
  8. `  └────────────────────────────────`
  9. `✓ Saved to ~/Vault/Daily/2025-03-24.md` (초록색)
- 애니메이션: CSS keyframes로 각 줄이 0.3~0.5초 딜레이로 순차 등장 (opacity: 0 → 1)
- 뷰포트 진입 시 시작 (Intersection Observer)
- max-width: ~520px

### 4. Comparison Table (vs /insights)

- 섹션 헤드라인: "Not another efficiency report."
- 서브카피: "rwd is a developer journal, not a dashboard."
- 좌우 2컬럼 카드:
  - 좌측: Claude `/insights` (muted border) — 📊 Tool usage patterns, 📈 Token consumption, 📋 30-day HTML report, 🔢 Quantitative
  - 우측: `rwd` (subtle highlight border) — 🧠 Decisions & reasoning, 📝 Learnings & corrections, 📅 Daily Obsidian notes, 💭 Qualitative
- 모바일: 세로 스택

### 5. Features (4-card grid)

- 섹션 헤드라인: "What rwd extracts"
- 2x2 그리드 카드:
  - **Decisions**: "Why you chose A over B. Captured with reasoning."
  - **Learnings**: "Things you were curious or confused about."
  - **Corrections**: "When you fixed the AI's mistakes. What went wrong and how."
  - **Privacy**: "Sensitive data auto-masked before LLM analysis."
- 각 카드: bg-secondary, border, 제목(14px bold) + 설명(12px muted)
- 모바일: 단일 컬럼

### 6. Install Guide

- 섹션 헤드라인: "Install in seconds"
- 3개 플랫폼별 설치 블록 (세로 스택):
  - macOS (Apple Silicon): `curl -fsSL https://...install.sh | sh`
  - Windows: `irm https://...install.ps1 | iex`
  - Build from source: `cargo install --git https://...`
- 각 블록: 플랫폼 라벨 + monospace 명령어 + [Copy] 버튼
- Copy 버튼: `navigator.clipboard.writeText()` 사용, 클릭 시 "Copied!" 피드백

### 7. Output Example (Before → After)

- 섹션 헤드라인: "From sessions to journal"
- 서브카피: "Raw AI logs become structured daily notes."
- 좌우 Split 레이아웃:
  - 좌측 "Session Logs": raw JSONL 스니펫 (muted monospace)
  - 화살표: →
  - 우측 "Daily Note": 추출된 Markdown 결과 (structured, highlighted headings)
- 모바일: 세로 스택 (위: logs, 아래: output)

### 8. FAQ

- 아코디언 스타일 (클릭으로 열기/닫기)
- 항목:
  1. "Which AI tools are supported?" → Claude Code and Codex. More coming.
  2. "Do I need Obsidian?" → No. Output goes to any directory as .md files.
  3. "Is it free?" → Yes. MIT licensed, open source. You need your own LLM API key.
  4. "What LLM providers are supported?" → Anthropic Claude and OpenAI.
  5. "Is my data safe?" → Logs are processed locally. Sensitive data is auto-masked before sending to the LLM.

### 9. Footer CTA + Footer

- 헤드라인: "Start journaling your AI sessions."
- CTA 반복: [Install rwd] + [View on GitHub]
- 푸터: "MIT License · Made by zerorder"
- 소셜 링크: GitHub

## i18n Strategy

- `i18n/en.json`, `i18n/ko.json`에 모든 UI 텍스트를 key-value로 관리
- `js/i18n.js`가 JSON 로드 후 `data-i18n` 속성 기반으로 텍스트 교체
- 언어 선택은 `localStorage`에 저장, 재방문 시 유지
- HTML 구조:
  ```html
  <h1 data-i18n="hero.title">Your AI sessions, journaled.</h1>
  ```
- 기본값은 영어가 HTML에 하드코딩 (JS 비활성 시에도 영어 표시)

## Project Structure

```
rwd-website/              (별도 레포지토리)
├── index.html            # 메인 페이지
├── css/
│   └── style.css         # 전체 스타일 (다크 테마, 반응형)
├── js/
│   ├── main.js           # 터미널 애니메이션, FAQ 아코디언, 클립보드 복사
│   └── i18n.js           # 언어 토글 로직
├── i18n/
│   ├── en.json           # 영어 텍스트
│   └── ko.json           # 한국어 텍스트
├── assets/
│   └── og-image.png      # Open Graph 이미지
├── vercel.json           # Vercel 배포 설정 (optional)
└── README.md
```

## SEO & Meta

- `<title>`: "rwd — Your AI sessions, journaled"
- `<meta description>`: "CLI tool that extracts decisions, learnings, and corrections from AI coding sessions and saves them as Obsidian Daily Notes."
- Open Graph tags (title, description, image, url)
- Schema.org JSON-LD (SoftwareApplication)

## Interactions

| 요소 | 동작 |
|------|------|
| 터미널 데모 | 뷰포트 진입 시 CSS 타이핑 애니메이션 시작 |
| Copy 버튼 | 클릭 → 클립보드 복사 → "Copied!" 텍스트 변경 (2초 후 복원) |
| FAQ | 클릭 → 아코디언 열기/닫기 (CSS transition) |
| 언어 토글 | 클릭 → localStorage 저장 + 페이지 텍스트 교체 |
| 스크롤 | 각 섹션 fade-in (Intersection Observer + CSS opacity transition) |

## Out of Scope

- 사용자 인증 / 로그인
- 블로그 / Changelog
- 문서 사이트 (docs)
- 다크/라이트 모드 토글 (다크 전용)
- 애널리틱스 (추후 추가 가능)
