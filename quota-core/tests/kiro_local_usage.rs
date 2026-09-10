use quota_core::kiro::local_usage::{normalize_usage_response, usage_url_for_profile_arn};
use serde_json::json;

fn breakdown(limit: f64, used: f64) -> serde_json::Value {
    json!({
        "usageBreakdownList": [
            { "type": "CREDIT", "usageLimit": limit, "currentUsage": used }
        ]
    })
}

#[test]
fn computes_remaining_credits_from_the_total_and_the_amount_used() {
    // Kiro reports totals and consumption, never a percentage, so the shared
    // model's remaining percent is computed rather than inverted.
    let usage = normalize_usage_response(&breakdown(500.0, 190.0), None);

    assert_eq!(usage.provider, "kiro");
    assert_eq!(usage.windows.len(), 1);
    assert_eq!(usage.windows[0].label, "Credits");
    assert_eq!(usage.windows[0].remaining_percent, Some(62));
}

#[test]
fn reads_the_estimated_usage_shape_too() {
    let raw = json!({ "estimatedUsage": { "total": 200, "used": 50 } });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(75));
}

#[test]
fn accepts_amounts_sent_as_strings() {
    let raw = json!({ "estimatedUsage": { "total": "200", "used": "50" } });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(75));
}

#[test]
fn a_zero_total_reports_no_number_rather_than_a_full_pool() {
    // Dividing by this would either panic or claim a full allowance for an
    // account that has none.
    let usage = normalize_usage_response(&breakdown(0.0, 0.0), None);

    assert_eq!(usage.windows[0].remaining_percent, None);
    assert_eq!(usage.compact_token(), None);
}

#[test]
fn a_missing_total_reports_no_number() {
    let raw = json!({ "usageBreakdownList": [ { "type": "CREDIT", "currentUsage": 10 } ] });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, None);
}

#[test]
fn a_fully_used_pool_reports_zero_rather_than_a_negative() {
    let usage = normalize_usage_response(&breakdown(100.0, 137.0), None);

    assert_eq!(usage.windows[0].remaining_percent, Some(0));
}

#[test]
fn a_bonus_pool_becomes_its_own_window() {
    let raw = json!({
        "usageBreakdownList": [{
            "type": "CREDIT",
            "usageLimit": 500,
            "currentUsage": 190,
            "freeTrialUsage": { "usageLimit": 100, "currentUsage": 25 }
        }]
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows.len(), 2);
    assert_eq!(usage.windows[0].label, "Credits");
    assert_eq!(usage.windows[1].label, "Bonus");
    assert_eq!(usage.windows[1].remaining_percent, Some(75));
    assert_eq!(
        usage.compact_token().as_deref(),
        Some("Credits 62% · Bonus 75%")
    );
}

#[test]
fn no_bonus_pool_adds_no_second_window() {
    // An absent bonus must not put an empty row into every account's token.
    let usage = normalize_usage_response(&breakdown(500.0, 190.0), None);

    assert_eq!(usage.windows.len(), 1);
    assert_eq!(usage.compact_token().as_deref(), Some("Credits 62%"));
}

#[test]
fn a_zero_sized_bonus_pool_is_not_shown() {
    let raw = json!({
        "usageBreakdownList": [{
            "type": "CREDIT",
            "usageLimit": 500,
            "currentUsage": 190,
            "freeTrialUsage": { "usageLimit": 0, "currentUsage": 0 }
        }]
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows.len(), 1);
}

#[test]
fn prefers_the_credit_pool_over_other_breakdowns() {
    let raw = json!({
        "usageBreakdownList": [
            { "type": "SOMETHING_ELSE", "usageLimit": 10, "currentUsage": 9 },
            { "type": "CREDIT", "usageLimit": 500, "currentUsage": 190 }
        ]
    });

    let usage = normalize_usage_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(62));
}

#[test]
fn routes_the_request_to_the_region_named_in_the_profile_arn() {
    let url = usage_url_for_profile_arn("arn:aws:codewhisperer:eu-central-1:123456789012:profile/ABC");

    assert!(url.starts_with("https://q.eu-central-1.amazonaws.com/getUsageLimits"));
    assert!(url.contains("resourceType=AGENTIC_REQUEST"));
}

#[test]
fn an_unrecognized_region_falls_back_to_the_default_endpoint() {
    let url = usage_url_for_profile_arn("arn:aws:codewhisperer:ap-south-1:123456789012:profile/ABC");

    assert!(url.starts_with("https://q.us-east-1.amazonaws.com/"));
}

#[test]
fn the_profile_arn_is_url_encoded_into_the_query() {
    let url = usage_url_for_profile_arn("arn:aws:codewhisperer:us-east-1:1:profile/ABC");

    assert!(url.contains("profileArn=arn%3Aaws%3Acodewhisperer"));
}
