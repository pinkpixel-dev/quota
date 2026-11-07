use quota_core::antigravity::local_usage::{normalize_quota_response, project_id, user_agent};
use serde_json::json;

fn quota_payload() -> serde_json::Value {
    json!({
        "groups": [
            {
                "buckets": [
                    { "bucketId": "gemini-5h", "remainingFraction": 0.88, "resetTime": "2027-01-01T00:00:00Z" },
                    { "bucketId": "gemini-weekly", "remainingFraction": 0.74 }
                ]
            },
            {
                "buckets": [
                    { "bucketId": "3p-5h", "remainingFraction": 0.5 },
                    { "bucketId": "3p-weekly", "remainingFraction": 0.31 }
                ]
            }
        ]
    })
}

#[test]
fn keeps_remaining_fraction_as_remaining_without_inverting() {
    // Antigravity is the exception among these providers. It reports what is
    // left, not what was used, so nothing here inverts.
    let usage = normalize_quota_response(&quota_payload(), None);

    assert_eq!(usage.provider, "antigravity");
    assert_eq!(usage.windows[0].remaining_percent, Some(88));
    assert_eq!(usage.windows[1].remaining_percent, Some(74));
}

#[test]
fn the_compact_token_shows_only_the_gemini_pair() {
    // Four numbers stop being a compact token, and Gemini is the reason people
    // run Antigravity.
    let usage = normalize_quota_response(&quota_payload(), None);

    assert_eq!(usage.compact_token().as_deref(), Some("5h 88% · Wk 74%"));
}

#[test]
fn the_third_party_windows_are_still_carried_in_full() {
    let usage = normalize_quota_response(&quota_payload(), None);

    assert_eq!(usage.windows.len(), 4);
    assert_eq!(usage.windows[2].label, "3p 5h");
    assert_eq!(usage.windows[2].remaining_percent, Some(50));
    assert_eq!(usage.windows[3].label, "3p Wk");
    assert_eq!(usage.windows[3].remaining_percent, Some(31));
}

#[test]
fn buckets_are_found_regardless_of_which_group_holds_them() {
    // The grouping carries no meaning Quota needs, so a payload that puts every
    // bucket in one group must read the same.
    let raw = json!({
        "groups": [{
            "buckets": [
                { "bucketId": "3p-weekly", "remainingFraction": 0.31 },
                { "bucketId": "gemini-weekly", "remainingFraction": 0.74 },
                { "bucketId": "gemini-5h", "remainingFraction": 0.88 }
            ]
        }]
    });

    let usage = normalize_quota_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(88));
    assert_eq!(usage.windows[1].remaining_percent, Some(74));
    assert_eq!(usage.windows[3].remaining_percent, Some(31));
}

#[test]
fn a_fraction_sent_as_a_string_still_parses() {
    let raw = json!({
        "groups": [{ "buckets": [{ "bucketId": "gemini-5h", "remainingFraction": "0.42" }] }]
    });

    let usage = normalize_quota_response(&raw, None);

    assert_eq!(usage.windows[0].remaining_percent, Some(42));
}

#[test]
fn an_empty_payload_produces_four_windows_with_no_numbers() {
    let usage = normalize_quota_response(&json!({}), None);

    assert_eq!(usage.windows.len(), 4);
    assert!(usage.windows.iter().all(|w| w.remaining_percent.is_none()));
    assert_eq!(usage.compact_token(), None);
}

#[test]
fn an_unknown_bucket_id_is_ignored() {
    let raw = json!({
        "groups": [{
            "buckets": [
                { "bucketId": "gemini-5h", "remainingFraction": 0.9 },
                { "bucketId": "something-new", "remainingFraction": 0.1 }
            ]
        }]
    });

    let usage = normalize_quota_response(&raw, None);

    assert_eq!(usage.windows.len(), 4);
    assert_eq!(usage.windows[0].remaining_percent, Some(90));
}

#[test]
fn reset_times_parse_from_rfc3339_and_from_epochs() {
    let raw = json!({
        "groups": [{
            "buckets": [
                { "bucketId": "gemini-5h", "remainingFraction": 1, "resetTime": "2027-01-01T00:00:00Z" },
                { "bucketId": "gemini-weekly", "remainingFraction": 1, "resetTime": 1_798_761_600_000i64 },
                { "bucketId": "3p-5h", "remainingFraction": 1, "resetTime": "1798761600" }
            ]
        }]
    });

    let usage = normalize_quota_response(&raw, None);

    assert_eq!(usage.windows[0].reset_at, Some(1_798_761_600));
    assert_eq!(usage.windows[1].reset_at, Some(1_798_761_600));
    assert_eq!(usage.windows[2].reset_at, Some(1_798_761_600));
}

#[test]
fn a_zero_reset_time_is_treated_as_unknown() {
    let raw = json!({
        "groups": [{ "buckets": [{ "bucketId": "gemini-5h", "remainingFraction": 1, "resetTime": 0 }] }]
    });

    let usage = normalize_quota_response(&raw, None);

    assert_eq!(usage.windows[0].reset_at, None);
}

#[test]
fn the_project_id_reads_from_every_shape_google_sends() {
    assert_eq!(
        project_id(&json!({ "cloudaicompanionProject": "proj-bare" })).as_deref(),
        Some("proj-bare")
    );
    assert_eq!(
        project_id(&json!({ "cloudaicompanionProject": { "id": "proj-id" } })).as_deref(),
        Some("proj-id")
    );
    assert_eq!(
        project_id(&json!({ "cloudaicompanionProject": { "projectId": "proj-field" } })).as_deref(),
        Some("proj-field")
    );
    assert_eq!(project_id(&json!({})), None);
    assert_eq!(project_id(&json!({ "cloudaicompanionProject": "  " })), None);
}

#[test]
fn the_quota_summary_call_uses_the_cli_user_agent() {
    // Without it, Google leaves the Gemini group out of the quota summary (issue #17).
    assert!(user_agent("v1internal:retrieveUserQuotaSummary").starts_with("antigravity/cli/"));
}

#[test]
fn the_other_code_assist_calls_keep_the_ide_user_agent() {
    assert!(user_agent("v1internal:fetchAvailableModels").starts_with("antigravity/1."));
    let load = user_agent("v1internal:loadCodeAssist");
    assert!(load.starts_with("antigravity/1.") && load.contains("google-api-nodejs-client/"));
}

#[test]
fn a_disabled_bucket_reports_zero_rather_than_a_stale_fraction() {
    // Once the paired weekly limit is exhausted Antigravity disables the
    // 5-hour bucket, but the raw fraction can still read 1.
    let raw = json!({
        "groups": [{
            "buckets": [
                { "bucketId": "gemini-5h", "remainingFraction": 1, "disabled": true, "resetTime": "2027-01-01T00:00:00Z" },
                { "bucketId": "gemini-weekly", "remainingFraction": 0.74 }
            ]
        }]
    });

    let usage = normalize_quota_response(&raw, None);

    assert_eq!(usage.windows[0].label, "5h");
    assert_eq!(usage.windows[0].remaining_percent, Some(0));
    assert_eq!(usage.windows[0].reset_at, Some(1_798_761_600));
    assert_eq!(usage.windows[1].remaining_percent, Some(74));
}

#[test]
fn a_bucket_without_the_disabled_flag_keeps_its_fraction() {
    // A payload from before the flag existed must read exactly as it did.
    let usage = normalize_quota_response(&quota_payload(), None);

    assert_eq!(usage.windows[0].remaining_percent, Some(88));
}
