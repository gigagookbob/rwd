# Next Session Prompt — Multi-Root Agent Session Discovery

다음 세션에서 아래 프롬프트를 그대로 사용한다.

```text
/home/jinwoo/workspace/hobby/rwd 레포에서 아래 설계를 구현해줘.

먼저 이 문서를 읽어:
- docs/superpowers/specs/2026-04-11-multi-root-agent-session-discovery-design.md

이번 작업 목표:
- Claude Code와 Codex 세션 로그를 "하나의 루트만 선택"하지 말고, 존재하는 모든 유효 루트에서 수집하도록 구현
- /home 계열 경로와 /mnt/c/... 계열 경로가 동시에 존재하면 둘 다 읽기
- 같은 세션/같은 엔트리가 중복 수집되면 dedupe
- Config override도 지원하기

구현 원칙:
- Rust 2024 stable 기준
- 작은 단계로 나눠서 구현
- 새로운 Rust 개념을 쓰면 왜 필요한지 설명
- unwrap() 대신 Result와 ? 사용
- 가능하면 공용 helper로 Claude/Codex의 중복 경로 탐색 로직을 정리
- Context7 MCP가 사용 가능하면 관련 crate API를 먼저 확인하고, 불가능하면 그 사실을 짧게 기록

구체 요구사항:
- src/config.rs에 optional [input] 섹션 추가
  - input.codex_roots: Option<Vec<String>>
  - input.claude_roots: Option<Vec<String>>
- parser layer에 multi-root discovery API 추가
  - Claude: discover_claude_log_roots()
  - Codex: discover_codex_session_roots()
- WSL 환경에서는 /home 루트와 /mnt/c/Users/* 후보를 모두 수집
- Codex는 session_id 기준으로 세션 merge
- Claude는 entry fingerprint 기준으로 dedupe
- main.rs의 collect_claude_entries_with_stats, collect_claude_entries, collect_codex_sessions를 multi-root 기반으로 바꾸기
- verbose 모드에서 실제 사용한 루트 목록을 출력하도록 개선하면 더 좋음

검증은 반드시 이 순서로:
- cargo build
- cargo clippy --all-targets --all-features -- -D warnings
- cargo test

주의:
- 구현은 이번 턴에서만 하고, 설계 재논의로 시간을 오래 쓰지 말 것
- /home와 /mnt 둘 다 존재할 때 한쪽만 우선 선택하는 기존 동작을 유지하면 안 됨
- app vs CLI로 분기하지 말고, 실제 로그 루트 목록을 기준으로 처리할 것

작업이 끝나면 다음을 정리해줘:
- 어떤 루트 해석 우선순위로 구현했는지
- Codex/Claude dedupe key가 각각 무엇인지
- 남은 리스크가 있으면 무엇인지
```
