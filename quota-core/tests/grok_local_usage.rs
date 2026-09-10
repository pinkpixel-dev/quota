use quota_core::grok::local_usage::normalize_billing_response;
use serde_json::json;

#[test]
fn inverts_credit_usage_percent_into_remaining_percent() {
    // Grok reports how much of the credit allowance has been used. Everything
    // downstream, including the sidebar token, reads remaining. If this test
    // fails, the sidebar is telling users the opposite of the truth.
    let raw = json!({
        "config": {
            "creditUsagePercent": 38.0,
            "subscriptionTier": "SUBSCRIPTION_TIER_X_PLUS"
        }
    });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.provider, "grok");
    assert_eq!(usage.windows.len(), 1);
    assert_eq!(usage.windows[0].label, "Credit");
    assert_eq!(usage.windows[0].remaining_percent, Some(62));
}

#[test]
fn renders_a_single_window_token_with_no_separator() {
    let raw = json!({ "config": { "creditUsagePercent": 38.0 } });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.compact_token().as_deref(), Some("Credit 62%"));
}

#[test]
fn humanizes_the_subscription_tier_the_way_the_desktop_app_does() {
    let raw = json!({
        "config": { "creditUsagePercent": 1.0, "subscriptionTier": "SUBSCRIPTION_TIER_X_PLUS" }
    });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.account_label.as_deref(), Some("X Plus"));
}

#[test]
fn falls_back_to_the_worst_product_when_no_overall_percent_is_sent() {
    // Averaging would hide a product that is already exhausted, so the highest
    // used percent becomes the headline.
    let raw = json!({
        "config": {
            "productUsage": [
                { "product": "grok-code", "usagePercent": 12.0 },
                { "product": "grok-4", "usagePercent": 81.0 },
                { "product": "grok-3", "usagePercent": 40.0 }
            ]
        }
    });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(19));
}

#[test]
fn an_overall_percent_wins_over_product_usage() {
    let raw = json!({
        "config": {
            "creditUsagePercent": 5.0,
            "productUsage": [{ "product": "grok-4", "usagePercent": 81.0 }]
        }
    });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(95));
}

#[test]
fn a_payload_with_no_numbers_produces_no_token() {
    let usage = normalize_billing_response(&json!({ "config": {} }), None);

    assert_eq!(usage.windows.len(), 1);
    assert_eq!(usage.windows[0].remaining_percent, None);
    assert_eq!(usage.compact_token(), None);
}

#[test]
fn a_fully_used_allowance_reports_zero_rather_than_a_negative() {
    let raw = json!({ "config": { "creditUsagePercent": 130.0 } });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(0));
}

#[test]
fn reads_the_period_end_as_the_reset_time() {
    let raw = json!({
        "config": {
            "creditUsagePercent": 10.0,
            "currentPeriod": { "type": "USAGE_PERIOD_TYPE_MONTHLY", "end": "2027-01-01T00:00:00Z" }
        }
    });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.windows[0].reset_at, Some(1_798_761_600));
}

#[test]
fn falls_back_to_the_billing_period_end() {
    let raw = json!({
        "config": { "creditUsagePercent": 10.0, "billingPeriodEnd": "2027-01-01T00:00:00Z" }
    });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.windows[0].reset_at, Some(1_798_761_600));
}

#[test]
fn an_explicit_account_label_wins_over_the_payload_tier() {
    let raw = json!({
        "config": { "creditUsagePercent": 10.0, "subscriptionTier": "SUBSCRIPTION_TIER_X_PLUS" }
    });

    let usage = normalize_billing_response(&raw, Some("Team".to_string()));

    assert_eq!(usage.account_label.as_deref(), Some("Team"));
}

#[test]
fn derives_the_percent_from_used_over_monthly_limit() {
    // The live billing payload carries no creditUsagePercent and no
    // productUsage. This pair is the only usage figure it does send.
    let raw = json!({
        "config": {
            "monthlyLimit": { "val": 200 },
            "used": { "val": 50 }
        }
    });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(75));
    assert_eq!(usage.note, None);
}

#[test]
fn an_overall_percent_wins_over_the_used_pair() {
    let raw = json!({
        "config": {
            "creditUsagePercent": 10,
            "monthlyLimit": { "val": 200 },
            "used": { "val": 100 }
        }
    });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(90));
}

#[test]
fn a_plan_with_no_allocation_says_so_instead_of_showing_nothing() {
    // A free promotional plan reports every figure as zero. Rendering that as a
    // blank pane is indistinguishable from a provider that is broken.
    let raw = json!({
        "config": {
            "monthlyLimit": { "val": 0 },
            "used": { "val": 0 },
            "onDemandCap": { "val": 0 }
        }
    });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, None);
    assert_eq!(usage.note.as_deref(), Some("no credit allocation"));
    assert_eq!(usage.compact_token().as_deref(), Some("no credit allocation"));
}

#[test]
fn a_zero_limit_is_never_divided_by() {
    let raw = json!({
        "config": {
            "monthlyLimit": { "val": 0 },
            "used": { "val": 25 }
        }
    });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, None);
}

#[test]
fn a_payload_without_the_credit_pair_carries_no_note() {
    // Absent fields are not the same as a zero allocation, so this must stay
    // silent rather than claim the plan has nothing allocated.
    let raw = json!({ "config": { "billingPeriodEnd": "2026-10-01T00:00:00Z" } });

    let usage = normalize_billing_response(&raw, None);

    assert_eq!(usage.note, None);
    assert_eq!(usage.compact_token(), None);
}
