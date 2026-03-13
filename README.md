# rwd (rewind)

AI 코딩 세션 로그를 분석하여 일일 개발 인사이트를 추출하고, Obsidian vault에 Markdown으로 저장하는 CLI 도구.

## 설치

### GitHub Release에서 다운로드

[Releases](https://github.com/gigagookbob/rwd/releases) 페이지에서 OS에 맞는 바이너리를 다운로드합니다.

```bash
# macOS (Apple Silicon)
curl -LO https://github.com/gigagookbob/rwd/releases/latest/download/rwd-aarch64-apple-darwin.tar.gz
tar xzf rwd-aarch64-apple-darwin.tar.gz
sudo mv rwd /usr/local/bin/

# macOS (Intel)
curl -LO https://github.com/gigagookbob/rwd/releases/latest/download/rwd-x86_64-apple-darwin.tar.gz
tar xzf rwd-x86_64-apple-darwin.tar.gz
sudo mv rwd /usr/local/bin/

# Linux (x86_64)
curl -LO https://github.com/gigagookbob/rwd/releases/latest/download/rwd-x86_64-unknown-linux-gnu.tar.gz
tar xzf rwd-x86_64-unknown-linux-gnu.tar.gz
sudo mv rwd /usr/local/bin/
```

> macOS에서 "개발자를 확인할 수 없습니다" 경고가 뜨면:
> ```bash
> xattr -d com.apple.quarantine /usr/local/bin/rwd
> ```

### 소스에서 빌드

```bash
cargo install --git https://github.com/gigagookbob/rwd.git
```

## 설정

실행 디렉토리에 `.env` 파일을 생성합니다.

```bash
# LLM 프로바이더 선택 (anthropic 또는 openai, 기본값: anthropic)
# LLM_PROVIDER=anthropic

# Anthropic Claude API 키
ANTHROPIC_API_KEY=sk-ant-...

# 또는 OpenAI API 키
# LLM_PROVIDER=openai
# OPENAI_API_KEY=sk-...

# Obsidian vault 경로 (분석 결과를 Markdown으로 저장)
# RWD_VAULT_PATH=/path/to/obsidian/vault
```

## 사용법

```bash
# 오늘의 AI 코딩 세션 분석
rwd today
```

## 라이선스

MIT
