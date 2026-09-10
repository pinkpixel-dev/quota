use base64::Engine;
use quota_core::codex::local::{read_local_credentials_at, LocalCredentialError};
use std::fs;

fn temp_file(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "quota-codex-local-{}-{}",
        std::process::id(),
        name
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("auth.json");
    fs::write(&path, body).expect("write auth file");
    path
}

/// Builds a token shaped like the id token Codex stores: three dot-separated
/// parts, with base64url claims in the middle.
fn fake_id_token(claims: serde_json::Value) -> String {
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&claims).expect("encode claims"));
    format!("header.{}.signature", payload)
}

#[test]
fn reads_an_oauth_auth_file() {
    let id_token = fake_id_token(serde_json::json!({
        "email": "person@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acct-from-jwt",
            "chatgpt_plan_type": "pro"
        }
    }));
    let path = temp_file(
        "valid",
        &format!(
            r#"{{"tokens":{{"id_token":"{}","access_token":"codex-access-token","refresh_token":"codex-refresh"}}}}"#,
            id_token
        ),
    );

    let credentials = read_local_credentials_at(&path).expect("read credentials");

    assert_eq!(credentials.access_token, "codex-access-token");
    assert_eq!(credentials.account_id.as_deref(), Some("acct-from-jwt"));
    assert_eq!(credentials.email.as_deref(), Some("person@example.com"));
    assert_eq!(credentials.plan.as_deref(), Some("pro"));
}

#[test]
fn prefers_the_jwt_account_id_over_the_tokens_field() {
    let id_token = fake_id_token(serde_json::json!({
        "https://api.openai.com/auth": { "account_id": "acct-preferred" }
    }));
    let path = temp_file(
        "account-id-precedence",
        &format!(
            r#"{{"tokens":{{"id_token":"{}","access_token":"token","account_id":"acct-fallback"}}}}"#,
            id_token
        ),
    );

    let credentials = read_local_credentials_at(&path).expect("read credentials");

    assert_eq!(credentials.account_id.as_deref(), Some("acct-preferred"));
}

#[test]
fn falls_back_to_the_tokens_account_id_when_the_jwt_is_unusable() {
    let path = temp_file(
        "bad-jwt",
        r#"{"tokens":{"id_token":"not-a-jwt","access_token":"token","account_id":"acct-fallback"}}"#,
    );

    let credentials = read_local_credentials_at(&path).expect("read credentials");

    assert_eq!(credentials.account_id.as_deref(), Some("acct-fallback"));
    assert_eq!(credentials.access_token, "token");
}

#[test]
fn missing_file_is_a_distinct_error() {
    let path = std::env::temp_dir().join(format!(
        "quota-codex-local-{}-absent/auth.json",
        std::process::id()
    ));

    match read_local_credentials_at(&path) {
        Err(LocalCredentialError::NotFound) => {}
        other => panic!("expected NotFound, got {:?}", other.map(|_| "credentials")),
    }
}

#[test]
fn an_api_key_login_reports_that_it_has_no_usage_window() {
    let path = temp_file("api-key", r#"{"OPENAI_API_KEY":"sk-not-a-real-key"}"#);

    match read_local_credentials_at(&path) {
        Err(LocalCredentialError::ApiKeyOnly) => {}
        other => panic!("expected ApiKeyOnly, got {:?}", other.map(|_| "credentials")),
    }
}

#[test]
fn an_empty_access_token_is_malformed() {
    let path = temp_file("empty-token", r#"{"tokens":{"access_token":"   "}}"#);

    match read_local_credentials_at(&path) {
        Err(LocalCredentialError::Malformed(_)) => {}
        other => panic!("expected Malformed, got {:?}", other.map(|_| "credentials")),
    }
}

#[test]
fn credentials_struct_debug_redacts_access_token() {
    let path = temp_file(
        "redaction",
        r#"{"tokens":{"access_token":"super-secret-codex-token"}}"#,
    );

    let credentials = read_local_credentials_at(&path).expect("read credentials");
    let rendered = format!("{:?}", credentials);

    assert!(
        !rendered.contains("super-secret-codex-token"),
        "Debug output leaked the access token: {}",
        rendered
    );
    assert!(rendered.contains("<redacted>"), "got: {}", rendered);
}

#[test]
fn a_parse_failure_never_quotes_the_token() {
    // Truncated JSON, with the token as the last thing serde saw. serde_json's
    // own message would quote surrounding input, so only line and column may
    // be reported.
    let path = temp_file(
        "leaky-parse",
        r#"{"tokens":{"access_token":"super-secret-codex-token"#,
    );

    match read_local_credentials_at(&path) {
        Err(err) => {
            let rendered = format!("{} {:?}", err, err);
            assert!(
                !rendered.contains("super-secret-codex-token"),
                "parse error leaked the token: {}",
                rendered
            );
        }
        Ok(_) => panic!("expected the truncated file to fail parsing"),
    }
}
