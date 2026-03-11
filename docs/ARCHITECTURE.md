# Architecture — rwd

## 핵심 흐름

```
CLI 진입 → 로그 파일 탐색/수집 → JSONL 파싱 & 구조화 → LLM API 호출 → Markdown 생성 → Obsidian Vault 저장
```

## 추출 대상 인사이트

- 사용자의 선택 분기 (A vs B 중 왜 A를 선택했는가)
- 사용자가 궁금했던 것, 헷갈렸던 것
- 모델이 틀리거나 몰라서 사용자가 수정한 것
- 세션 간 맥락 전환 (어떤 프로젝트에서 어떤 작업을 했는가)

## 2단계 처리 전략

1. **파싱 단계 (규칙 기반)**: 로그 파일을 구조화된 데이터로 변환
   - 누가 말했는지 (user / assistant)
   - 어떤 도구를 호출했는지
   - 에러 발생 여부
   - 되돌리기/수정 패턴

2. **분석 단계 (LLM 기반)**: 구조화된 데이터를 LLM에 전달하여 인사이트 추출
   - 원본 로그가 아닌 구조화된 데이터를 전달하여 정보 손실 최소화

## 입력 소스

### Claude Code

- 로그 위치: `~/.claude/projects/` 하위 JSONL 파일
- 형식: 각 줄이 독립된 JSON 객체

### Codex (추후 확장)

- 로그 위치 및 형식 파악 필요

## 프로젝트 구조

```
rwd/
├── Cargo.toml
├── AGENTS.md
├── docs/
│   ├── ARCHITECTURE.md    # 이 문서
│   ├── MILESTONES.md
│   ├── CONVENTIONS.md
│   └── LEARNING_GUIDE.md
├── src/
│   ├── main.rs            # CLI 진입점
│   ├── cli.rs             # clap 기반 CLI 정의
│   ├── parser/            # 로그 파싱 모듈
│   │   ├── mod.rs
│   │   └── claude.rs      # Claude Code 로그 파서
│   ├── analyzer/          # 구조화된 데이터 → LLM 인사이트 추출
│   │   └── mod.rs
│   ├── output/            # Markdown 생성 및 파일 저장
│   │   └── mod.rs
│   └── config.rs          # 설정 (경로, API 키 등)
└── tests/
```

## Core Dependencies

| 크레이트    | 용도               |
| ----------- | ------------------ |
| `clap`      | CLI 파싱           |
| `serde`     | 직렬화/역직렬화    |
| `serde_json`| JSON/JSONL 파싱    |
| `reqwest`   | HTTP 클라이언트    |
| `tokio`     | Async 런타임       |
| `walkdir`   | 파일 시스템 탐색   |
