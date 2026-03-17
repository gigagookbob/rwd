# 업데이트 체크 캐싱 구현 계획

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 모든 rwd 커맨드에서 업데이트 알림을 표시하되, GitHub API 호출을 24시간에 1회로 제한한다.

**Architecture:** `cache.rs`에 업데이트 체크 캐시 구조체/함수를 추가하고, `update.rs`의 `notify_if_update_available()`이 캐시를 활용하도록 수정한다. `main.rs`에서 모든 커맨드 전에 호출한다.

**Tech Stack:** Rust, chrono (serde feature), serde_json

**Spec:** `docs/superpowers/specs/2026-03-17-update-check-caching-design.md`

---

## Chunk 1: cache.rs 확장 + update.rs 캐시 적용 + main.rs 연동

### 파일 구조

| 파일 | 변경 | 역할 |
|------|------|------|
| `src/cache.rs` | 수정 | `UpdateCheckCache` 구조체 + `update_check_path()` / `load_update_check()` / `save_update_check()` 추가 |
| `src/update.rs` | 수정 | `notify_if_update_available()`에 캐시 로직 적용 + `run_update()` 성공 시 캐시 갱신 |
| `src/main.rs` | 수정 | `match` 앞에서 호출, `Commands::Update` 스킵, `run_today()` 내부 호출 제거 |

---

### Task 1: cache.rs에 UpdateCheckCache 테스트 작성

**Files:**
- Modify: `src/cache.rs`

- [ ] **Step 1: UpdateCheckCache 직렬화/역직렬화 테스트 작성**

`src/cache.rs` 파일 끝에 `#[cfg(test)] mod tests` 블록을 새로 추가 (현재 이 파일에는 테스트 블록이 없음):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_check_cache_직렬화_역직렬화_동일_데이터_반환() {
        let cache = UpdateCheckCache {
            checked_at: chrono::Utc::now(),
            latest_version: "0.6.0".to_string(),
        };
        let json = serde_json::to_string_pretty(&cache).expect("직렬화 성공");
        let loaded: UpdateCheckCache = serde_json::from_str(&json).expect("역직렬화 성공");
        assert_eq!(loaded.latest_version, "0.6.0");
    }
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test test_update_check_cache_직렬화 -- --nocapture`
Expected: 컴파일 에러 — `UpdateCheckCache`가 아직 없음

- [ ] **Step 3: UpdateCheckCache 구조체 추가**

`src/cache.rs`에서 기존 `use` 문 아래, `TodayCache` 앞에 추가:

```rust
/// 업데이트 체크 캐시. ~/.rwd/cache/update-check.json에 저장.
/// chrono의 serde feature를 활용하여 DateTime을 JSON으로 자동 변환합니다 (Cargo.toml에 이미 활성화됨).
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckCache {
    /// 마지막 체크 시각. UTC 기준으로 저장하여 타임존 변경에 영향받지 않도록 합니다.
    /// DateTime<Utc>를 사용하는 이유: TTL 비교 시 동일한 타입끼리 뺄셈해야 하기 때문입니다.
    /// chrono의 Sub 구현은 양쪽 타입이 같아야 동작합니다 (DateTime<Utc> - DateTime<Utc>).
    pub checked_at: chrono::DateTime<chrono::Utc>,
    /// 그때 확인한 최신 버전 (예: "0.6.0")
    pub latest_version: String,
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test test_update_check_cache_직렬화 -- --nocapture`
Expected: PASS

- [ ] **Step 5: 커밋**

```bash
git add src/cache.rs
git commit -m "feat: UpdateCheckCache 구조체 추가"
```

---

### Task 2: cache.rs에 load/save 함수 테스트 및 구현

**Files:**
- Modify: `src/cache.rs`

- [ ] **Step 1: save/load 왕복 테스트 작성**

```rust
#[test]
fn test_update_check_save_and_load_왕복() {
    let temp_dir = std::env::temp_dir().join("rwd_test_update_check");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("디렉토리 생성");
    let path = temp_dir.join("update-check.json");

    let cache = UpdateCheckCache {
        checked_at: chrono::Utc::now(),
        latest_version: "0.7.0".to_string(),
    };

    save_update_check_to(&cache, &path).expect("저장 성공");
    let loaded = load_update_check_from(&path);
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().latest_version, "0.7.0");

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_update_check_load_파일없으면_none() {
    let path = std::env::temp_dir().join("rwd_test_nonexistent_update_check.json");
    let _ = std::fs::remove_file(&path);
    let loaded = load_update_check_from(&path);
    assert!(loaded.is_none());
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cargo test test_update_check -- --nocapture`
Expected: 컴파일 에러 — 함수들이 아직 없음

- [ ] **Step 3: update_check_path(), load/save 함수 구현**

`src/cache.rs`에 추가:

```rust
/// 업데이트 체크 캐시 파일 경로: ~/.rwd/cache/update-check.json
/// 기존 cache_path()와 동일한 패턴이지만, 날짜가 아닌 고정 파일명을 사용합니다.
fn update_check_path() -> Result<PathBuf, CacheError> {
    Ok(cache_dir()?.join("update-check.json"))
}

/// 업데이트 체크 캐시를 로드합니다. 파일 없음/손상 시 None을 반환합니다.
/// 기존 load_cache()와 동일한 패턴: 캐시 미스는 정상 동작이므로 Option으로 처리.
pub fn load_update_check() -> Option<UpdateCheckCache> {
    let path = update_check_path().ok()?;
    load_update_check_from(&path)
}

/// 지정 경로에서 업데이트 체크 캐시를 로드합니다. 테스트에서 경로를 주입할 수 있도록 분리.
fn load_update_check_from(path: &std::path::Path) -> Option<UpdateCheckCache> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// 업데이트 체크 캐시를 저장합니다. 기존 save_cache()와 동일한 패턴.
pub fn save_update_check(cache: &UpdateCheckCache) -> Result<(), CacheError> {
    let path = update_check_path()?;
    save_update_check_to(cache, &path)
}

/// 지정 경로에 업데이트 체크 캐시를 저장합니다. 테스트에서 경로를 주입할 수 있도록 분리.
fn save_update_check_to(cache: &UpdateCheckCache, path: &std::path::Path) -> Result<(), CacheError> {
    let json = serde_json::to_string_pretty(cache)?;
    std::fs::write(path, json)?;
    Ok(())
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test test_update_check -- --nocapture`
Expected: 3개 테스트 모두 PASS

- [ ] **Step 5: clippy 확인**

Run: `cargo clippy`
Expected: warning 0개

- [ ] **Step 6: 커밋**

```bash
git add src/cache.rs
git commit -m "feat: UpdateCheckCache load/save 함수 추가"
```

---

### Task 3: update.rs에 캐시 로직 적용

**Files:**
- Modify: `src/update.rs`

- [ ] **Step 1: notify_if_update_available() 리팩토링**

`src/update.rs`의 `notify_if_update_available()` 함수를 아래로 교체:

```rust
/// 현재 버전보다 새 버전이 있으면 안내 메시지를 출력합니다.
/// 24시간 이내에 이미 체크했으면 캐시된 결과를 사용합니다 (GitHub API 호출 스킵).
pub async fn notify_if_update_available() {
    // 캐시가 있고, 24시간 이내면 캐시된 버전으로 알림
    if let Some(cached) = crate::cache::load_update_check() {
        let now = chrono::Utc::now();
        let interval = chrono::Duration::hours(24);
        if now - cached.checked_at < interval {
            print_update_notice(&cached.latest_version);
            return;
        }
    }

    // 캐시 미스 또는 만료 — GitHub API 호출
    if let Ok(latest) = check_latest_version().await {
        // 결과를 캐시에 저장 (실패해도 조용히 무시)
        let cache = crate::cache::UpdateCheckCache {
            checked_at: chrono::Utc::now(),
            latest_version: latest.clone(),
        };
        let _ = crate::cache::save_update_check(&cache);

        print_update_notice(&latest);
    }
}

/// 최신 버전이 현재 버전과 다르면 업데이트 안내를 출력합니다.
fn print_update_notice(latest_version: &str) {
    if latest_version != CURRENT_VERSION {
        eprintln!(
            "새 버전이 있습니다: v{latest_version} (현재: v{CURRENT_VERSION})"
        );
        eprintln!("업데이트: rwd update\n");
    }
}
```

- [ ] **Step 2: 빌드 확인**

Run: `cargo build`
Expected: 컴파일 성공

- [ ] **Step 3: clippy 확인**

Run: `cargo clippy`
Expected: warning 0개

- [ ] **Step 4: 커밋**

```bash
git add src/update.rs
git commit -m "feat: notify_if_update_available에 24시간 캐시 로직 적용"
```

---

### Task 4: main.rs 연동

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: match 앞에서 notify_if_update_available() 호출 추가**

`src/main.rs`에서 `match args.command {` 앞에 추가:

```rust
// 모든 커맨드 실행 전에 업데이트 알림을 표시합니다.
// Commands::Update는 자체적으로 버전 체크를 하므로 스킵합니다 (중복 알림 방지).
if !matches!(args.command, Commands::Update) {
    update::notify_if_update_available().await;
}
```

- [ ] **Step 2: run_today() 내부의 기존 호출 제거**

`src/main.rs`의 `run_today()` 함수에서 아래 줄을 삭제:

```rust
update::notify_if_update_available().await;
```

- [ ] **Step 3: 빌드 확인**

Run: `cargo build`
Expected: 컴파일 성공

- [ ] **Step 4: 전체 테스트 확인**

Run: `cargo test`
Expected: 모든 테스트 PASS

- [ ] **Step 5: clippy 확인**

Run: `cargo clippy`
Expected: warning 0개

- [ ] **Step 6: 커밋**

```bash
git add src/main.rs
git commit -m "feat: 모든 커맨드에서 업데이트 알림 표시"
```

---

### Task 5: run_update() 성공 시 캐시 갱신

**Files:**
- Modify: `src/update.rs`

- [ ] **Step 1: run_update() 끝에 캐시 갱신 코드 추가**

`src/update.rs`의 `run_update()` 함수에서 `eprintln!("rwd v{latest} 업데이트 완료!");` 직전에 추가:

```rust
    // 업데이트 성공 후 캐시를 갱신하여, 다음 실행 시 "새 버전 있음" 알림이 뜨지 않도록 합니다.
    let cache = crate::cache::UpdateCheckCache {
        checked_at: chrono::Utc::now(),
        latest_version: latest.clone(),
    };
    let _ = crate::cache::save_update_check(&cache);
```

- [ ] **Step 2: 빌드 확인**

Run: `cargo build`
Expected: 컴파일 성공

- [ ] **Step 3: clippy 확인**

Run: `cargo clippy`
Expected: warning 0개

- [ ] **Step 4: 커밋**

```bash
git add src/update.rs
git commit -m "feat: run_update 성공 시 업데이트 체크 캐시 갱신"
```
