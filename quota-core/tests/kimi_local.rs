use quota_core::kimi::local::{
    local_credentials_path, read_local_credentials_at, LocalCredentialError,
};
use serde_json::json;
use std::path::PathBuf;

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str, body: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("quota-kimi-local-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("kimi-code.json"), body).unwrap();
        Self(dir)
    }

    fn path(&self) -> PathBuf {
        self.0.join("kimi-code.json")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn reads_oauth_without_modifying_the_file_and_redacts_debug() {
    let body = json!({
        "access_token": "  secret-access-token  ",
        "refresh_token": "secret-refresh-token",
        "expires_at": chrono::Utc::now().timestamp() + 3600,
        "scope": "kimi-code", "token_type": "Bearer", "expires_in": 3600
    })
    .to_string();
    let fixture = Fixture::new("valid", &body);
    let credentials = read_local_credentials_at(&fixture.path()).unwrap();
    assert_eq!(credentials.access_token, "secret-access-token");
    let debug = format!("{:?}", credentials);
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("secret-access-token"));
    assert!(!debug.contains("secret-refresh-token"));
    assert_eq!(std::fs::read_to_string(fixture.path()).unwrap(), body);
}

#[test]
fn missing_and_unreadable_credentials_are_distinct() {
    let fixture = Fixture::new("missing", "{}");
    std::fs::remove_file(fixture.path()).unwrap();
    assert!(matches!(
        read_local_credentials_at(&fixture.path()),
        Err(LocalCredentialError::NotFound)
    ));
    assert!(matches!(
        read_local_credentials_at(&fixture.0),
        Err(LocalCredentialError::Unreadable(_))
    ));
}

#[test]
fn rejects_missing_and_wrongly_typed_fields_without_leaking_values() {
    for (index, body) in [
        json!({"access_token": "secret-access-token"}).to_string(),
        json!({"access_token": "secret-access-token", "expires_at": "secret-expiry"}).to_string(),
        json!({"access_token": "secret-access-token", "expires_at": 0}).to_string(),
        r#"{"access_token":"secret-access-token"#.to_string(),
        "null".to_string(),
    ]
    .iter()
    .enumerate()
    {
        let fixture = Fixture::new(&format!("malformed-{}", index), body);
        let error = read_local_credentials_at(&fixture.path()).unwrap_err();
        assert!(matches!(error, LocalCredentialError::Malformed(_)));
        let rendered = format!("{} {:?}", error, error);
        assert!(!rendered.contains("secret-access-token"));
        assert!(!rendered.contains("secret-expiry"));
    }
}

#[test]
fn empty_access_token_means_signed_out_without_changing_credentials() {
    // Kimi Code writes this payload when the session is revoked.
    let body = r#"{"access_token":"","refresh_token":"","expires_at":0,"scope":"kimi-code","token_type":"Bearer","expires_in":0}"#;
    let fixture = Fixture::new("signed-out", body);
    let error = read_local_credentials_at(&fixture.path()).unwrap_err();
    assert!(matches!(error, LocalCredentialError::SignedOut));
    assert_eq!(
        error.to_string(),
        "Kimi Code is signed out. Run /login in Kimi Code."
    );
    assert_eq!(std::fs::read_to_string(fixture.path()).unwrap(), body);

    let body = json!({
        "access_token": "   ", "expires_at": chrono::Utc::now().timestamp() + 3600
    })
    .to_string();
    let fixture = Fixture::new("signed-out-whitespace", &body);
    let error = read_local_credentials_at(&fixture.path()).unwrap_err();
    assert!(matches!(error, LocalCredentialError::SignedOut));
    assert_eq!(
        error.to_string(),
        "Kimi Code is signed out. Run /login in Kimi Code."
    );
    assert_eq!(std::fs::read_to_string(fixture.path()).unwrap(), body);
}

#[test]
fn expired_oauth_asks_the_cli_to_renew_without_changing_credentials() {
    let body = json!({"access_token": "expired-token", "expires_at": 1}).to_string();
    let fixture = Fixture::new("expired", &body);
    let error = read_local_credentials_at(&fixture.path()).unwrap_err();
    assert!(matches!(error, LocalCredentialError::Expired));
    assert!(error.to_string().contains("Run Kimi Code"));
    assert_eq!(std::fs::read_to_string(fixture.path()).unwrap(), body);
}

#[test]
fn resolves_only_the_new_kimi_code_home() {
    // This is the only test in this process that consults KIMI_CODE_HOME.
    let original = std::env::var_os("KIMI_CODE_HOME");
    std::env::remove_var("KIMI_CODE_HOME");
    assert_eq!(
        local_credentials_path().unwrap(),
        dirs::home_dir()
            .unwrap()
            .join(".kimi-code/credentials/kimi-code.json")
    );
    std::env::set_var("KIMI_CODE_HOME", "  '/tmp/quota-kimi-relocated'  ");
    assert_eq!(
        local_credentials_path().unwrap(),
        PathBuf::from("/tmp/quota-kimi-relocated/credentials/kimi-code.json")
    );
    std::env::set_var("KIMI_CODE_HOME", "   ");
    assert!(local_credentials_path()
        .unwrap()
        .ends_with(".kimi-code/credentials/kimi-code.json"));
    match original {
        Some(value) => std::env::set_var("KIMI_CODE_HOME", value),
        None => std::env::remove_var("KIMI_CODE_HOME"),
    }
}
