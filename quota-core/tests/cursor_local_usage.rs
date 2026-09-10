use quota_core::cursor::local_usage::normalize_usage_response;
use serde_json::json;

#[test]
fn inverts_total_percent_used_into_remaining_percent() {
    // The Cursor API reports how much of the plan has been used. Everything
    // downstream, including the sidebar token, reads remaining. If this test
    // fails, the sidebar is telling users the opposite of the truth.
    let raw = json!({
        "individualUsage": {
            "plan": { "totalPercentUsed": 29 }
        }
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.provider, "cursor");
    assert_eq!(usage.windows.len(), 1);
    assert_eq!(usage.windows[0].label, "Plan");
    assert_eq!(usage.windows[0].remaining_percent, Some(71));
}

#[test]
fn accepts_the_snake_case_payload_shape() {
    let raw = json!({
        "individual_usage": {
            "plan": { "total_percent_used": 40 }
        }
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(60));
}

#[test]
fn falls_back_to_the_flat_plan_usage_block() {
    let raw = json!({ "planUsage": { "totalPercentUsed": 10 } });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(90));
}

#[test]
fn a_fully_used_plan_reports_zero_rather_than_a_negative() {
    // Cursor can report over 100 percent once on-demand spending kicks in.
    let raw = json!({
        "individualUsage": {
            "plan": { "totalPercentUsed": 137 }
        }
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(0));
}

#[test]
fn reads_the_billing_cycle_end_as_the_window_reset() {
    let raw = json!({
        "individualUsage": { "plan": { "totalPercentUsed": 5 } },
        "billingCycleEnd": "2026-10-01T00:00:00Z"
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].reset_at, Some(1790812800));
}

#[test]
fn a_missing_plan_block_produces_a_window_with_no_number() {
    let raw = json!({ "somethingElse": true });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows.len(), 1);
    assert_eq!(usage.windows[0].remaining_percent, None);
    assert_eq!(usage.compact_token(), None);
}

#[test]
fn renders_the_compact_token_the_sidebar_shows() {
    let raw = json!({
        "individualUsage": {
            "plan": { "totalPercentUsed": 29 }
        }
    });

    let usage = normalize_usage_response(&raw, Some("free".to_string()));

    assert_eq!(usage.account_label.as_deref(), Some("free"));
    assert_eq!(usage.compact_token().as_deref(), Some("Plan 71%"));
}
