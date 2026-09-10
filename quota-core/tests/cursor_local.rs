use quota_core::cursor::local::{read_local_credentials_at, LocalCredentialError};
use std::fs;

fn temp_file(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "quota-cursor-local-{}-{}",
        std::process::id(),
        name
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("auth.json");
    fs::write(&path, body).expect("write auth file");
    path
}

#[test]
fn reads_the_cli_auth_file() {
    // The shape the Cursor CLI actually writes: flat, camelCase, two keys.
    let path = temp_file(
        "reads",
        r#"{"accessToken":"cursor-access-token","refreshToken":"cursor-refresh-token"}"#,
    );

    let credentials = read_local_credentials_at(&path).expect("read credentials");

    assert_eq!(credentials.access_token, "cursor-access-token");
}

#[test]
fn accepts_a_snake_case_access_token() {
    let path = temp_file("snake", r#"{"access_token":"cursor-access-token"}"#);

    let credentials = read_local_credentials_at(&path).expect("read credentials");

    assert_eq!(credentials.access_token, "cursor-access-token");
}

#[test]
fn missing_file_is_a_distinct_error() {
    let path = std::env::temp_dir().join(format!(
        "quota-cursor-local-{}-absent/auth.json",
        std::process::id()
    ));

    match read_local_credentials_at(&path) {
        Err(LocalCredentialError::NotFound) => {}
        other => panic!("expected NotFound, got {:?}", other),
    }
}

#[test]
fn an_empty_access_token_is_malformed() {
    let path = temp_file("empty", r#"{"accessToken":"   "}"#);

    match read_local_credentials_at(&path) {
        Err(LocalCredentialError::Malformed(_)) => {}
        other => panic!("expected Malformed, got {:?}", other),
    }
}

#[test]
fn credentials_struct_debug_redacts_access_token() {
    // Quota's errors and logs print credential structs. A derived Debug here
    // would put a live Cursor token into a terminal or a bug report.
    let path = temp_file("debug", r#"{"accessToken":"super-secret-token"}"#);
    let credentials = read_local_credentials_at(&path).expect("read credentials");

    let rendered = format!("{:?}", credentials);

    assert!(!rendered.contains("super-secret-token"));
    assert!(rendered.contains("<redacted>"));
}

#[test]
fn a_parse_failure_never_quotes_the_token() {
    // serde's own error message can echo the input it choked on. Only the line
    // and column are kept, so a malformed file cannot leak the token it holds.
    let path = temp_file(
        "malformed",
        r#"{"accessToken":"super-secret-token","#,
    );

    match read_local_credentials_at(&path) {
        Err(error) => {
            let rendered = format!("{} {:?}", error, error);
            assert!(!rendered.contains("super-secret-token"));
        }
        Ok(_) => panic!("expected a parse failure"),
    }
}

#[test]
fn reading_credentials_never_writes_to_the_file() {
    // Cursor's refresh token is shared with the user's own CLI. If Quota ever
    // rewrote this file, it would sign them out of the tool they are using.
    let path = temp_file(
        "readonly",
        r#"{"accessToken":"cursor-access-token","refreshToken":"cursor-refresh-token"}"#,
    );
    let before = fs::read(&path).expect("read file before");
    let modified_before = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .expect("mtime before");

    read_local_credentials_at(&path).expect("read credentials");

    let after = fs::read(&path).expect("read file after");
    let modified_after = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .expect("mtime after");

    assert_eq!(before, after);
    assert_eq!(modified_before, modified_after);
}
