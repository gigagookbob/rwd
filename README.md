# rwd (rewind)

AI 코딩 세션 로그를 분석하여 일일 개발 인사이트를 추출하고, Obsidian vault에 Markdown으로 저장하는 CLI 도구.

## 설치

### 원클릭 설치 (macOS Apple Silicon)

```bash
curl -fsSL https://raw.githubusercontent.com/gigagookbob/rwd/main/install.sh | sh
```

### 소스에서 빌드

```bash
cargo install --git https://github.com/gigagookbob/rwd.git
```

> macOS에서 "개발자를 확인할 수 없습니다" 경고가 뜨면:
> ```bash
> xattr -d com.apple.quarantine /usr/local/bin/rwd
> ```

## 설정

```bash
rwd init
```

API 키와 출력 경로를 설정합니다. Obsidian vault가 있으면 자동 감지됩니다.
설정은 `~/.config/rwd/config.toml`에 저장됩니다.

### 설정 변경

```bash
rwd config output-path /path/to/vault    # 출력 경로 변경
rwd config provider openai               # LLM 프로바이더 변경
rwd config api-key sk-...                # API 키 변경
```

## 사용법

```bash
# 오늘의 AI 코딩 세션 분석
rwd today
```

## 라이선스

MIT
