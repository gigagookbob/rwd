# Design: 업데이트 체크 캐싱

## 목적

모든 rwd 커맨드 실행 시 업데이트 알림을 표시하되, GitHub API 호출을 24시간에 1회로 제한하여 CLI 응답 속도를 유지한다.

## 현재 상태

- `notify_if_update_available()`가 `rwd today`에서만 호출됨
- 매번 GitHub API를 블로킹 호출 (1~2초 지연)
- `rwd init`, `rwd config`, `rwd summary`에서는 알림 없음

## 설계

### 접근 방식

gh (GitHub CLI) 스타일: 스탬프 파일에 마지막 체크 시각 + 최신 버전을 캐싱하고, 24시간 이내면 API 호출을 스킵한다.

### 변경 파일

#### 1. `cache.rs` — 업데이트 체크 캐시 구조체/함수 추가

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckCache {
    /// 마지막 체크 시각. chrono의 serde feature로 자동 직렬화/역직렬화.
    /// DateTime<Utc>를 사용하여 TTL 비교 시 타입 일치를 보장합니다.
    pub checked_at: chrono::DateTime<chrono::Utc>,
    /// 그때 확인한 최신 버전 (예: "0.6.0")
    pub latest_version: String,
}
```

- `update_check_path() -> Result<PathBuf, CacheError>` — `~/.rwd/cache/update-check.json` 경로 반환 (기존 `cache_path()` 패턴과 일관)
- `load_update_check() -> Option<UpdateCheckCache>` — 파일 읽기. 파일 없음/손상 시 None 반환 (조용히 재생성)
- `save_update_check(cache: &UpdateCheckCache) -> Result<(), CacheError>` — 파일 쓰기
- 기존 `cache_dir()` (`~/.rwd/cache/`)을 재사용

#### 2. `update.rs` — `notify_if_update_available()` 캐시 로직 적용

```
1. cache::load_update_check() 호출
2. 캐시가 있고, checked_at이 24시간 이내? (UTC 기준 비교 — 타임존 변경에 영향받지 않도록)
   → YES: 캐시의 latest_version과 CURRENT_VERSION 비교 → 알림
   → NO:  GitHub API 호출 → cache::save_update_check() → 알림
3. API 호출 실패 시 → 조용히 무시 (현재와 동일)
4. 캐시 저장 실패 시 → 조용히 무시 (API 실패와 동일하게 처리)
```

#### 3. `main.rs` — 모든 커맨드에서 호출

- `match` 분기 앞에서 `notify_if_update_available()` 호출
- `Commands::Update`일 때는 스킵 (중복 알림 방지)
- `run_today()` 내부의 기존 호출 제거

```rust
if !matches!(args.command, Commands::Update) {
    update::notify_if_update_available().await;
}
```

### 변경하지 않는 것

- `check_latest_version()` — 기존 GitHub API 호출 로직 그대로
- `run_update()` — 기존 셀프 업데이트 로직 그대로 (단, 성공 시 캐시를 현재 버전으로 갱신하여 업데이트 직후 잘못된 알림 방지)
- 알림 방식 — 기존 stderr 출력 유지
- 옵트아웃 — 지금은 추가하지 않음 (필요 시 나중에)

### 캐시 파일 예시

`~/.rwd/cache/update-check.json`:

```json
{
  "checked_at": "2026-03-17T05:30:00+00:00",
  "latest_version": "0.6.0"
}
```

### 추후 개선 (이 설계 범위 밖)

- 네트워크 타임아웃 설정 (현재 reqwest 기본값 사용)

## 조사 배경

gh, npm, rustup, brew, claude 등 주요 CLI 도구 조사 결과:
- 대부분 24시간 주기 캐싱 + 논블로킹 체크 패턴 사용
- rwd는 하루 수회 사용이므로 24시간 블로킹 체크(캐시 히트 시 즉시)가 적정선
- 완전 논블로킹(백그라운드 프로세스)은 현재 사용 빈도 대비 과잉 설계
