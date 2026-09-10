use quota_core::codex::local_usage::normalize_usage_response;
use serde_json::json;

#[test]
fn inverts_used_percent_into_remaining_percent() {
    // The Codex API reports how much of each window has been used. Everything
    // downstream, including the sidebar token, reads remaining. If this test
    // fails, the sidebar is telling users the opposite of the truth.
    let raw = json!({
        "plan_type": "pro",
        "rate_limit": {
            "primary_window": { "used_percent": 9, "limit_window_seconds": 18000 },
            "secondary_window": { "used_percent": 37, "limit_window_seconds": 604800 }
        }
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.provider, "codex");
    assert_eq!(usage.account_label.as_deref(), Some("pro"));
    assert_eq!(usage.windows[0].remaining_percent, Some(91));
    assert_eq!(usage.windows[1].remaining_percent, Some(63));
}

#[test]
fn derives_window_labels_from_the_reported_window_length() {
    let raw = json!({
        "rate_limit": {
            "primary_window": { "used_percent": 0, "limit_window_seconds": 18000 },
            "secondary_window": { "used_percent": 0, "limit_window_seconds": 604800 }
        }
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].label, "5h");
    assert_eq!(usage.windows[1].label, "Wk");
}

#[test]
fn an_odd_window_length_keeps_the_fallback_label() {
    // 437 minutes is neither a whole hour nor a whole day. Inventing a label
    // for it would read worse than the default.
    let raw = json!({
        "rate_limit": {
            "primary_window": { "used_percent": 10, "limit_window_seconds": 26220 }
        }
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].label, "5h");
}

#[test]
fn renders_the_compact_token_the_sidebar_shows() {
    let raw = json!({
        "rate_limit": {
            "primary_window": { "used_percent": 9, "limit_window_seconds": 18000 },
            "secondary_window": { "used_percent": 37, "limit_window_seconds": 604800 }
        }
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.compact_token().as_deref(), Some("5h 91% · Wk 63%"));
}

#[test]
fn a_missing_rate_limit_block_produces_windows_with_no_numbers() {
    let usage = normalize_usage_response(&json!({ "plan_type": "plus" }), None);

    assert_eq!(usage.windows.len(), 2);
    assert!(usage.windows.iter().all(|w| w.remaining_percent.is_none()));
    assert_eq!(usage.compact_token(), None);
}

#[test]
fn a_fully_used_window_reports_zero_rather_than_a_negative() {
    let raw = json!({
        "rate_limit": {
            "primary_window": { "used_percent": 140, "limit_window_seconds": 18000 }
        }
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(0));
}

#[test]
fn an_absolute_reset_timestamp_is_used_as_given() {
    let raw = json!({
        "rate_limit": {
            "primary_window": {
                "used_percent": 5,
                "limit_window_seconds": 18000,
                "reset_at": 1_789_041_497i64,
                "reset_after_seconds": 60
            }
        }
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].reset_at, Some(1_789_041_497));
}

#[test]
fn a_relative_reset_is_turned_into_a_timestamp() {
    let raw = json!({
        "rate_limit": {
            "primary_window": {
                "used_percent": 5,
                "limit_window_seconds": 18000,
                "reset_after_seconds": 3600
            }
        }
    });

    let now = chrono::Utc::now().timestamp();
    let usage = normalize_usage_response(&raw, None);
    let reset = usage.windows[0].reset_at.expect("reset timestamp");

    assert!(
        (reset - (now + 3600)).abs() <= 5,
        "expected roughly an hour from now, got {} against {}",
        reset,
        now + 3600
    );
}

#[test]
fn an_explicit_account_label_wins_over_the_payload_plan() {
    let raw = json!({ "plan_type": "plus" });

    let usage = normalize_usage_response(&raw, Some("pro".to_string()));

    assert_eq!(usage.account_label.as_deref(), Some("pro"));
}
