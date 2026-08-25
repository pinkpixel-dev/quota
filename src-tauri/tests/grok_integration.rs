use base64::Engine;
use quota_lib::grok::{
    classify_grok_refresh_failure_for_test, import_grok_from_auth_dir_for_test,
    parse_grok_quota_for_test, GrokAccountIndex,
};
use serde_json::json;
use std::fs;
use std::path::Path;

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "quota-{}-{}-{}",
        name,
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn jwt_with_payload(payload: serde_json::Value) -> String {
    let body = serde_json::to_vec(&payload).expect("encode jwt payload");
    format!(
        "header.{}.signature",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(body)
    )
}

fn grok_access_token(email: &str, principal_id: &str, team_id: &str, tier: i64) -> String {
    jwt_with_payload(json!({
        "sub": principal_id,
        "email": email,
        "principal_id": principal_id,
        "team_id": team_id,
        "tier": tier,
        "exp": 1_771_736_400_i64
    }))
}

fn read_index(storage_dir: &Path) -> GrokAccountIndex {
    let raw = fs::read_to_string(storage_dir.join("grok_accounts.json")).expect("read index");
    serde_json::from_str(&raw).expect("parse index")
}

#[test]
fn imports_local_grok_auth_file_without_returning_tokens_in_summary() {
    let auth_dir = unique_temp_dir("grok-auth");
    let storage_dir = unique_temp_dir("grok-storage");
    let access_token = grok_access_token(
        "sizzlebop@example.com",
        "principal_123",
        "team_123",
        4,
    );

    fs::write(
        auth_dir.join("auth.json"),
        serde_json::to_string_pretty(&json!({
            "https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828": {
                "key": access_token,
                "refresh_token": "grok-refresh-token",
                "expires_at": "2026-08-31T21:44:00Z",
                "email": "sizzlebop@example.com",
                "first_name": "Sizzle",
                "user_id": "user_123",
                "principal_id": "principal_123",
                "team_id": "team_123"
            }
        }))
        .expect("encode auth"),
    )
    .expect("write auth");

    let summaries = import_grok_from_auth_dir_for_test(&auth_dir, &storage_dir).expect("import");

    assert_eq!(summaries.len(), 1);
    let summary = &summaries[0];
    assert_eq!(summary.email, "sizzlebop@example.com");
    assert_eq!(summary.display_name, Some("Sizzle".to_string()));
    assert_eq!(summary.user_id, Some("user_123".to_string()));
    assert_eq!(summary.team_id, Some("team_123".to_string()));
    assert_eq!(summary.tier, Some(4));
    assert!(!summary.requires_reauthentication);

    let serialized_summary = serde_json::to_string(summary).expect("serialize summary");
    assert!(!serialized_summary.contains("grok-refresh-token"));
    assert!(!serialized_summary.contains(&access_token));

    let raw_account = fs::read_to_string(
        storage_dir
            .join("grok_accounts")
            .join(format!("{}.json", summary.id)),
    )
    .expect("read stored account");
    assert!(raw_account.contains("grok-refresh-token"));
    assert!(raw_account.contains("https://auth.x.ai"));
    assert!(raw_account.contains("b1a00492-073a-47ea-816f-4c329264a828"));

    let index = read_index(&storage_dir);
    assert_eq!(index.account_ids, vec![summary.id.clone()]);
}

#[test]
fn imports_every_account_in_a_multi_issuer_grok_auth_file() {
    let auth_dir = unique_temp_dir("grok-multi-auth");
    let storage_dir = unique_temp_dir("grok-multi-storage");

    fs::write(
        auth_dir.join("auth.json"),
        serde_json::to_string_pretty(&json!({
            "https://auth.x.ai::client-a": {
                "key": grok_access_token("first@example.com", "principal_a", "team_a", 3),
                "email": "first@example.com",
                "principal_id": "principal_a"
            },
            "https://auth.x.ai::client-b": {
                "key": grok_access_token("second@example.com", "principal_b", "team_b", 4),
                "email": "second@example.com",
                "principal_id": "principal_b"
            }
        }))
        .expect("encode auth"),
    )
    .expect("write auth");

    let mut summaries = import_grok_from_auth_dir_for_test(&auth_dir, &storage_dir).expect("import");
    summaries.sort_by(|left, right| left.email.cmp(&right.email));

    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].email, "first@example.com");
    assert_eq!(summaries[1].email, "second@example.com");
    assert_ne!(summaries[0].id, summaries[1].id);

    let index = read_index(&storage_dir);
    assert_eq!(index.account_ids.len(), 2);
}

#[test]
fn rejects_grok_auth_entries_without_credentials() {
    let auth_dir = unique_temp_dir("grok-empty-auth");
    let storage_dir = unique_temp_dir("grok-empty-storage");

    fs::write(
        auth_dir.join("auth.json"),
        serde_json::to_string_pretty(&json!({
            "https://auth.x.ai::client-a": {
                "email": "first@example.com"
            }
        }))
        .expect("encode auth"),
    )
    .expect("write auth");

    let error = import_grok_from_auth_dir_for_test(&auth_dir, &storage_dir)
        .expect_err("import should fail without an access token");
    assert!(error.contains("access token"));
}

#[test]
fn parses_grok_credit_window_and_monthly_history() {
    let credits = json!({
        "config": {
            "currentPeriod": {
                "type": "USAGE_PERIOD_TYPE_WEEKLY",
                "start": "2026-08-24T21:44:00Z",
                "end": "2026-08-31T21:44:00Z"
            },
            "creditUsagePercent": 8.0,
            "productUsage": [
                { "product": "grok-code", "usagePercent": 8.0 },
                { "product": "grok-4", "usagePercent": 2.0 }
            ],
            "onDemandCap": { "val": 50.0 },
            "onDemandUsed": { "val": 12.5 },
            "prepaidBalance": { "val": 3.25 },
            "subscriptionTier": "SUBSCRIPTION_TIER_X_PREMIUM_PLUS"
        }
    });
    let history = json!({
        "config": {
            "used": { "val": 21.75 },
            "monthlyLimit": { "val": 100.0 },
            "billingPeriodStart": "2026-08-01T00:00:00Z",
            "billingPeriodEnd": "2026-09-01T00:00:00Z"
        }
    });

    let parsed = parse_grok_quota_for_test(&credits, Some(&history)).expect("parse quota");

    assert_eq!(parsed.plan, Some("X Premium Plus".to_string()));
    assert_eq!(parsed.quota.credit_used_percent, Some(8.0));
    assert_eq!(parsed.quota.credit_remaining_percent, Some(92));
    assert_eq!(parsed.quota.period_label, Some("Weekly".to_string()));
    assert_eq!(parsed.quota.period_window_minutes, Some(10080));
    assert_eq!(parsed.quota.monthly_used, Some(21.75));
    assert_eq!(parsed.quota.monthly_limit, Some(100.0));
    assert_eq!(parsed.quota.on_demand_used, Some(12.5));
    assert_eq!(parsed.quota.on_demand_cap, Some(50.0));
    assert_eq!(parsed.quota.prepaid_balance, Some(3.25));
    assert_eq!(parsed.quota.product_usage.len(), 2);
    assert_eq!(parsed.quota.product_usage[0].product, "grok-code");
    assert_eq!(parsed.quota.product_usage[0].remaining_percent, 92);
}

#[test]
fn derives_grok_credit_usage_from_products_when_the_summary_percent_is_missing() {
    let credits = json!({
        "config": {
            "productUsage": [
                { "product": "grok-code", "usagePercent": 15.0 },
                { "product": "grok-4", "usagePercent": 42.0 }
            ]
        }
    });

    let parsed = parse_grok_quota_for_test(&credits, None).expect("parse quota");

    assert_eq!(parsed.quota.credit_used_percent, Some(42.0));
    assert_eq!(parsed.quota.credit_remaining_percent, Some(58));
    assert_eq!(parsed.quota.monthly_limit, None);
    assert_eq!(parsed.quota.on_demand_cap, None);
}

#[test]
fn classifies_rejected_grok_refresh_tokens_without_exposing_response_bodies() {
    let body = r#"{"error":"invalid_grant","secret":"must-not-leak"}"#;
    let (message, requires_reauthentication) = classify_grok_refresh_failure_for_test(401, body);

    assert!(requires_reauthentication);
    assert_eq!(
        message,
        "Grok authorization is no longer valid. Reconnect Grok to continue."
    );
    assert!(!message.contains("must-not-leak"));

    let (temporary_message, temporary_requires_reauthentication) =
        classify_grok_refresh_failure_for_test(503, body);
    assert!(!temporary_requires_reauthentication);
    assert!(!temporary_message.contains("must-not-leak"));
}
