use quota_lib::claude::{
    apply_claude_token_response_for_test, build_claude_oauth_start_for_test,
    parse_claude_callback_input_for_test, parse_claude_quota_for_test, ClaudeAccountIndex,
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

fn read_index(storage_dir: &Path) -> ClaudeAccountIndex {
    let raw = fs::read_to_string(storage_dir.join("claude_accounts.json")).expect("read index");
    serde_json::from_str(&raw).expect("parse index")
}

#[test]
fn builds_claude_oauth_start_with_pkce_and_manual_callback() {
    let start = build_claude_oauth_start_for_test("login_123", "state_123", "verifier_123")
        .expect("build oauth start");

    assert_eq!(start.login_id, "login_123");
    assert_eq!(
        start.callback_url,
        "https://platform.claude.com/oauth/code/callback"
    );
    assert!(start
        .auth_url
        .starts_with("https://claude.com/cai/oauth/authorize?"));
    assert!(start
        .auth_url
        .contains("client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e"));
    assert!(start.auth_url.contains("response_type=code"));
    assert!(start.auth_url.contains("state=state_123"));
    assert!(start.auth_url.contains("code_challenge_method=S256"));
    assert!(start.auth_url.contains("user%3Aprofile"));
    assert!(start.auth_url.contains("user%3Asessions%3Aclaude_code"));
    assert!(!start.auth_url.contains("verifier_123"));
}

#[test]
fn parses_claude_callback_url_or_raw_code() {
    let parsed = parse_claude_callback_input_for_test(
        "https://platform.claude.com/oauth/code/callback?code=code_123%23state_456",
    )
    .expect("parse callback url");

    assert_eq!(parsed.0, "code_123");
    assert_eq!(parsed.1, Some("state_456".to_string()));

    let parsed = parse_claude_callback_input_for_test("code_789#state_abc").expect("parse code");
    assert_eq!(parsed.0, "code_789");
    assert_eq!(parsed.1, Some("state_abc".to_string()));
}

#[test]
fn parses_claude_usage_into_remaining_percentages() {
    let raw = json!({
        "five_hour": {
            "utilization": 25.0,
            "resets_at": "2026-06-25T22:30:00Z"
        },
        "seven_day": {
            "utilization": 80,
            "resets_at": 1771718400000i64
        },
        "seven_day_sonnet": {
            "utilization": 10,
            "resets_at": 1771718400
        },
        "extra_usage": {
            "is_enabled": true,
            "utilization": 50,
            "resets_at": 1771736400,
            "used_credits": 120,
            "monthly_limit": 1000
        }
    });

    let quota = parse_claude_quota_for_test(&raw);

    assert_eq!(quota.five_hour_remaining_percent, Some(75));
    assert!(quota.five_hour_reset_at.is_some());
    assert_eq!(quota.weekly_remaining_percent, Some(20));
    assert_eq!(quota.weekly_reset_at, Some(1771718400));
    assert_eq!(quota.weekly_sonnet_remaining_percent, Some(90));
    assert_eq!(quota.weekly_sonnet_reset_at, Some(1771718400));
    assert_eq!(quota.extra_usage_remaining_percent, Some(50));
    assert_eq!(quota.extra_usage_reset_at, Some(1771736400));
    assert_eq!(quota.extra_usage_used_cents, Some(120));
    assert_eq!(quota.extra_usage_limit_cents, Some(1000));
}

#[test]
fn applies_claude_oauth_token_response_without_returning_tokens_in_summary() {
    let storage_dir = unique_temp_dir("claude-oauth-storage");
    let response = json!({
        "access_token": "claude-access-token",
        "refresh_token": "claude-refresh-token",
        "token_type": "Bearer",
        "expires_in": 3600,
        "scope": "user:profile user:inference user:sessions:claude_code",
        "account": {
            "uuid": "account-uuid",
            "email_address": "sizzle@example.com"
        },
        "organization": {
            "uuid": "org-uuid",
            "name": "Pink Pixel"
        }
    });
    let profile = json!({
        "account": {
            "uuid": "account-uuid",
            "email_address": "sizzle@example.com",
            "display_name": "Sizzle",
            "avatar_url": "https://example.com/avatar.png"
        },
        "organization": {
            "uuid": "org-uuid",
            "name": "Pink Pixel",
            "organization_type": "claude_pro"
        }
    });

    let summary = apply_claude_token_response_for_test(&storage_dir, &response, Some(&profile))
        .expect("apply token response");

    assert_eq!(summary.email, "sizzle@example.com");
    assert_eq!(summary.account_uuid, Some("account-uuid".to_string()));
    assert_eq!(summary.organization_uuid, Some("org-uuid".to_string()));
    assert_eq!(summary.organization_name, Some("Pink Pixel".to_string()));
    assert_eq!(summary.plan_type, Some("Pro".to_string()));

    let serialized_summary = serde_json::to_string(&summary).expect("serialize summary");
    assert!(!serialized_summary.contains("claude-access-token"));
    assert!(!serialized_summary.contains("claude-refresh-token"));

    let raw_account = fs::read_to_string(
        storage_dir
            .join("claude_accounts")
            .join(format!("{}.json", summary.id)),
    )
    .expect("read stored account");
    assert!(raw_account.contains("claude-access-token"));
    assert!(raw_account.contains("claude-refresh-token"));

    let index = read_index(&storage_dir);
    assert_eq!(index.account_ids, vec![summary.id]);
}
