use quota_core::grok::local::{read_local_credentials_at_time, LocalCredentialError};
use std::fs;

fn temp_file(name: &str, body: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "quota-grok-local-{}-{}",
        std::process::id(),
        name
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("auth.json");
    fs::write(&path, body).expect("write auth file");
    path
}

const NOW: i64 = 1_789_000_000;
const LATER: i64 = 1_799_000_000;
const EARLIER: i64 = 1_779_000_000;

#[test]
fn reads_a_single_entry_auth_file() {
    let path = temp_file(
        "single",
        &format!(
            r#"{{"https://auth.x.ai":{{"key":"grok-access-token","email":"person@example.com","expires_at":"{}"}}}}"#,
            LATER
        ),
    );

    let credentials = read_local_credentials_at_time(&path, NOW).expect("read credentials");

    assert_eq!(credentials.access_token, "grok-access-token");
    assert_eq!(credentials.email.as_deref(), Some("person@example.com"));
    assert_eq!(credentials.expires_at, Some(LATER));
    assert!(!credentials.is_expired_at(NOW));
}

#[test]
fn prefers_the_entry_that_has_not_expired() {
    // Signing in through a second issuer leaves the first entry in place. The
    // stale one must not win just because its issuer sorts first.
    let path = temp_file(
        "two-issuers",
        &format!(
            r#"{{
                "https://auth.a.example":{{"key":"stale-token","expires_at":"{}"}},
                "https://auth.z.example":{{"key":"current-token","expires_at":"{}"}}
            }}"#,
            EARLIER, LATER
        ),
    );

    let credentials = read_local_credentials_at_time(&path, NOW).expect("read credentials");

    assert_eq!(credentials.access_token, "current-token");
}

#[test]
fn falls_back_to_the_most_recently_expired_entry() {
    let path = temp_file(
        "all-expired",
        &format!(
            r#"{{
                "https://auth.a.example":{{"key":"very-old","expires_at":"{}"}},
                "https://auth.z.example":{{"key":"less-old","expires_at":"{}"}}
            }}"#,
            EARLIER - 1000,
            EARLIER
        ),
    );

    let credentials = read_local_credentials_at_time(&path, NOW).expect("read credentials");

    assert_eq!(credentials.access_token, "less-old");
    assert!(credentials.is_expired_at(NOW));
}

#[test]
fn accepts_an_rfc3339_expiry() {
    let path = temp_file(
        "rfc3339",
        r#"{"https://auth.x.ai":{"key":"token","expires_at":"2027-01-01T00:00:00Z"}}"#,
    );

    let credentials = read_local_credentials_at_time(&path, NOW).expect("read credentials");

    assert_eq!(credentials.expires_at, Some(1_798_761_600));
}

#[test]
fn accepts_an_expiry_in_milliseconds() {
    let path = temp_file(
        "millis",
        &format!(
            r#"{{"https://auth.x.ai":{{"key":"token","expires_at":"{}"}}}}"#,
            LATER * 1000
        ),
    );

    let credentials = read_local_credentials_at_time(&path, NOW).expect("read credentials");

    assert_eq!(credentials.expires_at, Some(LATER));
}

#[test]
fn an_entry_with_no_expiry_is_treated_as_usable() {
    let path = temp_file("no-expiry", r#"{"https://auth.x.ai":{"key":"token"}}"#);

    let credentials = read_local_credentials_at_time(&path, NOW).expect("read credentials");

    assert_eq!(credentials.expires_at, None);
    assert!(!credentials.is_expired_at(NOW));
}

#[test]
fn missing_file_is_a_distinct_error() {
    let path = std::env::temp_dir().join(format!(
        "quota-grok-local-{}-absent/auth.json",
        std::process::id()
    ));

    match read_local_credentials_at_time(&path, NOW) {
        Err(LocalCredentialError::NotFound) => {}
        other => panic!("expected NotFound, got {:?}", other.map(|_| "credentials")),
    }
}

#[test]
fn an_empty_map_reports_no_usable_entry() {
    let path = temp_file("empty", "{}");

    match read_local_credentials_at_time(&path, NOW) {
        Err(LocalCredentialError::NoUsableEntry) => {}
        other => panic!(
            "expected NoUsableEntry, got {:?}",
            other.map(|_| "credentials")
        ),
    }
}

#[test]
fn an_entry_with_a_blank_token_is_skipped() {
    let path = temp_file(
        "blank-token",
        r#"{"https://auth.a.example":{"key":"   "},"https://auth.z.example":{"key":"real-token"}}"#,
    );

    let credentials = read_local_credentials_at_time(&path, NOW).expect("read credentials");

    assert_eq!(credentials.access_token, "real-token");
}

#[test]
fn credentials_struct_debug_redacts_access_token() {
    let path = temp_file(
        "redaction",
        r#"{"https://auth.x.ai":{"key":"super-secret-grok-token"}}"#,
    );

    let credentials = read_local_credentials_at_time(&path, NOW).expect("read credentials");
    let rendered = format!("{:?}", credentials);

    assert!(
        !rendered.contains("super-secret-grok-token"),
        "Debug output leaked the access token: {}",
        rendered
    );
    assert!(rendered.contains("<redacted>"), "got: {}", rendered);
}

#[test]
fn a_parse_failure_never_quotes_the_token() {
    let path = temp_file(
        "leaky-parse",
        r#"{"https://auth.x.ai":{"key":"super-secret-grok-token"#,
    );

    match read_local_credentials_at_time(&path, NOW) {
        Err(err) => {
            let rendered = format!("{} {:?}", err, err);
            assert!(
                !rendered.contains("super-secret-grok-token"),
                "parse error leaked the token: {}",
                rendered
            );
        }
        Ok(_) => panic!("expected the truncated file to fail parsing"),
    }
}
