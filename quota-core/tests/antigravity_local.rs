use quota_core::antigravity::local::{read_local_credentials_in, LocalCredentialError};
use std::fs;

fn temp_home(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "quota-antigravity-local-{}-{}",
        std::process::id(),
        name
    ));
    fs::create_dir_all(&dir).expect("create temp home");
    dir
}

fn write(home: &std::path::Path, file: &str, body: &str) {
    fs::write(home.join(file), body).expect("write file");
}

const NOW_MS: i64 = 1_789_000_000_000;

#[test]
fn reads_a_valid_credentials_file() {
    let home = temp_home("valid");
    write(
        &home,
        "oauth_creds.json",
        &format!(
            r#"{{"access_token":"ya29-access","refresh_token":"1//refresh","expiry_date":{}}}"#,
            NOW_MS + 3_600_000
        ),
    );

    let credentials = read_local_credentials_in(&home).expect("read credentials");

    assert_eq!(credentials.access_token, "ya29-access");
    assert_eq!(credentials.refresh_token.as_deref(), Some("1//refresh"));
    assert!(!credentials.needs_refresh_at_ms(NOW_MS));
}

#[test]
fn a_token_expiring_within_the_minute_needs_refreshing() {
    // A token that dies mid-request is no better than one already dead, so the
    // margin matches the desktop app's.
    let home = temp_home("margin");
    write(
        &home,
        "oauth_creds.json",
        &format!(
            r#"{{"access_token":"ya29-access","expiry_date":{}}}"#,
            NOW_MS + 30_000
        ),
    );

    let credentials = read_local_credentials_in(&home).expect("read credentials");

    assert!(credentials.needs_refresh_at_ms(NOW_MS));
}

#[test]
fn an_expired_token_needs_refreshing() {
    let home = temp_home("expired");
    write(
        &home,
        "oauth_creds.json",
        &format!(
            r#"{{"access_token":"ya29-access","expiry_date":{}}}"#,
            NOW_MS - 86_400_000
        ),
    );

    let credentials = read_local_credentials_in(&home).expect("read credentials");

    assert!(credentials.needs_refresh_at_ms(NOW_MS));
}

#[test]
fn a_file_with_no_expiry_is_treated_as_usable() {
    let home = temp_home("no-expiry");
    write(&home, "oauth_creds.json", r#"{"access_token":"ya29-access"}"#);

    let credentials = read_local_credentials_in(&home).expect("read credentials");

    assert_eq!(credentials.expiry_date_ms, None);
    assert!(!credentials.needs_refresh_at_ms(NOW_MS));
}

#[test]
fn reads_the_signed_in_address_from_the_accounts_file() {
    let home = temp_home("email");
    write(&home, "oauth_creds.json", r#"{"access_token":"ya29-access"}"#);
    write(
        &home,
        "google_accounts.json",
        r#"{"active":"person@example.com"}"#,
    );

    let credentials = read_local_credentials_in(&home).expect("read credentials");

    assert_eq!(credentials.email.as_deref(), Some("person@example.com"));
}

#[test]
fn a_missing_accounts_file_is_not_an_error() {
    // Usage does not depend on knowing the address, so a missing or unreadable
    // accounts file must not fail the read.
    let home = temp_home("no-accounts");
    write(&home, "oauth_creds.json", r#"{"access_token":"ya29-access"}"#);

    let credentials = read_local_credentials_in(&home).expect("read credentials");

    assert_eq!(credentials.email, None);
}

#[test]
fn missing_file_is_a_distinct_error() {
    let home = temp_home("absent");
    let _ = fs::remove_file(home.join("oauth_creds.json"));

    match read_local_credentials_in(&home) {
        Err(LocalCredentialError::NotFound) => {}
        other => panic!("expected NotFound, got {:?}", other.map(|_| "credentials")),
    }
}

#[test]
fn credentials_struct_debug_redacts_both_tokens() {
    let home = temp_home("redaction");
    write(
        &home,
        "oauth_creds.json",
        r#"{"access_token":"super-secret-access","refresh_token":"super-secret-refresh"}"#,
    );

    let credentials = read_local_credentials_in(&home).expect("read credentials");
    let rendered = format!("{:?}", credentials);

    assert!(
        !rendered.contains("super-secret-access"),
        "Debug output leaked the access token: {}",
        rendered
    );
    assert!(
        !rendered.contains("super-secret-refresh"),
        "Debug output leaked the refresh token: {}",
        rendered
    );
}

#[test]
fn a_parse_failure_never_quotes_the_token() {
    let home = temp_home("leaky-parse");
    write(
        &home,
        "oauth_creds.json",
        r#"{"access_token":"super-secret-access"#,
    );

    match read_local_credentials_in(&home) {
        Err(err) => {
            let rendered = format!("{} {:?}", err, err);
            assert!(
                !rendered.contains("super-secret-access"),
                "parse error leaked the token: {}",
                rendered
            );
        }
        Ok(_) => panic!("expected the truncated file to fail parsing"),
    }
}

#[test]
fn reading_never_writes_to_the_credentials_file() {
    // The whole refresh design rests on this file being left alone.
    let home = temp_home("no-writes");
    let body = r#"{"access_token":"ya29-access","refresh_token":"1//refresh","expiry_date":1}"#;
    write(&home, "oauth_creds.json", body);
    let path = home.join("oauth_creds.json");
    let before = fs::metadata(&path).expect("stat").modified().expect("mtime");

    let _ = read_local_credentials_in(&home).expect("read credentials");

    let after = fs::metadata(&path).expect("stat").modified().expect("mtime");
    assert_eq!(before, after, "the credentials file was modified");
    assert_eq!(fs::read_to_string(&path).expect("re-read"), body);
}

// ---------------------------------------------------------------------------
// Keyring source
// ---------------------------------------------------------------------------

use quota_core::antigravity::local::{parse_keyring_secret, KEYRING_SERVICE, KEYRING_USER};

/// Builds an id token shaped like the one the Antigravity CLI stores.
fn fake_id_token(claims: serde_json::Value) -> String {
    use base64::Engine;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(&claims).expect("encode claims"));
    format!("header.{}.signature", payload)
}

fn keyring_secret(expiry: &str) -> String {
    let id_token = fake_id_token(serde_json::json!({
        "email": "person@example.com",
        "aud": "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com"
    }));
    format!(
        r#"{{"auth_method":"consumer","id_token":"{}","token":{{"access_token":"ya29-access","refresh_token":"1//refresh","token_type":"Bearer","expiry":"{}"}}}}"#,
        id_token, expiry
    )
}

#[test]
fn parses_the_keyring_payload_the_cli_writes() {
    let credentials =
        parse_keyring_secret(&keyring_secret("2027-01-01T00:00:00Z")).expect("parse secret");

    assert_eq!(credentials.access_token, "ya29-access");
    assert_eq!(credentials.refresh_token.as_deref(), Some("1//refresh"));
    assert_eq!(credentials.email.as_deref(), Some("person@example.com"));
    assert_eq!(credentials.expiry_date_ms, Some(1_798_761_600_000));
}

#[test]
fn the_keyring_entry_names_the_antigravity_oauth_client() {
    // The CLI's token belongs to Antigravity's own client, while the Gemini
    // file belongs to the Gemini CLI's. Getting this wrong sends the request
    // to the wrong Code Assist host.
    let credentials =
        parse_keyring_secret(&keyring_secret("2027-01-01T00:00:00Z")).expect("parse secret");

    assert_eq!(
        credentials.issued_to_client_id.as_deref(),
        Some("1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com")
    );
}

#[test]
fn the_keyring_expiry_is_rfc3339_not_epoch_millis() {
    // The file source stores epoch milliseconds and the keyring stores RFC 3339.
    // Reading one as the other silently produces a nonsense expiry.
    let credentials =
        parse_keyring_secret(&keyring_secret("2020-01-01T00:00:00Z")).expect("parse secret");

    assert!(credentials.needs_refresh_at_ms(1_789_000_000_000));
}

#[test]
fn a_keyring_payload_with_no_token_block_is_malformed() {
    match parse_keyring_secret(r#"{"auth_method":"consumer"}"#) {
        Err(_) => {}
        Ok(_) => panic!("expected a payload with no token block to fail"),
    }
}

#[test]
fn a_keyring_parse_failure_never_quotes_the_token() {
    match parse_keyring_secret(r#"{"token":{"access_token":"super-secret-access"#) {
        Err(err) => {
            let rendered = format!("{} {:?}", err, err);
            assert!(
                !rendered.contains("super-secret-access"),
                "parse error leaked the token: {}",
                rendered
            );
        }
        Ok(_) => panic!("expected the truncated payload to fail parsing"),
    }
}

#[test]
fn the_keyring_coordinates_match_what_the_cli_writes() {
    // go-keyring on the CLI side writes service "gemini", user "antigravity".
    // These are the lookup keys; a typo means silently finding nothing.
    assert_eq!(KEYRING_SERVICE, "gemini");
    assert_eq!(KEYRING_USER, "antigravity");
}
