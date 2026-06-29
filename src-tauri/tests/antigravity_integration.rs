use base64::Engine;
use quota_lib::antigravity::{
    apply_antigravity_token_response_for_test, build_antigravity_code_assist_headers_for_test,
    build_antigravity_load_code_assist_payload_for_test, build_antigravity_oauth_start_for_test,
    import_antigravity_from_gemini_home_for_test, parse_antigravity_code_assist_response_for_test,
    parse_antigravity_load_status_for_test, parse_antigravity_quota_for_test,
    record_antigravity_refresh_error_for_test, AntigravityAccountIndex,
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

fn read_index(storage_dir: &Path) -> AntigravityAccountIndex {
    let raw =
        fs::read_to_string(storage_dir.join("antigravity_accounts.json")).expect("read index");
    serde_json::from_str(&raw).expect("parse index")
}

#[test]
fn imports_local_antigravity_credentials_without_returning_tokens_in_summary() {
    let gemini_home = unique_temp_dir("antigravity-gemini-home");
    let storage_dir = unique_temp_dir("antigravity-storage");
    let id_token = jwt_with_payload(json!({
        "email": "sizzlebop@example.com",
        "sub": "google-user-123",
        "name": "Sizzle Bop"
    }));

    fs::write(
        gemini_home.join("oauth_creds.json"),
        serde_json::to_string_pretty(&json!({
            "access_token": "access-token",
            "refresh_token": "refresh-token",
            "id_token": id_token,
            "token_type": "Bearer",
            "scope": "email profile",
            "expiry_date": 1771718400000i64
        }))
        .expect("encode oauth creds"),
    )
    .expect("write oauth creds");
    fs::write(
        gemini_home.join("google_accounts.json"),
        serde_json::to_string_pretty(&json!({
            "active": "sizzlebop@example.com",
            "accounts": {
                "sizzlebop@example.com": {
                    "email": "sizzlebop@example.com"
                }
            }
        }))
        .expect("encode google accounts"),
    )
    .expect("write google accounts");
    fs::write(
        gemini_home.join("settings.json"),
        serde_json::to_string_pretty(&json!({
            "security": {
                "auth": {
                    "selectedType": "oauth-personal"
                }
            }
        }))
        .expect("encode settings"),
    )
    .expect("write settings");

    let summary =
        import_antigravity_from_gemini_home_for_test(&gemini_home, &storage_dir).expect("import");

    assert_eq!(summary.email, "sizzlebop@example.com");
    assert_eq!(summary.auth_id, Some("google-user-123".to_string()));
    assert_eq!(summary.name, Some("Sizzle Bop".to_string()));
    assert_eq!(
        summary.selected_auth_type,
        Some("oauth-personal".to_string())
    );
    assert_eq!(summary.source, "local");

    let serialized_summary = serde_json::to_string(&summary).expect("serialize summary");
    assert!(!serialized_summary.contains("access-token"));
    assert!(!serialized_summary.contains("refresh-token"));
    assert!(!serialized_summary.contains("id_token"));

    let index = read_index(&storage_dir);
    assert_eq!(index.account_ids, vec![summary.id]);
}

#[test]
fn parses_antigravity_quota_buckets_into_remaining_percentages() {
    let raw = json!({
        "groups": [
            {
                "buckets": [
                    {
                        "bucketId": "gemini-5h",
                        "remainingFraction": 0.42,
                        "resetTime": "2026-06-25T16:30:00Z"
                    },
                    {
                        "bucketId": "gemini-weekly",
                        "remainingFraction": 0.8,
                        "resetTime": 1771718400
                    },
                    {
                        "bucketId": "3p-5h",
                        "remainingFraction": "0.25",
                        "resetTime": 1771736400000i64
                    },
                    {
                        "bucketId": "3p-weekly",
                        "remainingFraction": 0,
                        "resetTime": null
                    }
                ]
            }
        ]
    });

    let quota = parse_antigravity_quota_for_test(&raw);

    assert_eq!(quota.gemini_five_hour.remaining_percent, Some(42));
    assert!(quota.gemini_five_hour.reset_at.is_some());
    assert_eq!(quota.gemini_weekly.remaining_percent, Some(80));
    assert_eq!(quota.gemini_weekly.reset_at, Some(1771718400));
    assert_eq!(quota.third_party_five_hour.remaining_percent, Some(25));
    assert_eq!(quota.third_party_five_hour.reset_at, Some(1771736400));
    assert_eq!(quota.third_party_weekly.remaining_percent, Some(0));
    assert_eq!(quota.third_party_weekly.reset_at, None);
}

#[test]
fn builds_antigravity_load_code_assist_payload_with_antigravity_metadata() {
    let payload = build_antigravity_load_code_assist_payload_for_test();

    assert_eq!(payload["mode"], "FULL_ELIGIBILITY_CHECK");
    assert_eq!(payload["metadata"]["ideName"], "antigravity");
    assert_eq!(payload["metadata"]["ideType"], "ANTIGRAVITY");
    assert_eq!(payload["metadata"]["ideVersion"], "1.20.5");
    assert_eq!(payload["metadata"]["pluginVersion"], "quota");
    assert_eq!(payload["metadata"]["updateChannel"], "stable");
    assert_eq!(payload["metadata"]["pluginType"], "GEMINI");
    assert!(payload["metadata"]["platform"].as_str().is_some());
}

#[test]
fn parses_antigravity_ai_credits_from_paid_tier() {
    let raw = json!({
        "cloudaicompanionProject": "project-123",
        "paidTier": {
            "id": "g1-pro-tier",
            "name": "Pro",
            "availableCredits": [
                {
                    "creditType": "GOOGLE_ONE_AI",
                    "creditAmount": "25,000",
                    "minimumCreditAmountForUsage": "50"
                },
                {
                    "creditType": "IGNORED_WITHOUT_AMOUNT"
                }
            ]
        }
    });

    let status = parse_antigravity_load_status_for_test(&raw);

    assert_eq!(status.project_id, Some("project-123".to_string()));
    assert_eq!(status.tier_id, Some("g1-pro-tier".to_string()));
    assert_eq!(status.tier_name, Some("Pro".to_string()));
    assert_eq!(status.credits.len(), 1);
    assert_eq!(status.credits[0].credit_type, "GOOGLE_ONE_AI");
    assert_eq!(status.credits[0].credit_amount, Some("25,000".to_string()));
    assert_eq!(
        status.credits[0].minimum_credit_amount_for_usage,
        Some("50".to_string())
    );
}

#[test]
fn builds_antigravity_oauth_start_with_google_scopes_and_local_callback() {
    let start = build_antigravity_oauth_start_for_test("login_123", "state_123")
        .expect("build oauth start");

    assert_eq!(start.login_id, "login_123");
    assert!(start.callback_url.starts_with("http://127.0.0.1:"));
    assert!(start.callback_url.ends_with("/oauth-callback"));
    assert!(start
        .auth_url
        .starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
    assert!(start.auth_url.contains("response_type=code"));
    assert!(start.auth_url.contains("access_type=offline"));
    assert!(start.auth_url.contains("state=state_123"));
    assert!(start.auth_url.contains(
        "client_id=1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com"
    ));
    assert!(start
        .auth_url
        .contains("https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform"));
    assert!(start
        .auth_url
        .contains("https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fuserinfo.email"));
    assert!(start
        .auth_url
        .contains("https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fuserinfo.profile"));
    assert!(start
        .auth_url
        .contains("https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcclog"));
    assert!(start
        .auth_url
        .contains("https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fexperimentsandconfigs"));
}

#[test]
fn applies_antigravity_oauth_token_response_without_returning_tokens_in_summary() {
    let storage_dir = unique_temp_dir("antigravity-oauth-storage");
    let id_token = jwt_with_payload(json!({
        "email": "oauth-sizzle@example.com",
        "sub": "google-user-oauth",
        "name": "OAuth Sizzle"
    }));

    let summary = apply_antigravity_token_response_for_test(
        &storage_dir,
        &json!({
            "access_token": "oauth-access-token",
            "refresh_token": "oauth-refresh-token",
            "id_token": id_token,
            "token_type": "Bearer",
            "scope": "https://www.googleapis.com/auth/cloud-platform",
            "expires_in": 3600
        }),
    )
    .expect("apply token response");

    assert_eq!(summary.email, "oauth-sizzle@example.com");
    assert_eq!(summary.auth_id, Some("google-user-oauth".to_string()));
    assert_eq!(summary.name, Some("OAuth Sizzle".to_string()));
    assert_eq!(summary.source, "oauth");

    let serialized_summary = serde_json::to_string(&summary).expect("serialize summary");
    assert!(!serialized_summary.contains("oauth-access-token"));
    assert!(!serialized_summary.contains("oauth-refresh-token"));

    let raw_account = fs::read_to_string(
        storage_dir
            .join("antigravity_accounts")
            .join(format!("{}.json", summary.id)),
    )
    .expect("read stored account");
    assert!(raw_account.contains("oauth-access-token"));
    assert!(raw_account.contains("oauth-refresh-token"));
}

#[test]
fn parses_empty_successful_antigravity_code_assist_response_as_empty_object() {
    let parsed = parse_antigravity_code_assist_response_for_test(
        "https://daily-cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
        200,
        "",
    )
    .expect("parse empty response");

    assert_eq!(parsed, json!({}));
}

#[test]
fn reports_antigravity_code_assist_parse_errors_with_response_context() {
    let error = parse_antigravity_code_assist_response_for_test(
        "https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
        200,
        "not json",
    )
    .expect_err("expected parse error");

    assert!(error.contains("Could not parse Antigravity quota response"));
    assert!(error.contains("status=200"));
    assert!(error.contains("body_length=8"));
    assert!(error.contains("not json"));
}

#[test]
fn antigravity_code_assist_headers_do_not_request_unsupported_compression() {
    let headers = build_antigravity_code_assist_headers_for_test(
        "https://daily-cloudcode-pa.googleapis.com/v1internal:loadCodeAssist",
    );

    assert!(headers
        .iter()
        .all(|(name, _)| name.to_lowercase() != "accept-encoding"));
}

#[test]
fn records_antigravity_refresh_errors_on_the_account_summary() {
    let storage_dir = unique_temp_dir("antigravity-error-storage");
    let id_token = jwt_with_payload(json!({
        "email": "error-sizzle@example.com",
        "sub": "google-user-error",
        "name": "Error Sizzle"
    }));

    let summary = apply_antigravity_token_response_for_test(
        &storage_dir,
        &json!({
            "access_token": "oauth-access-token",
            "refresh_token": "oauth-refresh-token",
            "id_token": id_token,
            "token_type": "Bearer",
            "scope": "https://www.googleapis.com/auth/cloud-platform",
            "expires_in": 3600
        }),
    )
    .expect("apply token response");

    let errored = record_antigravity_refresh_error_for_test(
        &storage_dir,
        &summary.id,
        "Could not parse Antigravity quota response: status=200 body_length=8 preview=not json",
    )
    .expect("record refresh error");

    assert_eq!(
        errored.quota_query_last_error,
        Some(
            "Could not parse Antigravity quota response: status=200 body_length=8 preview=not json"
                .to_string()
        )
    );
    assert!(errored.quota_query_last_error_at.is_some());
    assert!(errored.last_used >= summary.last_used);

    let raw_account = fs::read_to_string(
        storage_dir
            .join("antigravity_accounts")
            .join(format!("{}.json", summary.id)),
    )
    .expect("read stored account");
    assert!(raw_account.contains("Could not parse Antigravity quota response"));
}
