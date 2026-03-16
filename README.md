# rwd (rewind)

AI 코딩 세션 로그를 분석하여 일일 개발 인사이트를 추출하고, Obsidian vault에 Markdown으로 저장하는 CLI 도구.

## rwd vs Claude Code `/insights`

Claude Code에는 `/insights` 명령어가 내장되어 있습니다. rwd는 이와 목적이 다릅니다.

`/insights`는 **"나는 Claude를 효율적으로 쓰고 있나?"** 에 대한 답입니다 — 30일간의 도구 사용 패턴, 토큰 소비량, 마찰점을 분석하여 HTML 대시보드로 보여줍니다. 코딩 효율성 리포트입니다.

rwd는 **"오늘 뭘 결정했고, 왜?"** 에 대한 답입니다 — 하루의 모든 세션에서 의사결정, 궁금했던 것, 모델이 틀렸던 것을 추출하여 Obsidian Daily Notes로 쌓아갑니다. 개발 일지입니다.

| | `/insights` | `rwd` |
|--|-------------|-------|
| 관점 | 도구 사용 패턴, 효율성 | 의사결정, 학습, 모델 수정 |
| 기간 | 30일 롤링 | 매일 |
| 출력 | HTML 대시보드 (1회성) | Obsidian Daily Notes (누적) |
| 분석 | 정량적 (토큰, 도구 횟수, 코드량) | 정성적 (왜 A를 선택했나, 뭐가 궁금했나) |
| 환경 | Claude Code 세션 내에서만 | 독립 CLI, 어디서든 |

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

## 삭제

```bash
# rwd 바이너리 삭제
sh -c 'rm "$(command -v rwd)"'

# 설정/데이터도 완전히 삭제하려면:
rm -rf ~/.config/rwd ~/.rwd
```

## 라이선스

MIT
