# Development Milestones — rwd

아래 순서대로 단계적으로 구현한다. 각 마일스톤은 독립적으로 빌드/테스트 가능해야 한다.

## M1: CLI 뼈대

- clap으로 기본 명령어 구조 정의
- `rwd today`, `rwd --help` 등 기본 동작 확인
- **학습 포인트**: cargo 프로젝트 구조, 의존성 관리, 기본 Rust 문법

## M2: 로그 파일 탐색 및 파싱

- Claude Code 로그 파일(JSONL) 위치 탐색
- serde로 구조체 역직렬화
- 유효하지 않은 로그 라인에 대한 에러 처리
- **학습 포인트**: ownership, borrowing, struct, enum, Result/Option, serde, 에러 처리

## M3: LLM API 연동

- 구조화된 데이터를 Claude API에 전달
- 인사이트 응답 수신 및 파싱
- API 키 관리 (환경 변수 또는 설정 파일)
- **학습 포인트**: async/await, reqwest, tokio, API 통신, 환경 변수 처리

## M4: Markdown 생성 및 저장

- 인사이트를 템플릿 기반 Markdown으로 변환
- Obsidian vault 경로에 날짜별 파일 저장
- **학습 포인트**: 파일 I/O, 문자열 포매팅, std::path, 날짜 처리

## M5: 마무리 및 개선

- 에러 처리 강화 (anyhow 또는 thiserror 도입)
- 설정 파일(config) 지원
- 복수 에이전트(Codex 등) 로그 지원 확장
- **학습 포인트**: 크레이트 설계, 에러 타입 추상화, 설정 관리 패턴
