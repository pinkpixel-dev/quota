//! Covers the text report's window rendering, including the reset durations
//! the Herdr sidebar token deliberately leaves out.

use quota_cli::render::usage_detail;
use quota_core::usage::{ProviderUsage, UsageWindow};

const NOW: i64 = 1_757_500_000;

fn window(label: &str, percent: Option<i32>, reset_at: Option<i64>) -> UsageWindow {
    UsageWindow {
        label: label.to_string(),
        remaining_percent: percent,
        reset_at,
    }
}

fn usage(windows: Vec<UsageWindow>, note: Option<&str>) -> ProviderUsage {
    ProviderUsage {
        provider: "claude".to_string(),
        account_label: None,
        windows,
        note: note.map(str::to_string),
    }
}

#[test]
fn a_window_with_a_reset_shows_how_long_is_left() {
    let report = usage(
        vec![window("5h", Some(47), Some(NOW + 2 * 3600 + 14 * 60))],
        None,
    );
    assert_eq!(
        usage_detail(&report, NOW).as_deref(),
        Some("5h 47% (resets in 2h 14m)")
    );
}

#[test]
fn a_window_without_a_reset_shows_only_the_percent() {
    let report = usage(vec![window("Credits", Some(100), None)], None);
    assert_eq!(usage_detail(&report, NOW).as_deref(), Some("Credits 100%"));
}

#[test]
fn every_window_is_rendered_not_just_the_first_two() {
    let report = usage(
        vec![
            window("5h", Some(47), None),
            window("Wk", Some(36), None),
            window("Mo", Some(80), None),
        ],
        None,
    );
    assert_eq!(
        usage_detail(&report, NOW).as_deref(),
        Some("5h 47% · Wk 36% · Mo 80%")
    );
}

#[test]
fn a_reset_further_out_than_a_day_reads_in_days_and_hours() {
    let report = usage(
        vec![window("Wk", Some(36), Some(NOW + 6 * 86_400 + 3 * 3600))],
        None,
    );
    assert_eq!(
        usage_detail(&report, NOW).as_deref(),
        Some("Wk 36% (resets in 6d 3h)")
    );
}

#[test]
fn a_whole_number_of_hours_drops_the_minutes() {
    let report = usage(vec![window("5h", Some(10), Some(NOW + 3 * 3600))], None);
    assert_eq!(
        usage_detail(&report, NOW).as_deref(),
        Some("5h 10% (resets in 3h)")
    );
}

#[test]
fn a_whole_number_of_days_drops_the_hours() {
    let report = usage(vec![window("Wk", Some(10), Some(NOW + 2 * 86_400))], None);
    assert_eq!(
        usage_detail(&report, NOW).as_deref(),
        Some("Wk 10% (resets in 2d)")
    );
}

#[test]
fn less_than_a_minute_left_still_reads_as_a_minute_not_zero() {
    let report = usage(vec![window("5h", Some(3), Some(NOW + 20))], None);
    assert_eq!(
        usage_detail(&report, NOW).as_deref(),
        Some("5h 3% (resets in 1m)")
    );
}

#[test]
fn a_reset_already_passed_reads_as_due_rather_than_negative() {
    let report = usage(vec![window("5h", Some(0), Some(NOW - 600))], None);
    assert_eq!(
        usage_detail(&report, NOW).as_deref(),
        Some("5h 0% (resets now)")
    );
}

#[test]
fn a_window_with_no_percent_is_skipped_entirely() {
    let report = usage(
        vec![
            window("Credit", None, Some(NOW + 3600)),
            window("Wk", Some(50), None),
        ],
        None,
    );
    assert_eq!(usage_detail(&report, NOW).as_deref(), Some("Wk 50%"));
}

#[test]
fn an_account_with_nothing_to_measure_falls_back_to_its_note() {
    let report = usage(
        vec![window("Credit", None, None)],
        Some("no credit allocation"),
    );
    assert_eq!(
        usage_detail(&report, NOW).as_deref(),
        Some("no credit allocation")
    );
}

#[test]
fn nothing_to_report_and_no_note_renders_nothing() {
    let report = usage(vec![window("Credit", None, None)], None);
    assert_eq!(usage_detail(&report, NOW), None);
}
