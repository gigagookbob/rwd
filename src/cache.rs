// Caches today's analysis results to avoid redundant LLM calls.
// Reuses previous results when entry count hasn't changed.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::analyzer::insight::AnalysisResult;

pub type CacheError = Box<dyn std::error::Error>;

/// Update check cache. Stored at ~/.rwd/cache/update-check.json.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckCache {
    pub checked_at: chrono::DateTime<chrono::Utc>,
    /// Latest version found at check time (e.g., "0.6.0")
    pub latest_version: String,
}

/// Cached analysis data for a single day.
#[derive(Debug, Serialize, Deserialize)]
pub struct TodayCache {
    pub date: String,
    pub claude_entry_count: usize,
    pub codex_session_count: usize,
    /// Total parsed Codex entries for the target date.
    ///
    /// Added after `codex_session_count` to avoid false cache hits when
    /// conversations continue inside existing sessions.
    #[serde(default)]
    pub codex_entry_count: usize,
    /// Local-day UTC window key for cache safety across timezone changes.
    ///
    /// Format: "<start_utc_rfc3339>..<end_utc_rfc3339>".
    /// Legacy caches may not have this field.
    #[serde(default)]
    pub timezone_window_key: Option<String>,
    /// Per-source analysis results.
    pub sources: Vec<(String, AnalysisResult)>,
}

/// Returns a timezone-aware cache identity for a local date.
///
/// The key is derived from the local [00:00, next 00:00) window represented in UTC.
/// It changes when timezone/daylight-saving rules change.
pub fn timezone_window_key(date: NaiveDate) -> Option<String> {
    let window = crate::parser::local_date_to_utc_window(date).ok()?;
    Some(format!(
        "{}..{}",
        window.start_utc.to_rfc3339(),
        window.end_utc.to_rfc3339()
    ))
}

/// Returns whether a cached result is compatible with the current local timezone rules.
///
/// Legacy caches without timezone metadata are treated as incompatible when the current
/// key can be computed, forcing one-time refresh to prevent wrong cache reuse.
pub fn is_timezone_compatible(cache: &TodayCache, date: NaiveDate) -> bool {
    match timezone_window_key(date) {
        Some(current_key) => cache.timezone_window_key.as_deref() == Some(current_key.as_str()),
        // If the current timezone window cannot be resolved, fall back to existing behavior.
        None => true,
    }
}

/// Returns cache directory path: ~/.rwd/cache/
fn cache_dir() -> Result<PathBuf, CacheError> {
    let home = dirs::home_dir().ok_or(crate::messages::error::HOME_DIR_NOT_FOUND)?;
    let dir = home.join(".rwd").join("cache");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Cache file path: ~/.rwd/cache/today-YYYY-MM-DD.json
fn cache_path(date: NaiveDate) -> Result<PathBuf, CacheError> {
    Ok(cache_dir()?.join(format!("today-{date}.json")))
}

/// Loads cache. Returns None on missing file or parse failure (cache miss is normal).
pub fn load_cache(date: NaiveDate) -> Option<TodayCache> {
    let path = cache_path(date).ok()?;
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Saves cache as pretty-printed JSON for easy debugging.
pub fn save_cache(cache: &TodayCache, date: NaiveDate) -> Result<(), CacheError> {
    let path = cache_path(date)?;
    let json = serde_json::to_string_pretty(cache)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Update check cache path: ~/.rwd/cache/update-check.json
fn update_check_path() -> Result<PathBuf, CacheError> {
    Ok(cache_dir()?.join("update-check.json"))
}

/// Loads update check cache. Returns None on missing/corrupted file.
pub fn load_update_check() -> Option<UpdateCheckCache> {
    let path = update_check_path().ok()?;
    load_update_check_from(&path)
}

/// Loads update check cache from a specific path (for testability).
fn load_update_check_from(path: &std::path::Path) -> Option<UpdateCheckCache> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Saves update check cache.
pub fn save_update_check(cache: &UpdateCheckCache) -> Result<(), CacheError> {
    let path = update_check_path()?;
    save_update_check_to(cache, &path)
}

/// Saves update check cache to a specific path (for testability).
fn save_update_check_to(
    cache: &UpdateCheckCache,
    path: &std::path::Path,
) -> Result<(), CacheError> {
    let json = serde_json::to_string_pretty(cache)?;
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_today_cache(date: &str, timezone_window_key: Option<String>) -> TodayCache {
        TodayCache {
            date: date.to_string(),
            claude_entry_count: 0,
            codex_session_count: 0,
            codex_entry_count: 0,
            timezone_window_key,
            sources: Vec::new(),
        }
    }

    #[test]
    fn test_update_check_cache_serialize_deserialize_roundtrip() {
        let cache = UpdateCheckCache {
            checked_at: chrono::Utc::now(),
            latest_version: "0.6.0".to_string(),
        };
        let json = serde_json::to_string_pretty(&cache).expect("serialize");
        let loaded: UpdateCheckCache = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loaded.latest_version, "0.6.0");
    }

    #[test]
    fn test_update_check_save_then_load_roundtrip() {
        let temp_dir = std::env::temp_dir().join("rwd_test_update_check");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("create dir");
        let path = temp_dir.join("update-check.json");

        let cache = UpdateCheckCache {
            checked_at: chrono::Utc::now(),
            latest_version: "0.7.0".to_string(),
        };

        save_update_check_to(&cache, &path).expect("save");
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

    #[test]
    fn test_today_cache_deserialize_legacy_json_defaults_codex_entry_count() {
        let json = r#"{
  "date":"2026-04-11",
  "claude_entry_count":10,
  "codex_session_count":2,
  "sources":[]
}"#;
        let loaded: TodayCache = serde_json::from_str(json).expect("deserialize legacy cache");
        assert_eq!(loaded.codex_entry_count, 0);
        assert!(loaded.timezone_window_key.is_none());
    }

    #[test]
    fn test_timezone_window_key_has_start_and_end() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 11).expect("valid date");
        let key = timezone_window_key(date).expect("timezone window key");
        let parts: Vec<&str> = key.split("..").collect();
        assert_eq!(parts.len(), 2);
        assert!(!parts[0].is_empty());
        assert!(!parts[1].is_empty());
    }

    #[test]
    fn test_is_timezone_compatible_requires_timezone_key() {
        let date = NaiveDate::from_ymd_opt(2026, 4, 11).expect("valid date");
        let Some(current_key) = timezone_window_key(date) else {
            // If timezone window resolution fails, compatibility falls back to true by design.
            let cache = empty_today_cache("2026-04-11", None);
            assert!(is_timezone_compatible(&cache, date));
            return;
        };

        let legacy_cache = empty_today_cache("2026-04-11", None);
        assert!(!is_timezone_compatible(&legacy_cache, date));

        let compatible_cache = empty_today_cache("2026-04-11", Some(current_key.clone()));
        assert!(is_timezone_compatible(&compatible_cache, date));

        let incompatible_cache = empty_today_cache("2026-04-11", Some("wrong".to_string()));
        assert!(!is_timezone_compatible(&incompatible_cache, date));
    }
}
