// Reads log files and transforms them into structured data.

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone, Utc};

pub mod claude;
pub mod roots;

// Codex submodule: OpenAI Codex CLI session log parsing.
// Types are accessed via parser::codex:: namespace to avoid name conflicts with Claude types.
pub mod codex;

// TODO: Replace with a dedicated error type via thiserror.
pub type ParseError = Box<dyn std::error::Error>;

/// UTC half-open time window [start_utc, end_utc) for one local date.
#[derive(Debug, Clone, Copy)]
pub struct UtcDateWindow {
    pub start_utc: DateTime<Utc>,
    pub end_utc: DateTime<Utc>,
}

impl UtcDateWindow {
    /// Returns true when a UTC timestamp is within [start_utc, end_utc).
    pub fn contains(&self, ts: DateTime<Utc>) -> bool {
        self.start_utc <= ts && ts < self.end_utc
    }
}

/// Builds a UTC half-open window [local 00:00, next local 00:00) for a date.
pub fn local_date_to_utc_window(local_date: NaiveDate) -> Result<UtcDateWindow, ParseError> {
    let local_start = Local
        .with_ymd_and_hms(local_date.year(), local_date.month(), local_date.day(), 0, 0, 0)
        .earliest()
        .ok_or_else(|| format!("Could not resolve local midnight for {local_date}"))?;

    let next_date = local_date
        .succ_opt()
        .ok_or_else(|| format!("Could not compute next date from {local_date}"))?;

    let local_end = Local
        .with_ymd_and_hms(next_date.year(), next_date.month(), next_date.day(), 0, 0, 0)
        .earliest()
        .ok_or_else(|| format!("Could not resolve local midnight for {next_date}"))?;

    let start_utc = local_start.with_timezone(&Utc);
    let end_utc = local_end.with_timezone(&Utc);
    if end_utc <= start_utc {
        return Err(format!("Invalid UTC window for local date {local_date}").into());
    }

    Ok(UtcDateWindow { start_utc, end_utc })
}

/// Returns all UTC calendar dates touched by the given local date window.
pub fn utc_dates_for_local_date(local_date: NaiveDate) -> Result<Vec<NaiveDate>, ParseError> {
    let window = local_date_to_utc_window(local_date)?;
    let mut dates = Vec::new();
    let mut current = window.start_utc.date_naive();
    let last_inclusive = (window.end_utc - chrono::Duration::nanoseconds(1)).date_naive();
    while current <= last_inclusive {
        dates.push(current);
        current = current
            .succ_opt()
            .ok_or_else(|| format!("Could not advance UTC date from {current}"))?;
    }
    Ok(dates)
}

pub use claude::{
    dedupe_entries as dedupe_claude_entries, discover_claude_log_roots, filter_entries_by_date,
    list_project_dirs_in_root, list_session_files, parse_jsonl_file, summarize_entries,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_date_to_utc_window_contains_local_noon() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 4, 11).expect("valid date");
        let window = local_date_to_utc_window(date).expect("utc window");
        let local_noon = chrono::Local
            .with_ymd_and_hms(2026, 4, 11, 12, 0, 0)
            .earliest()
            .expect("local noon");

        assert!(window.contains(local_noon.with_timezone(&chrono::Utc)));
    }

    #[test]
    fn test_utc_dates_for_local_date_has_small_bounded_set() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 4, 11).expect("valid date");
        let dates = utc_dates_for_local_date(date).expect("utc dates");

        assert!(!dates.is_empty());
        assert!(dates.len() <= 3);
    }
}
