# Learning Guide — rwd

이 프로젝트의 개발자는 Rust 학습자이다. 이 문서는 AI 에이전트가 학습자와 협업할 때 따라야 할 지침을 정의한다.

## 핵심 원칙

개발자가 완성된 코드를 받는 것이 아니라, 코드를 이해하며 만들어가는 것이 목표이다.

## 설명 기준

### Rust 개념 설명 시

- The Rust Programming Language (2024 Edition)의 해당 챕터를 반드시 명시할 것
  - 예: "이것은 ownership 개념입니다 (Rust Book Ch.4 참조)"
  - 온라인 기준: https://doc.rust-lang.org/book/
- Context7 MCP를 사용하여 크레이트 공식 문서를 참조한 뒤 코드를 작성할 것

### 설계 결정 시

- 왜 이 방식을 선택했는지 대안과 함께 설명할 것
- 예:
  > A 방식(Vec 사용)과 B 방식(Iterator 체이닝)이 있습니다.
  > 지금 단계에서는 A가 더 명확하고 이해하기 쉽습니다.
  > 나중에 익숙해지면 B로 리팩터링하는 것도 좋습니다.

## 코드 작성 규칙

### 단계적 구현

- 한 번에 전체를 작성하지 말 것
- 함수 하나 → 빌드 확인 → 설명 → 다음 단계 순서로 진행

### 고급 패턴 사용 제한

아래 패턴은 반드시 사전 설명 후에만 사용할 것:
- 트레이트 바운드 (`where T: Display + Clone`)
- 라이프타임 어노테이션 (`'a`)
- 매크로 정의 (`macro_rules!`)
- 클로저와 고차 함수 (`.map()`, `.filter()`, `.collect()`)
- 제네릭 (`<T>`)

### 금지 패턴

아래 패턴은 이 프로젝트에서 사용하지 않는다:
- `unsafe` 블록
- 고급 매크로 (proc macro 등)
- 복잡한 트레이트 체이닝

## 할루시네이션 방지

- 크레이트 API 사용 전 반드시 Context7 MCP로 공식 문서를 확인할 것
- Rust 개념 설명 시 Rust Book 챕터 번호를 명시하여 교차검증 가능하게 할 것
- 확신이 없는 내용은 "확실하지 않으니 공식 문서를 확인해주세요"라고 명시할 것
- 코드 작성 후 반드시 `cargo build`, `cargo clippy`로 컴파일러 검증을 수행할 것

## 참고 자료

| 자료 | URL | 용도 |
|------|-----|------|
| The Rust Programming Language (2024 Edition) | https://doc.rust-lang.org/book/ | Rust 핵심 개념 |
| Rust Standard Library Docs | https://doc.rust-lang.org/std/ | 표준 라이브러리 API |
| Rust By Example | https://doc.rust-lang.org/rust-by-example/ | 예제 기반 학습 |
| Rust Edition Guide | https://doc.rust-lang.org/edition-guide/ | 에디션 변경사항 |
