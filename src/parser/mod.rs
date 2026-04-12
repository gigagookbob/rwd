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

fn utc_window_for_date_in_timezone<Tz: TimeZone>(
    local_date: NaiveDate,
    timezone: &Tz,
) -> Result<UtcDateWindow, ParseError> {
    let local_start = timezone
        .with_ymd_and_hms(local_date.year(), local_date.month(), local_date.day(), 0, 0, 0)
        .earliest()
        .ok_or_else(|| format!("Could not resolve local midnight for {local_date}"))?;

    let next_date = local_date
        .succ_opt()
        .ok_or_else(|| format!("Could not compute next date from {local_date}"))?;

    let local_end = timezone
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

fn utc_dates_for_local_date_in_timezone<Tz: TimeZone>(
    local_date: NaiveDate,
    timezone: &Tz,
) -> Result<Vec<NaiveDate>, ParseError> {
    let window = utc_window_for_date_in_timezone(local_date, timezone)?;
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

/// Builds a UTC half-open window [local 00:00, next local 00:00) for a date.
pub fn local_date_to_utc_window(local_date: NaiveDate) -> Result<UtcDateWindow, ParseError> {
    utc_window_for_date_in_timezone(local_date, &Local)
}

/// Returns all UTC calendar dates touched by the given local date window.
pub fn utc_dates_for_local_date(local_date: NaiveDate) -> Result<Vec<NaiveDate>, ParseError> {
    utc_dates_for_local_date_in_timezone(local_date, &Local)
}

pub use claude::{
    dedupe_entries as dedupe_claude_entries, discover_claude_log_roots, filter_entries_by_date,
    list_project_dirs_in_root, list_session_files, parse_jsonl_file, summarize_entries,
};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, FixedOffset, TimeZone};
    use chrono_tz::America::New_York;

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

    #[test]
    fn test_local_date_to_utc_window_is_half_open() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 4, 11).expect("valid date");
        let window = local_date_to_utc_window(date).expect("utc window");

        let just_before_start = window.start_utc - Duration::nanoseconds(1);
        let just_before_end = window.end_utc - Duration::nanoseconds(1);

        assert!(!window.contains(just_before_start));
        assert!(window.contains(window.start_utc));
        assert!(window.contains(just_before_end));
        assert!(!window.contains(window.end_utc));
    }

    #[test]
    fn test_utc_dates_for_local_date_in_utc_plus_14() {
        let tz = FixedOffset::east_opt(14 * 3600).expect("UTC+14");
        let date = chrono::NaiveDate::from_ymd_opt(2026, 4, 11).expect("valid date");

        let window = utc_window_for_date_in_timezone(date, &tz).expect("utc window");
        assert_eq!(
            window.start_utc,
            chrono::Utc.with_ymd_and_hms(2026, 4, 10, 10, 0, 0).unwrap()
        );
        assert_eq!(
            window.end_utc,
            chrono::Utc.with_ymd_and_hms(2026, 4, 11, 10, 0, 0).unwrap()
        );

        let dates = utc_dates_for_local_date_in_timezone(date, &tz).expect("utc dates");
        assert_eq!(
            dates,
            vec![
                chrono::NaiveDate::from_ymd_opt(2026, 4, 10).expect("date"),
                chrono::NaiveDate::from_ymd_opt(2026, 4, 11).expect("date"),
            ]
        );
    }

    #[test]
    fn test_utc_dates_for_local_date_in_utc_minus_12() {
        let tz = FixedOffset::west_opt(12 * 3600).expect("UTC-12");
        let date = chrono::NaiveDate::from_ymd_opt(2026, 4, 11).expect("valid date");

        let window = utc_window_for_date_in_timezone(date, &tz).expect("utc window");
        assert_eq!(
            window.start_utc,
            chrono::Utc.with_ymd_and_hms(2026, 4, 11, 12, 0, 0).unwrap()
        );
        assert_eq!(
            window.end_utc,
            chrono::Utc.with_ymd_and_hms(2026, 4, 12, 12, 0, 0).unwrap()
        );

        let dates = utc_dates_for_local_date_in_timezone(date, &tz).expect("utc dates");
        assert_eq!(
            dates,
            vec![
                chrono::NaiveDate::from_ymd_opt(2026, 4, 11).expect("date"),
                chrono::NaiveDate::from_ymd_opt(2026, 4, 12).expect("date"),
            ]
        );
    }

    #[test]
    fn test_utc_window_duration_is_23_hours_on_dst_start_day() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 8).expect("valid date");
        let window = utc_window_for_date_in_timezone(date, &New_York).expect("utc window");
        assert_eq!(window.end_utc - window.start_utc, Duration::hours(23));
    }

    #[test]
    fn test_utc_window_duration_is_25_hours_on_dst_end_day() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).expect("valid date");
        let window = utc_window_for_date_in_timezone(date, &New_York).expect("utc window");
        assert_eq!(window.end_utc - window.start_utc, Duration::hours(25));
    }
}
