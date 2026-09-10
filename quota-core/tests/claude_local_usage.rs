use quota_core::claude::local_usage::normalize_usage_response;
use serde_json::json;

#[test]
fn normalizes_five_hour_and_weekly_into_ordered_windows() {
    let raw = json!({
        "five_hour": {
            "utilization": 11.0,
            "resets_at": "2026-09-10T09:59:59.709197+00:00"
        },
        "seven_day": {
            "utilization": 43.0,
            "resets_at": "2026-09-14T10:59:59.709221+00:00"
        }
    });

    let usage = normalize_usage_response(&raw, Some("pro".to_string()));

    assert_eq!(usage.provider, "claude");
    assert_eq!(usage.account_label.as_deref(), Some("pro"));
    assert_eq!(usage.windows.len(), 2);
    assert_eq!(usage.windows[0].label, "5h");
    assert_eq!(usage.windows[0].remaining_percent, Some(89));
    assert_eq!(usage.windows[1].label, "Wk");
    assert_eq!(usage.windows[1].remaining_percent, Some(57));
}

#[test]
fn renders_the_compact_token_the_sidebar_shows() {
    let raw = json!({
        "five_hour": { "utilization": 11.0 },
        "seven_day": { "utilization": 43.0 }
    });

    let usage = normalize_usage_response(&raw, None);
    assert_eq!(usage.compact_token().as_deref(), Some("5h 89% · Wk 57%"));
}

#[test]
fn a_missing_block_produces_a_window_with_no_number() {
    let raw = json!({ "five_hour": { "utilization": 11.0 } });

    let usage = normalize_usage_response(&raw, None);
    assert_eq!(usage.windows[1].label, "Wk");
    assert_eq!(usage.windows[1].remaining_percent, None);
    assert_eq!(usage.compact_token().as_deref(), Some("5h 89%"));
}

#[test]
fn extra_model_blocks_are_ignored() {
    let raw = json!({
        "five_hour": { "utilization": 11.0 },
        "seven_day": { "utilization": 43.0 },
        "seven_day_opus": { "utilization": 88.0 },
        "seven_day_sonnet": { "utilization": 2.0 }
    });

    let usage = normalize_usage_response(&raw, None);
    assert_eq!(usage.windows.len(), 2);
    assert_eq!(usage.compact_token().as_deref(), Some("5h 89% · Wk 57%"));
}
