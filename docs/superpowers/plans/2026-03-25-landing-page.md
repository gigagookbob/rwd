# rwd Landing Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** rwd의 제품 소개 + 다운로드 유도를 위한 정적 랜딩 페이지를 순수 HTML/CSS/JS로 구현

**Architecture:** 별도 레포지토리(`rwd-website`)에 단일 `index.html` + 분리된 CSS/JS/i18n 파일로 구성. 빌드 도구 없이 Vercel에 정적 배포. i18n은 `data-i18n` 속성 + JSON 파일 기반.

**Tech Stack:** HTML5, CSS3 (Custom Properties, Grid, Flexbox, Keyframes), Vanilla JS (Intersection Observer, Clipboard API), Vercel (정적 배포)

**Spec:** `docs/superpowers/specs/2026-03-24-landing-page-design.md`

**Repo location:** `/Users/jinwoohan/workspace/repos/company/zerorder/rwd-website`

---

## File Structure

```
rwd-website/
├── index.html          # 전체 HTML 구조 (9개 섹션, SEO meta, Schema.org)
├── css/
│   └── style.css       # CSS variables, reset, 전체 레이아웃, 반응형, 애니메이션
├── js/
│   ├── main.js         # 터미널 애니메이션, FAQ 아코디언, 클립보드 복사, 스크롤 fade-in
│   └── i18n.js         # 언어 토글 (JSON 로드 + data-i18n 텍스트 교체 + localStorage)
├── i18n/
│   ├── en.json         # 영어 텍스트 (모든 data-i18n 키)
│   └── ko.json         # 한국어 텍스트
├── assets/
│   └── og-image.png    # Open Graph 이미지 (placeholder → 추후 교체)
└── vercel.json         # Cache headers, clean URLs
```

---

### Task 1: Project Scaffolding

**Files:**
- Create: `rwd-website/` (새 레포지토리)
- Create: `index.html`, `css/style.css`, `js/main.js`, `js/i18n.js`

- [ ] **Step 1: 레포지토리 생성 및 디렉토리 구조**

```bash
mkdir -p /Users/jinwoohan/workspace/repos/company/zerorder/rwd-website/{css,js,i18n,assets}
cd /Users/jinwoohan/workspace/repos/company/zerorder/rwd-website
git init
```

- [ ] **Step 2: 빈 파일 생성**

```bash
touch index.html css/style.css js/main.js js/i18n.js i18n/en.json i18n/ko.json
```

- [ ] **Step 3: .gitignore 작성**

`.gitignore`:
```
.DS_Store
.superpowers/
node_modules/
```

- [ ] **Step 4: 브라우저에서 확인**

`index.html`에 최소 HTML 작성:
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>rwd — Your AI sessions, journaled</title>
  <link rel="stylesheet" href="css/style.css">
</head>
<body>
  <p>rwd-website scaffold</p>
  <script src="js/i18n.js"></script>
  <script src="js/main.js"></script>
</body>
</html>
```

브라우저에서 `index.html` 열어 텍스트가 보이는지 확인.

- [ ] **Step 5: 커밋**

```bash
git add .gitignore index.html css/style.css js/main.js js/i18n.js i18n/en.json i18n/ko.json
git commit -m "chore: scaffold rwd-website project"
```

---

### Task 2: CSS Foundation

**Files:**
- Modify: `css/style.css`

- [ ] **Step 1: CSS 변수 및 리셋**

`css/style.css`:
```css
/* === Variables === */
:root {
  --bg-primary: #09090b;
  --bg-secondary: #18181b;
  --border: #27272a;
  --text-primary: #fafafa;
  --text-secondary: #a1a1aa;
  --text-muted: #71717a;
  --text-dim: #52525b;
  --accent-green: #22c55e;
  --accent-red: #ef4444;
  --accent-yellow: #eab308;
  --cta-bg: #fafafa;
  --cta-text: #09090b;
  --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  --font-mono: "SF Mono", "Fira Code", "Cascadia Code", "Consolas", monospace;
  --max-width: 720px;
  --section-padding: 80px 24px;
}

/* === Reset === */
*, *::before, *::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html {
  scroll-behavior: smooth;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

body {
  font-family: var(--font-sans);
  background: var(--bg-primary);
  color: var(--text-secondary);
  line-height: 1.6;
}

a {
  color: inherit;
  text-decoration: none;
}

/* === Layout === */
.container {
  max-width: var(--max-width);
  margin: 0 auto;
  padding: 0 24px;
}

.section {
  padding: var(--section-padding);
}

.section-title {
  font-size: 24px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.5px;
  text-align: center;
  margin-bottom: 8px;
}

.section-subtitle {
  font-size: 15px;
  color: var(--text-muted);
  text-align: center;
  margin-bottom: 40px;
}

.divider {
  height: 1px;
  background: var(--border);
  margin: 0 24px;
  max-width: var(--max-width);
  margin-left: auto;
  margin-right: auto;
}

/* === Scroll Animation === */
.fade-in {
  opacity: 0;
  transform: translateY(20px);
  transition: opacity 0.6s ease, transform 0.6s ease;
}

.fade-in.visible {
  opacity: 1;
  transform: translateY(0);
}
```

- [ ] **Step 2: 브라우저에서 배경색 확인**

`index.html`의 `<body>`에 배경이 `#09090b`(거의 검정)으로 표시되는지 확인.

- [ ] **Step 3: 커밋**

```bash
git add css/style.css
git commit -m "style: add CSS foundation with variables, reset, layout utilities"
```

---

### Task 3: Navigation

**Files:**
- Modify: `index.html`
- Modify: `css/style.css`

- [ ] **Step 1: Nav HTML 작성**

`index.html`의 `<body>` 안에 기존 `<p>` 태그를 교체:
```html
<body>
  <!-- Nav -->
  <nav class="nav">
    <div class="nav-inner">
      <div class="nav-left">
        <a href="/" class="nav-logo">rwd</a>
        <div class="nav-links">
          <a href="https://github.com/gigagookbob/rwd" target="_blank" rel="noopener">GitHub</a>
          <a href="https://github.com/gigagookbob/rwd#readme" target="_blank" rel="noopener" data-i18n="nav.docs">Docs</a>
        </div>
      </div>
      <div class="nav-right">
        <button class="lang-toggle" id="langToggle" aria-label="Toggle language">
          <span class="lang-option" data-lang="en">EN</span>
          <span class="lang-separator">/</span>
          <span class="lang-option" data-lang="ko">KO</span>
        </button>
        <a href="https://github.com/gigagookbob/rwd" target="_blank" rel="noopener" class="nav-github-btn">
          ★ <span data-i18n="nav.star">Star on GitHub</span>
        </a>
      </div>
    </div>
  </nav>

  <script src="js/i18n.js"></script>
  <script src="js/main.js"></script>
</body>
```

- [ ] **Step 2: Nav CSS**

`css/style.css`에 추가:
```css
/* === Nav === */
.nav {
  position: sticky;
  top: 0;
  z-index: 100;
  background: var(--bg-primary);
  border-bottom: 1px solid var(--border);
  backdrop-filter: blur(12px);
}

.nav-inner {
  display: flex;
  justify-content: space-between;
  align-items: center;
  max-width: var(--max-width);
  margin: 0 auto;
  padding: 14px 24px;
}

.nav-left {
  display: flex;
  align-items: center;
  gap: 32px;
}

.nav-logo {
  font-size: 17px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.5px;
}

.nav-links {
  display: flex;
  gap: 20px;
}

.nav-links a {
  font-size: 14px;
  color: var(--text-muted);
  transition: color 0.2s;
}

.nav-links a:hover {
  color: var(--text-primary);
}

.nav-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.lang-toggle {
  background: none;
  border: none;
  color: var(--text-dim);
  font-size: 13px;
  cursor: pointer;
  font-family: var(--font-sans);
  padding: 4px 8px;
}

.lang-toggle .lang-option {
  transition: color 0.2s;
}

.lang-toggle .lang-option.active {
  color: var(--text-primary);
  font-weight: 600;
}

.lang-separator {
  margin: 0 2px;
}

.nav-github-btn {
  font-size: 13px;
  color: var(--text-primary);
  padding: 6px 14px;
  border: 1px solid var(--border);
  border-radius: 6px;
  transition: background 0.2s;
}

.nav-github-btn:hover {
  background: var(--bg-secondary);
}

/* Mobile nav */
@media (max-width: 640px) {
  .nav-links {
    display: none;
  }

  .nav-github-btn span {
    display: none;
  }
}
```

- [ ] **Step 3: 브라우저 확인**

상단 네비게이션이 sticky로 표시되고, 모바일 너비에서 링크가 숨겨지는지 확인.

- [ ] **Step 4: 커밋**

```bash
git add index.html css/style.css
git commit -m "feat: add sticky navigation with logo, links, lang toggle"
```

---

### Task 4: Hero Section

**Files:**
- Modify: `index.html`
- Modify: `css/style.css`

- [ ] **Step 1: Hero HTML**

`</nav>` 태그 바로 아래에 추가:
```html
  <!-- Hero -->
  <section class="hero">
    <div class="container">
      <h1 class="hero-title" data-i18n="hero.title">Your AI sessions, journaled.</h1>
      <p class="hero-subtitle" data-i18n="hero.subtitle">Extracts decisions, learnings, and model corrections from your AI coding sessions. Saves them as Obsidian Daily Notes.</p>
      <div class="hero-cta">
        <a href="#install" class="btn btn-primary" data-i18n="hero.install">Install</a>
        <a href="https://github.com/gigagookbob/rwd" target="_blank" rel="noopener" class="btn btn-secondary" data-i18n="hero.github">View on GitHub</a>
      </div>
    </div>
  </section>
```

- [ ] **Step 2: Hero CSS**

`css/style.css`에 추가:
```css
/* === Hero === */
.hero {
  text-align: center;
  padding: 100px 24px 60px;
}

.hero-title {
  font-size: 42px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -1.5px;
  line-height: 1.15;
  max-width: 560px;
  margin: 0 auto;
}

.hero-subtitle {
  font-size: 16px;
  color: var(--text-muted);
  margin-top: 16px;
  max-width: 460px;
  margin-left: auto;
  margin-right: auto;
  line-height: 1.6;
}

.hero-cta {
  display: flex;
  gap: 10px;
  justify-content: center;
  margin-top: 32px;
}

/* Buttons */
.btn {
  display: inline-flex;
  align-items: center;
  padding: 10px 24px;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 600;
  font-family: var(--font-sans);
  cursor: pointer;
  transition: opacity 0.2s, background 0.2s;
  border: none;
  text-decoration: none;
}

.btn:hover {
  opacity: 0.9;
}

.btn-primary {
  background: var(--cta-bg);
  color: var(--cta-text);
}

.btn-secondary {
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border);
}

.btn-secondary:hover {
  background: var(--bg-secondary);
}

@media (max-width: 640px) {
  .hero {
    padding: 64px 24px 40px;
  }

  .hero-title {
    font-size: 30px;
  }

  .hero-cta {
    flex-direction: column;
    align-items: center;
  }
}
```

- [ ] **Step 3: 브라우저 확인**

히어로 섹션이 중앙 정렬로 표시되고, 모바일에서 CTA 버튼이 세로로 스택되는지 확인.

- [ ] **Step 4: 커밋**

```bash
git add index.html css/style.css
git commit -m "feat: add hero section with headline, subtitle, CTA buttons"
```

---

### Task 5: Terminal Demo with Animation

**Files:**
- Modify: `index.html`
- Modify: `css/style.css`
- Modify: `js/main.js`

- [ ] **Step 1: Terminal HTML**

히어로 섹션 `</section>` 바로 아래에 추가:
```html
  <!-- Terminal Demo -->
  <section class="terminal-section">
    <div class="container">
      <div class="terminal" id="terminalDemo">
        <div class="terminal-header">
          <div class="terminal-dots">
            <span class="dot dot-red"></span>
            <span class="dot dot-yellow"></span>
            <span class="dot dot-green"></span>
          </div>
          <span class="terminal-title">Terminal</span>
        </div>
        <div class="terminal-body">
          <div class="term-line" data-delay="0"><span class="term-prompt">$ </span><span class="term-cmd">rwd today</span></div>
          <div class="term-line" data-delay="1"><span class="term-muted">⠋ Discovering sessions...</span></div>
          <div class="term-line" data-delay="2"><span class="term-muted">  Found 3 Claude Code sessions</span></div>
          <div class="term-line" data-delay="3"><span class="term-muted">⠋ Analyzing with Claude API...</span></div>
          <div class="term-line" data-delay="4"><span class="term-muted">  ┌ Decisions ─────────────────────</span></div>
          <div class="term-line" data-delay="5"><span class="term-text">  │ Chose Axum over Actix-web for</span></div>
          <div class="term-line" data-delay="6"><span class="term-text">  │ its simpler middleware API</span></div>
          <div class="term-line" data-delay="7"><span class="term-muted">  └────────────────────────────────</span></div>
          <div class="term-line" data-delay="8"><span class="term-success">✓ Saved to ~/Vault/Daily/2025-03-24.md</span></div>
        </div>
      </div>
    </div>
  </section>
```

- [ ] **Step 2: Terminal CSS**

`css/style.css`에 추가:
```css
/* === Terminal === */
.terminal-section {
  padding: 0 24px 80px;
}

.terminal {
  max-width: 520px;
  margin: 0 auto;
  background: var(--bg-secondary);
  border-radius: 12px;
  border: 1px solid var(--border);
  overflow: hidden;
}

.terminal-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
}

.terminal-dots {
  display: flex;
  gap: 6px;
}

.dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

.dot-red { background: var(--accent-red); }
.dot-yellow { background: var(--accent-yellow); }
.dot-green { background: var(--accent-green); }

.terminal-title {
  font-size: 12px;
  color: var(--text-dim);
  margin-left: 8px;
}

.terminal-body {
  padding: 20px;
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 2;
}

.term-line {
  opacity: 0;
  transform: translateY(4px);
  transition: opacity 0.4s ease, transform 0.4s ease;
}

.term-line.show {
  opacity: 1;
  transform: translateY(0);
}

.term-prompt { color: var(--text-secondary); }
.term-cmd { color: var(--text-primary); }
.term-muted { color: var(--text-muted); }
.term-text { color: var(--text-secondary); }
.term-success { color: var(--accent-green); }

@media (max-width: 640px) {
  .terminal-body {
    font-size: 11px;
    padding: 16px;
  }
}
```

- [ ] **Step 3: Terminal Animation JS**

`js/main.js`에 작성:
```js
// === Terminal Animation ===
function initTerminalAnimation() {
  const terminal = document.getElementById('terminalDemo');
  if (!terminal) return;

  const lines = terminal.querySelectorAll('.term-line');
  let animated = false;

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting && !animated) {
          animated = true;
          lines.forEach((line) => {
            const delay = parseInt(line.dataset.delay, 10) * 400;
            setTimeout(() => line.classList.add('show'), delay);
          });
          observer.unobserve(terminal);
        }
      });
    },
    { threshold: 0.3 }
  );

  observer.observe(terminal);
}

document.addEventListener('DOMContentLoaded', initTerminalAnimation);
```

- [ ] **Step 4: 브라우저 확인**

스크롤해서 터미널이 뷰포트에 들어오면 줄이 순차적으로 나타나는지 확인. 마지막 줄이 초록색인지 확인.

- [ ] **Step 5: 커밋**

```bash
git add index.html css/style.css js/main.js
git commit -m "feat: add terminal demo with sequential typing animation"
```

---

### Task 6: Comparison Table

**Files:**
- Modify: `index.html`
- Modify: `css/style.css`

- [ ] **Step 1: Comparison HTML**

터미널 `</section>` 아래에 추가:
```html
  <div class="divider"></div>

  <!-- Comparison -->
  <section class="section fade-in">
    <div class="container">
      <h2 class="section-title" data-i18n="comparison.title">Not another efficiency report.</h2>
      <p class="section-subtitle" data-i18n="comparison.subtitle">rwd is a developer journal, not a dashboard.</p>
      <div class="comparison-grid">
        <div class="comparison-card comparison-other">
          <div class="comparison-label" data-i18n="comparison.insights.label">Claude /insights</div>
          <ul class="comparison-list">
            <li data-i18n="comparison.insights.item1">📊 Tool usage patterns</li>
            <li data-i18n="comparison.insights.item2">📈 Token consumption</li>
            <li data-i18n="comparison.insights.item3">📋 30-day HTML report</li>
            <li data-i18n="comparison.insights.item4">🔢 Quantitative</li>
          </ul>
        </div>
        <div class="comparison-card comparison-rwd">
          <div class="comparison-label" data-i18n="comparison.rwd.label">rwd</div>
          <ul class="comparison-list">
            <li data-i18n="comparison.rwd.item1">🧠 Decisions &amp; reasoning</li>
            <li data-i18n="comparison.rwd.item2">📝 Learnings &amp; corrections</li>
            <li data-i18n="comparison.rwd.item3">📅 Daily Obsidian notes</li>
            <li data-i18n="comparison.rwd.item4">💭 Qualitative</li>
          </ul>
        </div>
      </div>
    </div>
  </section>
```

- [ ] **Step 2: Comparison CSS**

`css/style.css`에 추가:
```css
/* === Comparison === */
.comparison-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.comparison-card {
  background: var(--bg-secondary);
  border-radius: 10px;
  padding: 24px;
  border: 1px solid var(--border);
}

.comparison-rwd {
  border-color: rgba(250, 250, 250, 0.15);
}

.comparison-label {
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: var(--text-muted);
  margin-bottom: 16px;
}

.comparison-rwd .comparison-label {
  color: var(--text-primary);
}

.comparison-list {
  list-style: none;
  font-size: 14px;
  line-height: 2;
}

@media (max-width: 640px) {
  .comparison-grid {
    grid-template-columns: 1fr;
  }
}
```

- [ ] **Step 3: 브라우저 확인**

2컬럼 카드가 나란히 표시되고, rwd 카드의 테두리가 약간 밝은지 확인. 모바일에서 세로 스택.

- [ ] **Step 4: 커밋**

```bash
git add index.html css/style.css
git commit -m "feat: add comparison table (rwd vs /insights)"
```

---

### Task 7: Features Grid

**Files:**
- Modify: `index.html`
- Modify: `css/style.css`

- [ ] **Step 1: Features HTML**

비교 섹션 `</section>` 아래에 추가:
```html
  <div class="divider"></div>

  <!-- Features -->
  <section class="section fade-in">
    <div class="container">
      <h2 class="section-title" data-i18n="features.title">What rwd extracts</h2>
      <div class="features-grid">
        <div class="feature-card">
          <h3 class="feature-name" data-i18n="features.decisions.title">Decisions</h3>
          <p class="feature-desc" data-i18n="features.decisions.desc">Why you chose A over B. Captured with reasoning.</p>
        </div>
        <div class="feature-card">
          <h3 class="feature-name" data-i18n="features.learnings.title">Learnings</h3>
          <p class="feature-desc" data-i18n="features.learnings.desc">Things you were curious or confused about.</p>
        </div>
        <div class="feature-card">
          <h3 class="feature-name" data-i18n="features.corrections.title">Corrections</h3>
          <p class="feature-desc" data-i18n="features.corrections.desc">When you fixed the AI's mistakes. What went wrong and how.</p>
        </div>
        <div class="feature-card">
          <h3 class="feature-name" data-i18n="features.privacy.title">Privacy</h3>
          <p class="feature-desc" data-i18n="features.privacy.desc">Sensitive data auto-masked before LLM analysis.</p>
        </div>
      </div>
    </div>
  </section>
```

- [ ] **Step 2: Features CSS**

`css/style.css`에 추가:
```css
/* === Features === */
.features-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.feature-card {
  background: var(--bg-secondary);
  border-radius: 10px;
  padding: 24px;
  border: 1px solid var(--border);
}

.feature-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 8px;
}

.feature-desc {
  font-size: 14px;
  color: var(--text-muted);
  line-height: 1.5;
}

@media (max-width: 640px) {
  .features-grid {
    grid-template-columns: 1fr;
  }
}
```

- [ ] **Step 3: 브라우저 확인**

2x2 그리드가 표시되고, 모바일에서 1열로 변경되는지 확인.

- [ ] **Step 4: 커밋**

```bash
git add index.html css/style.css
git commit -m "feat: add features grid (decisions, learnings, corrections, privacy)"
```

---

### Task 8: Install Guide with Clipboard Copy

**Files:**
- Modify: `index.html`
- Modify: `css/style.css`
- Modify: `js/main.js`

- [ ] **Step 1: Install HTML**

기능 섹션 `</section>` 아래에 추가:
```html
  <div class="divider"></div>

  <!-- Install -->
  <section class="section fade-in" id="install">
    <div class="container">
      <h2 class="section-title" data-i18n="install.title">Install in seconds</h2>
      <div class="install-blocks">
        <div class="install-block">
          <div class="install-info">
            <span class="install-platform">macOS (Apple Silicon)</span>
            <code class="install-cmd">curl -fsSL https://raw.githubusercontent.com/gigagookbob/rwd/main/install.sh | sh</code>
          </div>
          <button class="copy-btn" data-copy="curl -fsSL https://raw.githubusercontent.com/gigagookbob/rwd/main/install.sh | sh" data-i18n="install.copy">Copy</button>
        </div>
        <div class="install-block">
          <div class="install-info">
            <span class="install-platform">Windows</span>
            <code class="install-cmd">irm https://raw.githubusercontent.com/gigagookbob/rwd/main/install.ps1 | iex</code>
          </div>
          <button class="copy-btn" data-copy="irm https://raw.githubusercontent.com/gigagookbob/rwd/main/install.ps1 | iex" data-i18n="install.copy">Copy</button>
        </div>
        <div class="install-block">
          <div class="install-info">
            <span class="install-platform" data-i18n="install.source">Build from source</span>
            <code class="install-cmd">cargo install --git https://github.com/gigagookbob/rwd.git</code>
          </div>
          <button class="copy-btn" data-copy="cargo install --git https://github.com/gigagookbob/rwd.git" data-i18n="install.copy">Copy</button>
        </div>
      </div>
    </div>
  </section>
```

- [ ] **Step 2: Install CSS**

`css/style.css`에 추가:
```css
/* === Install === */
.install-blocks {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-width: 560px;
  margin: 0 auto;
}

.install-block {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 16px 20px;
  gap: 16px;
}

.install-info {
  flex: 1;
  min-width: 0;
}

.install-platform {
  display: block;
  font-size: 12px;
  color: var(--text-muted);
  margin-bottom: 4px;
}

.install-cmd {
  display: block;
  font-family: var(--font-mono);
  font-size: 13px;
  color: var(--text-secondary);
  overflow-x: auto;
  white-space: nowrap;
}

.copy-btn {
  flex-shrink: 0;
  padding: 4px 12px;
  background: none;
  border: 1px solid var(--border);
  border-radius: 4px;
  color: var(--text-dim);
  font-size: 12px;
  font-family: var(--font-sans);
  cursor: pointer;
  transition: color 0.2s, border-color 0.2s;
}

.copy-btn:hover {
  color: var(--text-secondary);
  border-color: var(--text-dim);
}

.copy-btn.copied {
  color: var(--accent-green);
  border-color: var(--accent-green);
}
```

- [ ] **Step 3: Clipboard Copy JS**

`js/main.js`에 추가:
```js
// === Clipboard Copy ===
function initClipboardCopy() {
  document.querySelectorAll('.copy-btn').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const text = btn.dataset.copy;
      try {
        await navigator.clipboard.writeText(text);
        const original = btn.textContent;
        btn.textContent = 'Copied!';
        btn.classList.add('copied');
        setTimeout(() => {
          btn.textContent = original;
          btn.classList.remove('copied');
        }, 2000);
      } catch {
        // Fallback: select text in the code element
        const code = btn.parentElement.querySelector('.install-cmd');
        if (code) {
          const range = document.createRange();
          range.selectNodeContents(code);
          const sel = window.getSelection();
          sel.removeAllRanges();
          sel.addRange(range);
        }
      }
    });
  });
}

document.addEventListener('DOMContentLoaded', initClipboardCopy);
```

- [ ] **Step 4: 브라우저 확인**

3개 설치 블록이 표시되고, Copy 버튼 클릭 시 "Copied!"로 변경 후 2초 뒤 복원되는지 확인.

- [ ] **Step 5: 커밋**

```bash
git add index.html css/style.css js/main.js
git commit -m "feat: add install guide with clipboard copy for 3 platforms"
```

---

### Task 9: Output Example (Before → After)

**Files:**
- Modify: `index.html`
- Modify: `css/style.css`

- [ ] **Step 1: Output HTML**

설치 섹션 `</section>` 아래에 추가:
```html
  <div class="divider"></div>

  <!-- Output Example -->
  <section class="section fade-in">
    <div class="container">
      <h2 class="section-title" data-i18n="output.title">From sessions to journal</h2>
      <p class="section-subtitle" data-i18n="output.subtitle">Raw AI logs become structured daily notes.</p>
      <div class="output-split">
        <div class="output-panel output-before">
          <div class="output-label" data-i18n="output.before">Session Logs</div>
          <div class="output-code">
            <code>{"type":"human","text":"let's use axum instead"}</code>
            <code>{"type":"assistant","text":"Good choice. Axum..."}</code>
            <code>{"type":"human","text":"wait, that's wrong"}</code>
            <code>{"type":"tool_use","name":"Edit",...}</code>
          </div>
        </div>
        <div class="output-arrow">→</div>
        <div class="output-panel output-after">
          <div class="output-label" data-i18n="output.after">Daily Note</div>
          <div class="output-md">
            <div class="md-heading">## Decisions</div>
            <div class="md-text">- Chose Axum over Actix-web for its simpler middleware API</div>
            <div class="md-heading">## Corrections</div>
            <div class="md-text">- Fixed incorrect middleware setup — assistant suggested outdated pattern</div>
          </div>
        </div>
      </div>
    </div>
  </section>
```

- [ ] **Step 2: Output CSS**

`css/style.css`에 추가:
```css
/* === Output Example === */
.output-split {
  display: flex;
  gap: 12px;
  align-items: stretch;
}

.output-panel {
  flex: 1;
  background: var(--bg-secondary);
  border-radius: 10px;
  padding: 20px;
  border: 1px solid var(--border);
}

.output-after {
  border-color: rgba(250, 250, 250, 0.1);
}

.output-label {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 1px;
  color: var(--text-muted);
  margin-bottom: 12px;
}

.output-code code {
  display: block;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-dim);
  line-height: 1.8;
  word-break: break-all;
}

.output-arrow {
  display: flex;
  align-items: center;
  color: var(--text-dim);
  font-size: 24px;
  flex-shrink: 0;
}

.md-heading {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  margin-top: 8px;
}

.md-heading:first-child {
  margin-top: 0;
}

.md-text {
  font-size: 13px;
  color: var(--text-secondary);
  line-height: 1.6;
}

@media (max-width: 640px) {
  .output-split {
    flex-direction: column;
  }

  .output-arrow {
    justify-content: center;
    transform: rotate(90deg);
  }
}
```

- [ ] **Step 3: 브라우저 확인**

좌측 JSONL → 우측 Markdown이 나란히 표시되고, 모바일에서 세로 스택 + 화살표 회전 확인.

- [ ] **Step 4: 커밋**

```bash
git add index.html css/style.css
git commit -m "feat: add output example (session logs → daily note)"
```

---

### Task 10: FAQ Accordion

**Files:**
- Modify: `index.html`
- Modify: `css/style.css`
- Modify: `js/main.js`

- [ ] **Step 1: FAQ HTML**

출력 예시 `</section>` 아래에 추가:
```html
  <div class="divider"></div>

  <!-- FAQ -->
  <section class="section fade-in">
    <div class="container">
      <h2 class="section-title">FAQ</h2>
      <div class="faq-list">
        <div class="faq-item">
          <button class="faq-question" aria-expanded="false">
            <span data-i18n="faq.q1">Which AI tools are supported?</span>
            <span class="faq-icon">+</span>
          </button>
          <div class="faq-answer">
            <p data-i18n="faq.a1">Claude Code and Codex. More coming soon.</p>
          </div>
        </div>
        <div class="faq-item">
          <button class="faq-question" aria-expanded="false">
            <span data-i18n="faq.q2">Do I need Obsidian?</span>
            <span class="faq-icon">+</span>
          </button>
          <div class="faq-answer">
            <p data-i18n="faq.a2">No. Output goes to any directory as .md files. Obsidian is optional.</p>
          </div>
        </div>
        <div class="faq-item">
          <button class="faq-question" aria-expanded="false">
            <span data-i18n="faq.q3">Is it free?</span>
            <span class="faq-icon">+</span>
          </button>
          <div class="faq-answer">
            <p data-i18n="faq.a3">Yes. MIT licensed, fully open source. You just need your own LLM API key.</p>
          </div>
        </div>
        <div class="faq-item">
          <button class="faq-question" aria-expanded="false">
            <span data-i18n="faq.q4">What LLM providers are supported?</span>
            <span class="faq-icon">+</span>
          </button>
          <div class="faq-answer">
            <p data-i18n="faq.a4">Anthropic Claude and OpenAI.</p>
          </div>
        </div>
        <div class="faq-item">
          <button class="faq-question" aria-expanded="false">
            <span data-i18n="faq.q5">Is my data safe?</span>
            <span class="faq-icon">+</span>
          </button>
          <div class="faq-answer">
            <p data-i18n="faq.a5">Logs are processed locally. Sensitive data (API keys, tokens, IPs) is automatically masked before sending to the LLM.</p>
          </div>
        </div>
      </div>
    </div>
  </section>
```

- [ ] **Step 2: FAQ CSS**

`css/style.css`에 추가:
```css
/* === FAQ === */
.faq-list {
  max-width: 560px;
  margin: 0 auto;
  border-top: 1px solid var(--border);
}

.faq-item {
  border-bottom: 1px solid var(--border);
}

.faq-question {
  width: 100%;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 18px 0;
  background: none;
  border: none;
  color: var(--text-primary);
  font-size: 15px;
  font-family: var(--font-sans);
  cursor: pointer;
  text-align: left;
}

.faq-icon {
  font-size: 18px;
  color: var(--text-dim);
  transition: transform 0.3s;
  flex-shrink: 0;
  margin-left: 16px;
}

.faq-item.open .faq-icon {
  transform: rotate(45deg);
}

.faq-answer {
  max-height: 0;
  overflow: hidden;
  transition: max-height 0.3s ease;
}

.faq-answer p {
  padding-bottom: 18px;
  font-size: 14px;
  color: var(--text-muted);
  line-height: 1.6;
}
```

- [ ] **Step 3: FAQ Accordion JS**

`js/main.js`에 추가:
```js
// === FAQ Accordion ===
function initFaqAccordion() {
  document.querySelectorAll('.faq-question').forEach((btn) => {
    btn.addEventListener('click', () => {
      const item = btn.parentElement;
      const answer = item.querySelector('.faq-answer');
      const isOpen = item.classList.contains('open');

      // Close all
      document.querySelectorAll('.faq-item.open').forEach((openItem) => {
        openItem.classList.remove('open');
        openItem.querySelector('.faq-answer').style.maxHeight = null;
        openItem.querySelector('.faq-question').setAttribute('aria-expanded', 'false');
      });

      // Toggle current
      if (!isOpen) {
        item.classList.add('open');
        answer.style.maxHeight = answer.scrollHeight + 'px';
        btn.setAttribute('aria-expanded', 'true');
      }
    });
  });
}

document.addEventListener('DOMContentLoaded', initFaqAccordion);
```

- [ ] **Step 4: 브라우저 확인**

FAQ 아이템 클릭 시 답변이 슬라이드 다운으로 열리고, + 아이콘이 × 로 회전하는지 확인. 다른 아이템 클릭 시 이전 것이 닫히는지 확인.

- [ ] **Step 5: 커밋**

```bash
git add index.html css/style.css js/main.js
git commit -m "feat: add FAQ section with accordion interaction"
```

---

### Task 11: Footer CTA + Footer

**Files:**
- Modify: `index.html`
- Modify: `css/style.css`

- [ ] **Step 1: Footer HTML**

FAQ `</section>` 아래에 추가 (`<script>` 태그 앞):
```html
  <!-- Footer CTA -->
  <section class="footer-cta">
    <div class="container">
      <h2 class="footer-cta-title" data-i18n="cta.title">Start journaling your AI sessions.</h2>
      <div class="hero-cta">
        <a href="#install" class="btn btn-primary" data-i18n="cta.install">Install rwd</a>
        <a href="https://github.com/gigagookbob/rwd" target="_blank" rel="noopener" class="btn btn-secondary" data-i18n="cta.github">View on GitHub</a>
      </div>
    </div>
  </section>

  <!-- Footer -->
  <footer class="footer">
    <span data-i18n="footer.license">MIT License</span>
    <span class="footer-sep">·</span>
    <span data-i18n="footer.made">Made by zerorder</span>
  </footer>
```

- [ ] **Step 2: Footer CSS**

`css/style.css`에 추가:
```css
/* === Footer CTA === */
.footer-cta {
  text-align: center;
  padding: 80px 24px;
  border-top: 1px solid var(--border);
}

.footer-cta-title {
  font-size: 28px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.5px;
  margin-bottom: 24px;
}

/* === Footer === */
.footer {
  text-align: center;
  padding: 24px;
  font-size: 13px;
  color: var(--text-dim);
}

.footer-sep {
  margin: 0 8px;
}

@media (max-width: 640px) {
  .footer-cta-title {
    font-size: 22px;
  }
}
```

- [ ] **Step 3: 브라우저 확인**

하단 CTA가 표시되고, 푸터에 라이선스 정보가 보이는지 확인.

- [ ] **Step 4: 커밋**

```bash
git add index.html css/style.css
git commit -m "feat: add footer CTA and footer with license info"
```

---

### Task 12: i18n System

**Files:**
- Modify: `i18n/en.json`
- Modify: `i18n/ko.json`
- Modify: `js/i18n.js`

- [ ] **Step 1: en.json 작성**

`i18n/en.json`:
```json
{
  "nav.docs": "Docs",
  "nav.star": "Star on GitHub",
  "hero.title": "Your AI sessions, journaled.",
  "hero.subtitle": "Extracts decisions, learnings, and model corrections from your AI coding sessions. Saves them as Obsidian Daily Notes.",
  "hero.install": "Install",
  "hero.github": "View on GitHub",
  "comparison.title": "Not another efficiency report.",
  "comparison.subtitle": "rwd is a developer journal, not a dashboard.",
  "comparison.insights.label": "Claude /insights",
  "comparison.insights.item1": "📊 Tool usage patterns",
  "comparison.insights.item2": "📈 Token consumption",
  "comparison.insights.item3": "📋 30-day HTML report",
  "comparison.insights.item4": "🔢 Quantitative",
  "comparison.rwd.label": "rwd",
  "comparison.rwd.item1": "🧠 Decisions & reasoning",
  "comparison.rwd.item2": "📝 Learnings & corrections",
  "comparison.rwd.item3": "📅 Daily Obsidian notes",
  "comparison.rwd.item4": "💭 Qualitative",
  "features.title": "What rwd extracts",
  "features.decisions.title": "Decisions",
  "features.decisions.desc": "Why you chose A over B. Captured with reasoning.",
  "features.learnings.title": "Learnings",
  "features.learnings.desc": "Things you were curious or confused about.",
  "features.corrections.title": "Corrections",
  "features.corrections.desc": "When you fixed the AI's mistakes. What went wrong and how.",
  "features.privacy.title": "Privacy",
  "features.privacy.desc": "Sensitive data auto-masked before LLM analysis.",
  "install.title": "Install in seconds",
  "install.source": "Build from source",
  "install.copy": "Copy",
  "output.title": "From sessions to journal",
  "output.subtitle": "Raw AI logs become structured daily notes.",
  "output.before": "Session Logs",
  "output.after": "Daily Note",
  "faq.q1": "Which AI tools are supported?",
  "faq.a1": "Claude Code and Codex. More coming soon.",
  "faq.q2": "Do I need Obsidian?",
  "faq.a2": "No. Output goes to any directory as .md files. Obsidian is optional.",
  "faq.q3": "Is it free?",
  "faq.a3": "Yes. MIT licensed, fully open source. You just need your own LLM API key.",
  "faq.q4": "What LLM providers are supported?",
  "faq.a4": "Anthropic Claude and OpenAI.",
  "faq.q5": "Is my data safe?",
  "faq.a5": "Logs are processed locally. Sensitive data (API keys, tokens, IPs) is automatically masked before sending to the LLM.",
  "cta.title": "Start journaling your AI sessions.",
  "cta.install": "Install rwd",
  "cta.github": "View on GitHub",
  "footer.license": "MIT License",
  "footer.made": "Made by zerorder"
}
```

- [ ] **Step 2: ko.json 작성**

`i18n/ko.json`:
```json
{
  "nav.docs": "문서",
  "nav.star": "GitHub 스타",
  "hero.title": "AI 세션을 개발 일지로.",
  "hero.subtitle": "AI 코딩 세션에서 의사결정, 학습, 모델 수정사항을 자동 추출합니다. Obsidian 데일리 노트로 저장됩니다.",
  "hero.install": "설치하기",
  "hero.github": "GitHub에서 보기",
  "comparison.title": "또 다른 효율 리포트가 아닙니다.",
  "comparison.subtitle": "rwd는 대시보드가 아닌 개발자 저널입니다.",
  "comparison.insights.label": "Claude /insights",
  "comparison.insights.item1": "📊 도구 사용 패턴",
  "comparison.insights.item2": "📈 토큰 소비량",
  "comparison.insights.item3": "📋 30일 HTML 리포트",
  "comparison.insights.item4": "🔢 정량적 분석",
  "comparison.rwd.label": "rwd",
  "comparison.rwd.item1": "🧠 의사결정과 이유",
  "comparison.rwd.item2": "📝 학습과 수정사항",
  "comparison.rwd.item3": "📅 데일리 Obsidian 노트",
  "comparison.rwd.item4": "💭 정성적 분석",
  "features.title": "rwd가 추출하는 것",
  "features.decisions.title": "의사결정",
  "features.decisions.desc": "왜 A 대신 B를 선택했는지. 이유와 함께 기록됩니다.",
  "features.learnings.title": "학습",
  "features.learnings.desc": "궁금했거나 혼란스러웠던 것들.",
  "features.corrections.title": "수정사항",
  "features.corrections.desc": "AI의 실수를 고친 순간. 무엇이 잘못됐고 어떻게 고쳤는지.",
  "features.privacy.title": "프라이버시",
  "features.privacy.desc": "민감한 데이터는 LLM 분석 전에 자동으로 마스킹됩니다.",
  "install.title": "몇 초 만에 설치",
  "install.source": "소스에서 빌드",
  "install.copy": "복사",
  "output.title": "세션에서 저널로",
  "output.subtitle": "AI 로그가 구조화된 데일리 노트가 됩니다.",
  "output.before": "세션 로그",
  "output.after": "데일리 노트",
  "faq.q1": "어떤 AI 도구를 지원하나요?",
  "faq.a1": "Claude Code와 Codex를 지원합니다. 더 많은 도구가 추가될 예정입니다.",
  "faq.q2": "Obsidian이 필요한가요?",
  "faq.a2": "아니요. 어떤 디렉토리든 .md 파일로 출력됩니다. Obsidian은 선택 사항입니다.",
  "faq.q3": "무료인가요?",
  "faq.a3": "네. MIT 라이선스의 완전한 오픈소스입니다. LLM API 키만 있으면 됩니다.",
  "faq.q4": "어떤 LLM 제공자를 지원하나요?",
  "faq.a4": "Anthropic Claude와 OpenAI를 지원합니다.",
  "faq.q5": "데이터는 안전한가요?",
  "faq.a5": "로그는 로컬에서 처리됩니다. API 키, 토큰, IP 등 민감한 데이터는 LLM 전송 전에 자동으로 마스킹됩니다.",
  "cta.title": "AI 세션 저널링을 시작하세요.",
  "cta.install": "rwd 설치하기",
  "cta.github": "GitHub에서 보기",
  "footer.license": "MIT 라이선스",
  "footer.made": "Made by zerorder"
}
```

- [ ] **Step 3: i18n.js 작성**

`js/i18n.js`:
```js
(function () {
  const STORAGE_KEY = 'rwd-lang';
  const DEFAULT_LANG = 'en';
  const SUPPORTED = ['en', 'ko'];

  let translations = {};

  function detectLang() {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved && SUPPORTED.includes(saved)) return saved;
    const browser = navigator.language.slice(0, 2);
    return SUPPORTED.includes(browser) ? browser : DEFAULT_LANG;
  }

  function applyTranslations(lang) {
    document.querySelectorAll('[data-i18n]').forEach((el) => {
      const key = el.getAttribute('data-i18n');
      if (translations[key]) {
        el.textContent = translations[key];
      }
    });

    // Update active lang indicator
    document.querySelectorAll('.lang-option').forEach((opt) => {
      opt.classList.toggle('active', opt.dataset.lang === lang);
    });

    document.documentElement.lang = lang;
  }

  async function loadLang(lang) {
    try {
      const res = await fetch(`i18n/${lang}.json`);
      translations = await res.json();
      applyTranslations(lang);
      localStorage.setItem(STORAGE_KEY, lang);
    } catch {
      // Fallback: keep HTML defaults (English)
    }
  }

  function init() {
    const lang = detectLang();
    loadLang(lang);

    // Lang toggle click handler
    const toggle = document.getElementById('langToggle');
    if (toggle) {
      toggle.addEventListener('click', () => {
        const current = localStorage.getItem(STORAGE_KEY) || detectLang();
        const next = current === 'en' ? 'ko' : 'en';
        loadLang(next);
      });
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
```

- [ ] **Step 4: 브라우저 확인**

EN/KO 토글 클릭 시 모든 텍스트가 한국어↔영어로 전환되는지 확인. 페이지 새로고침 시 선택한 언어가 유지되는지 확인.

- [ ] **Step 5: 커밋**

```bash
git add i18n/en.json i18n/ko.json js/i18n.js
git commit -m "feat: add i18n system with EN/KO toggle and localStorage persistence"
```

---

### Task 13: Scroll Fade-in Animations

**Files:**
- Modify: `js/main.js`

- [ ] **Step 1: Intersection Observer JS**

`js/main.js`에 추가:
```js
// === Scroll Fade-in ===
function initScrollFadeIn() {
  const elements = document.querySelectorAll('.fade-in');
  if (!elements.length) return;

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add('visible');
          observer.unobserve(entry.target);
        }
      });
    },
    { threshold: 0.1 }
  );

  elements.forEach((el) => observer.observe(el));
}

document.addEventListener('DOMContentLoaded', initScrollFadeIn);
```

- [ ] **Step 2: 브라우저 확인**

페이지를 스크롤하며 각 섹션(`.fade-in` 클래스가 있는)이 뷰포트 진입 시 아래에서 위로 페이드인 되는지 확인.

- [ ] **Step 3: 커밋**

```bash
git add js/main.js
git commit -m "feat: add scroll fade-in animations with Intersection Observer"
```

---

### Task 14: SEO Meta + Schema.org

**Files:**
- Modify: `index.html`

- [ ] **Step 1: `<head>` 메타 태그 추가**

`index.html`의 `<head>` 섹션을 업데이트:
```html
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>rwd — Your AI sessions, journaled</title>
  <meta name="description" content="CLI tool that extracts decisions, learnings, and corrections from AI coding sessions and saves them as Obsidian Daily Notes.">

  <!-- Open Graph -->
  <meta property="og:title" content="rwd — Your AI sessions, journaled">
  <meta property="og:description" content="CLI tool that extracts decisions, learnings, and corrections from AI coding sessions and saves them as Obsidian Daily Notes.">
  <meta property="og:type" content="website">
  <meta property="og:image" content="assets/og-image.png">

  <!-- Twitter -->
  <meta name="twitter:card" content="summary_large_image">
  <meta name="twitter:title" content="rwd — Your AI sessions, journaled">
  <meta name="twitter:description" content="CLI tool that extracts decisions, learnings, and corrections from AI coding sessions and saves them as Obsidian Daily Notes.">

  <!-- Schema.org -->
  <script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    "name": "rwd",
    "description": "CLI tool that extracts decisions, learnings, and corrections from AI coding sessions and saves them as Obsidian Daily Notes.",
    "applicationCategory": "DeveloperApplication",
    "operatingSystem": "macOS, Windows, Linux",
    "offers": { "@type": "Offer", "price": "0" },
    "license": "https://opensource.org/licenses/MIT",
    "url": "https://github.com/gigagookbob/rwd"
  }
  </script>

  <link rel="stylesheet" href="css/style.css">
</head>
```

- [ ] **Step 2: og-image.png placeholder 생성**

1x1 투명 PNG를 placeholder로 생성 (추후 실제 이미지로 교체):

```bash
printf '\x89PNG\r\n\x1a\n' > assets/og-image.png
```

> 실제 OG 이미지는 사이트 완성 후 스크린샷 기반으로 별도 제작.

- [ ] **Step 3: 커밋**

```bash
git add index.html assets/og-image.png
git commit -m "feat: add SEO meta tags, Open Graph, Schema.org"
```

---

### Task 15: Vercel Deployment Config

**Files:**
- Create: `vercel.json`

- [ ] **Step 1: vercel.json 작성**

```json
{
  "headers": [
    {
      "source": "/css/(.*)",
      "headers": [{ "key": "Cache-Control", "value": "public, max-age=31536000, immutable" }]
    },
    {
      "source": "/js/(.*)",
      "headers": [{ "key": "Cache-Control", "value": "public, max-age=31536000, immutable" }]
    },
    {
      "source": "/assets/(.*)",
      "headers": [{ "key": "Cache-Control", "value": "public, max-age=31536000, immutable" }]
    },
    {
      "source": "/i18n/(.*)",
      "headers": [{ "key": "Cache-Control", "value": "public, max-age=3600" }]
    }
  ]
}
```

- [ ] **Step 2: GitHub 레포 생성 및 push**

```bash
cd /Users/jinwoohan/workspace/repos/company/zerorder/rwd-website
gh repo create zerorder/rwd-website --private --source=. --push
```

> public/private은 사용자 판단. Vercel 연결 후 자동 배포.

- [ ] **Step 3: Vercel 배포**

```bash
# Vercel CLI가 설치되어 있는 경우:
npx vercel --prod
# 또는 Vercel 대시보드에서 GitHub 레포 연결
```

- [ ] **Step 4: 배포된 URL 접속 확인**

모든 섹션이 정상 표시되고, i18n 토글, 터미널 애니메이션, FAQ, 클립보드 복사가 동작하는지 확인.

- [ ] **Step 5: 커밋**

```bash
git add vercel.json
git commit -m "chore: add Vercel deployment config with cache headers"
```
