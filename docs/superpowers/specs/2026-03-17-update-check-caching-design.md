# Design: Update Check Caching

## Purpose

Display update notifications on every rwd command execution, while limiting GitHub API calls to once every 24 hours to maintain CLI response speed.

## Current State

- `notify_if_update_available()` is only called from `rwd today`
- Makes a blocking GitHub API call every time (1-2 second delay)
- No notifications from `rwd init`, `rwd config`, or `rwd summary`

## Design

### Approach

gh (GitHub CLI) style: cache the last check timestamp + latest version in a stamp file, and skip the API call if within 24 hours.

### Changed Files

#### 1. `cache.rs` — Add update check cache struct/functions

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckCache {
    /// Last check timestamp. Automatically serialized/deserialized via chrono's serde feature.
    /// Uses DateTime<Utc> to ensure type consistency when comparing TTL.
    pub checked_at: chrono::DateTime<chrono::Utc>,
    /// Latest version found at that time (e.g., "0.6.0")
    pub latest_version: String,
}
```

- `update_check_path() -> Result<PathBuf, CacheError>` — returns `~/.rwd/cache/update-check.json` path (consistent with existing `cache_path()` pattern)
- `load_update_check() -> Option<UpdateCheckCache>` — reads the file. Returns None on missing/corrupted files (silently regenerates)
- `save_update_check(cache: &UpdateCheckCache) -> Result<(), CacheError>` — writes the file
- Reuses existing `cache_dir()` (`~/.rwd/cache/`)

#### 2. `update.rs` — Apply cache logic to `notify_if_update_available()`

```
1. Call cache::load_update_check()
2. Cache exists and checked_at is within 24 hours? (UTC comparison — unaffected by timezone changes)
   → YES: Compare cached latest_version with CURRENT_VERSION → notify
   → NO:  Call GitHub API → cache::save_update_check() → notify
3. API call failure → silently ignore (same as current behavior)
4. Cache save failure → silently ignore (treated same as API failure)
```

#### 3. `main.rs` — Call from all commands

- Call `notify_if_update_available()` before the `match` branch
- Skip for `Commands::Update` (prevent duplicate notifications)
- Remove existing call inside `run_today()`

```rust
if !matches!(args.command, Commands::Update) {
    update::notify_if_update_available().await;
}
```

### What Stays Unchanged

- `check_latest_version()` — existing GitHub API call logic remains as-is
- `run_update()` — existing self-update logic remains as-is (however, on success, update the cache to the current version to prevent false notifications right after update)
- Notification method — keep existing stderr output
- Opt-out — not adding it now (can add later if needed)

### Cache File Example

`~/.rwd/cache/update-check.json`:

```json
{
  "checked_at": "2026-03-17T05:30:00+00:00",
  "latest_version": "0.6.0"
}
```

### Future Improvements (Outside This Design's Scope)

- Network timeout configuration (currently using reqwest defaults)

## Research Background

Survey of major CLI tools including gh, npm, rustup, brew, and claude:
- Most use a 24-hour cache cycle + non-blocking check pattern
- Since rwd is used a few times per day, a 24-hour blocking check (instant on cache hit) is the right balance
- Fully non-blocking (background process) is over-engineering for the current usage frequency
