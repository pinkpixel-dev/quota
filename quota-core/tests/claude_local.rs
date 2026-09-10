use quota_core::claude::local::{read_local_credentials_at, LocalCredentialError};
use std::fs;

fn temp_file(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "quota-claude-local-{}-{}",
        std::process::id(),
        name
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(".credentials.json");
    fs::write(&path, body).expect("write credentials");
    path
}

#[test]
fn reads_a_valid_credentials_file() {
    let far_future = 4_102_444_800_000i64;
    let path = temp_file(
        "valid",
        &format!(
            r#"{{"claudeAiOauth":{{"accessToken":"tok","refreshToken":"r","expiresAt":{},"scopes":["user:profile"],"subscriptionType":"pro"}},"organizationUuid":"org-1"}}"#,
            far_future
        ),
    );

    let creds = read_local_credentials_at(&path).expect("parse");
    assert_eq!(creds.access_token, "tok");
    assert_eq!(creds.subscription_type.as_deref(), Some("pro"));
    assert_eq!(creds.organization_uuid.as_deref(), Some("org-1"));
    assert!(!creds.is_expired_at_ms(far_future - 1));
}

#[test]
fn reports_expiry_without_refreshing() {
    let past = 1_000_000_000_000i64;
    let path = temp_file(
        "expired",
        &format!(
            r#"{{"claudeAiOauth":{{"accessToken":"tok","refreshToken":"r","expiresAt":{}}}}}"#,
            past
        ),
    );

    let creds = read_local_credentials_at(&path).expect("parse");
    assert!(creds.is_expired_at_ms(past + 1));
}

#[test]
fn missing_file_is_a_distinct_error() {
    let path = std::env::temp_dir().join("quota-claude-local-does-not-exist/.credentials.json");
    match read_local_credentials_at(&path) {
        Err(LocalCredentialError::NotFound) => {}
        other => panic!("expected NotFound, got {:?}", other),
    }
}

#[test]
fn a_file_without_the_oauth_block_is_malformed() {
    let path = temp_file("empty", r#"{"organizationUuid":"org-1"}"#);
    match read_local_credentials_at(&path) {
        Err(LocalCredentialError::Malformed(_)) => {}
        other => panic!("expected Malformed, got {:?}", other),
    }
}

#[test]
fn the_error_debug_output_never_contains_a_token() {
    let path = temp_file("leak", r#"{"claudeAiOauth":{"accessToken":"super-secret"}}"#);
    let rendered = format!("{:?}", read_local_credentials_at(&path));
    assert!(!rendered.contains("super-secret"));
}
