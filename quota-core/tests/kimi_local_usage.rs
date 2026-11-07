use quota_core::kimi::local_usage::normalize_usage_response;
use serde_json::json;

#[test]
fn converts_all_windows_in_display_order_with_resets_and_two_compact_windows() {
    let usage = normalize_usage_response(&json!({"usages": {
        "limit_month_code": {"used_ratio": 0.5},
        "limit_7d": {"used_ratio": "0.36", "reset_time": "2027-01-01T00:00:00Z"},
        "limit_month_total": {"used_ratio": 0.25},
        "limit_5h": {"used_ratio": 0.18, "reset_time": "2027-01-01T08:00:00.123456789+08:00"}
    }}));
    assert_eq!(usage.provider, "kimi");
    assert_eq!(usage.account_label, None);
    assert_eq!(usage.note, None);
    assert_eq!(
        usage
            .windows
            .iter()
            .map(|w| w.label.as_str())
            .collect::<Vec<_>>(),
        ["5h", "Wk", "Mo", "Code"]
    );
    assert_eq!(
        usage
            .windows
            .iter()
            .map(|w| w.remaining_percent)
            .collect::<Vec<_>>(),
        [Some(82), Some(64), Some(75), Some(50)]
    );
    assert_eq!(usage.windows[0].reset_at, Some(1_798_761_600));
    assert_eq!(usage.windows[1].reset_at, Some(1_798_761_600));
    assert_eq!(usage.windows[2].reset_at, None);
    assert_eq!(usage.compact_token().as_deref(), Some("5h 82% · Wk 64%"));
    let wire = serde_json::to_value(&usage).unwrap();
    assert_eq!(wire["windows"][0]["remainingPercent"], 82);
    assert_eq!(wire["windows"].as_array().unwrap().len(), 4);
}

#[test]
fn skips_missing_or_invalid_windows_instead_of_inventing_full_quota() {
    let usage = normalize_usage_response(&json!({"usages": {
        "limit_5h": {"reset_time": "2027-01-01T00:00:00Z"},
        "limit_7d": {"used_ratio": "unknown"},
        "limit_month_total": {"used_ratio": "0.1", "reset_time": "bad date"},
        "limit_month_code": {"used_ratio": 0.9}
    }}));
    assert_eq!(usage.compact_token().as_deref(), Some("Mo 90% · Code 10%"));
    assert_eq!(usage.windows[0].reset_at, None);
}

#[test]
fn clamps_out_of_range_ratios_and_rounds_remaining_percent() {
    for (ratio, expected) in [(-0.2, 100), (0.0, 100), (0.123, 88), (1.0, 0), (1.5, 0)] {
        let usage =
            normalize_usage_response(&json!({"usages": {"limit_5h": {"used_ratio": ratio}}}));
        assert_eq!(usage.windows[0].remaining_percent, Some(expected));
    }
}

#[test]
fn non_finite_and_other_invalid_ratios_never_become_percentages() {
    for ratio in [
        json!("NaN"),
        json!("inf"),
        json!("-inf"),
        json!(""),
        json!(null),
        json!(true),
        json!({}),
    ] {
        let usage =
            normalize_usage_response(&json!({"usages": {"limit_5h": {"used_ratio": ratio}}}));
        assert!(usage.windows.is_empty());
        assert_eq!(usage.compact_token().as_deref(), Some("No usage data"));
    }
}

#[test]
fn empty_or_legacy_payloads_and_wallet_only_responses_have_an_explicit_note() {
    for raw in [
        json!({}),
        json!(null),
        json!({"usages": []}),
        json!({"usage": {"limit": 100, "used": 20}}),
        json!({"boosterWallet": {"balance": {"amount": 1000}}}),
    ] {
        let usage = normalize_usage_response(&raw);
        assert!(usage.windows.is_empty());
        assert_eq!(usage.note.as_deref(), Some("No usage data"));
    }
}
