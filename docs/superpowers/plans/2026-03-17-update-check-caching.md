# Update Check Caching Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Display update notifications on all rwd commands while limiting GitHub API calls to once every 24 hours.

**Architecture:** Add update check cache struct/functions to `cache.rs`, modify `notify_if_update_available()` in `update.rs` to use the cache, and call it from `main.rs` for all commands.

**Tech Stack:** Rust, chrono (serde feature), serde_json

**Spec:** `docs/superpowers/specs/2026-03-17-update-check-caching-design.md`

---

## Chunk 1: cache.rs Extension + update.rs Cache Integration + main.rs Hookup

### File Structure

| File | Change | Role |
|------|--------|------|
| `src/cache.rs` | Modify | Add `UpdateCheckCache` struct + `update_check_path()` / `load_update_check()` / `save_update_check()` |
| `src/update.rs` | Modify | Apply cache logic to `notify_if_update_available()` + update cache on `run_update()` success |
| `src/main.rs` | Modify | Call before `match`, skip for `Commands::Update`, remove call inside `run_today()` |

---

### Task 1: Write UpdateCheckCache tests in cache.rs

**Files:**
- Modify: `src/cache.rs`

- [ ] **Step 1: Write serialization/deserialization test for UpdateCheckCache**

Add a new `#[cfg(test)] mod tests` block at the end of `src/cache.rs` (this file currently has no test block):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_check_cache_serialization_deserialization_returns_same_data() {
        let cache = UpdateCheckCache {
            checked_at: chrono::Utc::now(),
            latest_version: "0.6.0".to_string(),
        };
        let json = serde_json::to_string_pretty(&cache).expect("serialization should succeed");
        let loaded: UpdateCheckCache = serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(loaded.latest_version, "0.6.0");
    }
}
```

- [ ] **Step 2: Verify test failure**

Run: `cargo test test_update_check_cache_serialization -- --nocapture`
Expected: compile error — `UpdateCheckCache` doesn't exist yet

- [ ] **Step 3: Add UpdateCheckCache struct**

Add to `src/cache.rs` below existing `use` statements, before `TodayCache`:

```rust
/// Update check cache. Stored at ~/.rwd/cache/update-check.json.
/// Uses chrono's serde feature for automatic DateTime JSON conversion (already enabled in Cargo.toml).
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckCache {
    /// Last check timestamp. Stored in UTC to be unaffected by timezone changes.
    /// Uses DateTime<Utc> because: TTL comparison requires subtraction between same types.
    /// chrono's Sub implementation requires both sides to be the same type (DateTime<Utc> - DateTime<Utc>).
    pub checked_at: chrono::DateTime<chrono::Utc>,
    /// Latest version found at that time (e.g., "0.6.0")
    pub latest_version: String,
}
```

- [ ] **Step 4: Verify test passes**

Run: `cargo test test_update_check_cache_serialization -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/cache.rs
git commit -m "feat: add UpdateCheckCache struct"
```

---

### Task 2: Implement and test load/save functions in cache.rs

**Files:**
- Modify: `src/cache.rs`

- [ ] **Step 1: Write save/load roundtrip test**

```rust
#[test]
fn test_update_check_save_and_load_roundtrip() {
    let temp_dir = std::env::temp_dir().join("rwd_test_update_check");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("create directory");
    let path = temp_dir.join("update-check.json");

    let cache = UpdateCheckCache {
        checked_at: chrono::Utc::now(),
        latest_version: "0.7.0".to_string(),
    };

    save_update_check_to(&cache, &path).expect("save should succeed");
    let loaded = load_update_check_from(&path);
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().latest_version, "0.7.0");

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_update_check_load_returns_none_when_file_missing() {
    let path = std::env::temp_dir().join("rwd_test_nonexistent_update_check.json");
    let _ = std::fs::remove_file(&path);
    let loaded = load_update_check_from(&path);
    assert!(loaded.is_none());
}
```

- [ ] **Step 2: Verify test failure**

Run: `cargo test test_update_check -- --nocapture`
Expected: compile error — functions don't exist yet

- [ ] **Step 3: Implement update_check_path(), load/save functions**

Add to `src/cache.rs`:

```rust
/// Update check cache file path: ~/.rwd/cache/update-check.json
/// Same pattern as existing cache_path() but uses a fixed filename instead of a date.
fn update_check_path() -> Result<PathBuf, CacheError> {
    Ok(cache_dir()?.join("update-check.json"))
}

/// Loads the update check cache. Returns None on missing/corrupted files.
/// Same pattern as existing load_cache(): cache miss is normal operation, so use Option.
pub fn load_update_check() -> Option<UpdateCheckCache> {
    let path = update_check_path().ok()?;
    load_update_check_from(&path)
}

/// Loads update check cache from a specified path. Separated for test path injection.
fn load_update_check_from(path: &std::path::Path) -> Option<UpdateCheckCache> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Saves the update check cache. Same pattern as existing save_cache().
pub fn save_update_check(cache: &UpdateCheckCache) -> Result<(), CacheError> {
    let path = update_check_path()?;
    save_update_check_to(cache, &path)
}

/// Saves update check cache to a specified path. Separated for test path injection.
fn save_update_check_to(cache: &UpdateCheckCache, path: &std::path::Path) -> Result<(), CacheError> {
    let json = serde_json::to_string_pretty(cache)?;
    std::fs::write(path, json)?;
    Ok(())
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test test_update_check -- --nocapture`
Expected: all 3 tests PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy`
Expected: 0 warnings

- [ ] **Step 6: Commit**

```bash
git add src/cache.rs
git commit -m "feat: add UpdateCheckCache load/save functions"
```

---

### Task 3: Apply cache logic to update.rs

**Files:**
- Modify: `src/update.rs`

- [ ] **Step 1: Refactor notify_if_update_available()**

Replace `notify_if_update_available()` in `src/update.rs` with:

```rust
/// Prints an update notice if a newer version is available.
/// Uses cached results if a check was performed within the last 24 hours (skips GitHub API call).
pub async fn notify_if_update_available() {
    // If cache exists and is within 24 hours, use cached version for notification
    if let Some(cached) = crate::cache::load_update_check() {
        let now = chrono::Utc::now();
        let interval = chrono::Duration::hours(24);
        if now - cached.checked_at < interval {
            print_update_notice(&cached.latest_version);
            return;
        }
    }

    // Cache miss or expired — call GitHub API
    if let Ok(latest) = check_latest_version().await {
        // Save result to cache (silently ignore failures)
        let cache = crate::cache::UpdateCheckCache {
            checked_at: chrono::Utc::now(),
            latest_version: latest.clone(),
        };
        let _ = crate::cache::save_update_check(&cache);

        print_update_notice(&latest);
    }
}

/// Prints an update notice if the latest version differs from the current version.
fn print_update_notice(latest_version: &str) {
    if latest_version != CURRENT_VERSION {
        eprintln!(
            "New version available: v{latest_version} (current: v{CURRENT_VERSION})"
        );
        eprintln!("Update: rwd update\n");
    }
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: compilation success

- [ ] **Step 3: Run clippy**

Run: `cargo clippy`
Expected: 0 warnings

- [ ] **Step 4: Commit**

```bash
git add src/update.rs
git commit -m "feat: apply 24-hour cache logic to notify_if_update_available"
```

---

### Task 4: main.rs hookup

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add notify_if_update_available() call before match**

Add before `match args.command {` in `src/main.rs`:

```rust
// Display update notification before every command.
// Commands::Update is skipped since it does its own version check (prevents duplicate notifications).
if !matches!(args.command, Commands::Update) {
    update::notify_if_update_available().await;
}
```

- [ ] **Step 2: Remove existing call inside run_today()**

Delete the following line from `run_today()` in `src/main.rs`:

```rust
update::notify_if_update_available().await;
```

- [ ] **Step 3: Verify build**

Run: `cargo build`
Expected: compilation success

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: all tests PASS

- [ ] **Step 5: Run clippy**

Run: `cargo clippy`
Expected: 0 warnings

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: display update notification on all commands"
```

---

### Task 5: Update cache on run_update() success

**Files:**
- Modify: `src/update.rs`

- [ ] **Step 1: Add cache update code at the end of run_update()**

Add just before `eprintln!("rwd v{latest} update complete!");` in `run_update()` in `src/update.rs`:

```rust
    // Update cache after successful update to prevent "new version available" notification on next run.
    let cache = crate::cache::UpdateCheckCache {
        checked_at: chrono::Utc::now(),
        latest_version: latest.clone(),
    };
    let _ = crate::cache::save_update_check(&cache);
```

- [ ] **Step 2: Verify build**

Run: `cargo build`
Expected: compilation success

- [ ] **Step 3: Run clippy**

Run: `cargo clippy`
Expected: 0 warnings

- [ ] **Step 4: Commit**

```bash
git add src/update.rs
git commit -m "feat: update check cache refresh on successful run_update"
```
