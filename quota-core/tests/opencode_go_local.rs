use quota_core::opencode_go::local::{LocalCredentialError, LocalOpenCodeCredentials};
use std::fs;

fn temp_file(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "quota-opencode-go-local-{}-{}",
        std::process::id(),
        name
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("credential.json");
    fs::write(&path, body).expect("write credential file");
    path
}

#[test]
fn reads_the_console_credential_file() {
    // The OpenCode console writes this after `npx @opencode/console login`.
    // Only the access token matters here; the other fields are ignored
    // rather than required, so a console update that adds one breaks nobody.
    let path = temp_file(
        "console",
        r#"{"type":"oauth","server":"https://opencode.ai/console",
            "access":"st-example-token","refresh":"rt-example-token",
            "accountID":"user_01","email":"person@example.com",
            "orgID":"org_01","orgName":"Personal"}"#,
    );

    let credentials = LocalOpenCodeCredentials::read_at(&path).expect("read credentials");

    assert_eq!(credentials.access_token, "st-example-token");
}

#[test]
fn a_credential_without_an_access_token_is_reported_not_parsed() {
    let path = temp_file("no-access", r#"{"type":"oauth","refresh":"rt-only"}"#);

    let error = LocalOpenCodeCredentials::read_at(&path).expect_err("must fail");

    assert!(matches!(error, LocalCredentialError::NoUsableToken));
    assert!(error.to_string().contains("npx @opencode/console login"));
}

#[test]
fn a_malformed_credential_file_says_so_and_quotes_nothing_from_it() {
    // The real file holds a token, and serde's own message quotes the input it
    // failed on. The error must carry only a position.
    let path = temp_file(
        "malformed",
        "{\"access\":\"st-secret-token\" not json at all",
    );

    let error = LocalOpenCodeCredentials::read_at(&path).expect_err("must fail");

    assert!(matches!(error, LocalCredentialError::Malformed(_)));
    assert!(error.to_string().contains("expected shape"));
    assert!(!error.to_string().contains("st-secret-token"));
}

#[test]
fn the_access_token_never_reaches_debug_output() {
    let path = temp_file("debug", r#"{"access":"st-secret-token"}"#);

    let credentials = LocalOpenCodeCredentials::read_at(&path).expect("read credentials");

    let rendered = format!("{:?}", credentials);
    assert!(!rendered.contains("st-secret-token"));
    assert!(rendered.contains("<redacted>"));
}
