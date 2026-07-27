use base64::Engine;
use quota_lib::codex::{
    apply_codex_token_response_for_test, build_codex_oauth_start_for_test,
    classify_codex_refresh_failure_for_test, import_codex_from_auth_dir_for_test,
    parse_codex_quota_for_test, CodexAccountIndex,
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

fn codex_id_token(email: &str, plan: &str, account_id: &str, organization_id: &str) -> String {
    jwt_with_payload(json!({
        "email": email,
        "sub": "user_123",
        "https://api.openai.com/auth": {
            "chatgpt_user_id": "user_123",
            "chatgpt_plan_type": plan,
            "account_id": account_id,
            "organization_id": organization_id
        }
    }))
}

fn read_index(storage_dir: &Path) -> CodexAccountIndex {
    let raw = fs::read_to_string(storage_dir.join("codex_accounts.json")).expect("read index");
    serde_json::from_str(&raw).expect("parse index")
}

#[test]
fn imports_local_codex_oauth_auth_file_without_returning_tokens_in_summary() {
    let auth_dir = unique_temp_dir("codex-auth");
    let storage_dir = unique_temp_dir("codex-storage");
    let id_token = codex_id_token("sizzlebop@example.com", "plus", "acc_123", "org_123");

    fs::write(
        auth_dir.join("auth.json"),
        serde_json::to_string_pretty(&json!({
            "tokens": {
                "id_token": id_token,
                "access_token": "access-token",
                "refresh_token": "refresh-token"
            },
            "last_refresh": 1771718400
        }))
        .expect("encode auth"),
    )
    .expect("write auth");

    let summary = import_codex_from_auth_dir_for_test(&auth_dir, &storage_dir).expect("import");

    assert_eq!(summary.email, "sizzlebop@example.com");
    assert_eq!(summary.plan, Some("plus".to_string()));
    assert_eq!(summary.account_id, Some("acc_123".to_string()));
    assert_eq!(summary.organization_id, Some("org_123".to_string()));
    assert_eq!(summary.auth_mode, "oauth");

    let serialized_summary = serde_json::to_string(&summary).expect("serialize summary");
    assert!(!serialized_summary.contains("access-token"));
    assert!(!serialized_summary.contains("refresh-token"));

    let index = read_index(&storage_dir);
    assert_eq!(index.account_ids, vec![summary.id]);
}

#[test]
fn applies_codex_refresh_response_and_preserves_existing_refresh_token_when_omitted() {
    let auth_dir = unique_temp_dir("codex-refresh-auth");
    let storage_dir = unique_temp_dir("codex-refresh-storage");
    fs::write(
        auth_dir.join("auth.json"),
        serde_json::to_string_pretty(&json!({
            "tokens": {
                "id_token": codex_id_token("sizzlebop@example.com", "plus", "acc_123", "org_123"),
                "access_token": "old-access-token",
                "refresh_token": "old-refresh-token"
            }
        }))
        .expect("encode auth"),
    )
    .expect("write auth");
    let imported = import_codex_from_auth_dir_for_test(&auth_dir, &storage_dir).expect("import");

    let refreshed = apply_codex_token_response_for_test(
        &storage_dir,
        &imported.id,
        &json!({
            "id_token": codex_id_token("sizzlebop@example.com", "pro", "acc_123", "org_123"),
            "access_token": "new-access-token"
        }),
    )
    .expect("apply token response");

    assert_eq!(refreshed.plan, Some("pro".to_string()));

    let raw_account = fs::read_to_string(
        storage_dir
            .join("codex_accounts")
            .join(format!("{}.json", imported.id)),
    )
    .expect("read stored account");
    assert!(raw_account.contains("new-access-token"));
    assert!(raw_account.contains("old-refresh-token"));

    let serialized_summary = serde_json::to_string(&refreshed).expect("serialize summary");
    assert!(!serialized_summary.contains("new-access-token"));
    assert!(!serialized_summary.contains("old-refresh-token"));
}

#[test]
fn builds_codex_oauth_start_with_pkce_state_and_local_callback() {
    let start = build_codex_oauth_start_for_test("login_123", "state_123", "verifier_123")
        .expect("build oauth start");

    assert_eq!(start.login_id, "login_123");
    assert_eq!(start.callback_url, "http://localhost:1455/auth/callback");
    assert!(start
        .auth_url
        .starts_with("https://auth.openai.com/oauth/authorize?"));
    assert!(start
        .auth_url
        .contains("client_id=app_EMoamEEZ73f0CkXaXp7hrann"));
    assert!(start
        .auth_url
        .contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
    assert!(start.auth_url.contains("response_type=code"));
    assert!(start.auth_url.contains("state=state_123"));
    assert!(start.auth_url.contains("code_challenge_method=S256"));
    assert!(!start.auth_url.contains("verifier_123"));
}

#[test]
fn parses_current_codex_primary_weekly_window() {
    let raw = json!({
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": 35,
                "limit_window_seconds": 604800,
                "reset_at": 1771736400
            }
        }
    });

    let parsed = parse_codex_quota_for_test(&raw).expect("parse quota");

    assert_eq!(parsed.plan, Some("plus".to_string()));
    assert_eq!(parsed.quota.hourly_remaining_percent, Some(65));
    assert_eq!(parsed.quota.hourly_window_minutes, Some(10080));
    assert_eq!(parsed.quota.hourly_reset_at, Some(1771736400));
    assert_eq!(parsed.quota.weekly_remaining_percent, None);
    assert_eq!(parsed.quota.weekly_window_minutes, None);
    assert_eq!(parsed.quota.weekly_reset_at, None);
}

#[test]
fn classifies_rejected_codex_refresh_tokens_without_exposing_response_bodies() {
    let body = r#"{"error":"invalid_grant","secret":"must-not-leak"}"#;
    let (message, requires_reauthentication) = classify_codex_refresh_failure_for_test(401, body);

    assert!(requires_reauthentication);
    assert_eq!(
        message,
        "Codex authorization expired. Reauthenticate to continue."
    );
    assert!(!message.contains("must-not-leak"));

    let (temporary_message, temporary_requires_reauthentication) =
        classify_codex_refresh_failure_for_test(503, body);
    assert!(!temporary_requires_reauthentication);
    assert_eq!(
        temporary_message,
        format!(
            "Codex token refresh returned 503 with body length {}",
            body.len()
        )
    );
}
