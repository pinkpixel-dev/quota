use quota_core::kiro::local::{
    local_profile_path, read_cli_credentials_at, read_local_credentials_at, LocalCredentialError,
};
use std::fs;
use std::path::{Path, PathBuf};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("quota-kiro-local-{}-{}", std::process::id(), name));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write(dir: &Path, file: &str, body: &str) -> PathBuf {
    let path = dir.join(file);
    fs::write(&path, body).expect("write file");
    path
}

#[test]
fn reads_the_token_and_arn_from_the_sso_cache_alone() {
    // A machine with only the Kiro CLI has no IDE profile file at all, so the
    // token has to be enough on its own.
    let dir = temp_dir("token-only");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"kiro-access-token","profileArn":"arn:aws:codewhisperer:us-east-1:1:profile/ABC"}"#,
    );

    let credentials = read_local_credentials_at(&token, None).expect("read credentials");

    assert_eq!(credentials.access_token, "kiro-access-token");
    assert_eq!(
        credentials.profile_arn,
        "arn:aws:codewhisperer:us-east-1:1:profile/ABC"
    );
}

#[test]
fn the_profile_arn_wins_over_the_cached_token_arn() {
    // The IDE rewrites the profile when the subscription changes, while the
    // cached token can still name the previous one.
    let dir = temp_dir("profile-wins");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"kiro-access-token","profileArn":"arn:aws:codewhisperer:us-east-1:1:profile/OLD"}"#,
    );
    let profile = write(
        &dir,
        "profile.json",
        r#"{"arn":"arn:aws:codewhisperer:eu-central-1:1:profile/NEW"}"#,
    );

    let credentials =
        read_local_credentials_at(&token, Some(&profile)).expect("read credentials");

    assert_eq!(
        credentials.profile_arn,
        "arn:aws:codewhisperer:eu-central-1:1:profile/NEW"
    );
}

#[test]
fn a_profile_path_that_does_not_exist_is_not_an_error() {
    let dir = temp_dir("absent-profile");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"kiro-access-token","arn":"arn:aws:codewhisperer:us-east-1:1:profile/ABC"}"#,
    );

    let credentials = read_local_credentials_at(&token, Some(&dir.join("nope.json")))
        .expect("read credentials");

    assert_eq!(credentials.access_token, "kiro-access-token");
}

#[test]
fn missing_token_file_is_a_distinct_error() {
    let dir = temp_dir("absent-token");

    match read_local_credentials_at(&dir.join("nope.json"), None) {
        Err(LocalCredentialError::NotFound) => {}
        other => panic!("expected NotFound, got {:?}", other),
    }
}

#[test]
fn a_token_without_an_arn_says_so_rather_than_failing_vaguely() {
    let dir = temp_dir("no-arn");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"kiro-access-token"}"#,
    );

    match read_local_credentials_at(&token, None) {
        Err(LocalCredentialError::NoProfileArn) => {}
        other => panic!("expected NoProfileArn, got {:?}", other),
    }
}

#[test]
fn credentials_struct_debug_redacts_the_access_token() {
    let dir = temp_dir("debug");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"super-secret-token","arn":"arn:aws:codewhisperer:us-east-1:1:profile/A"}"#,
    );
    let credentials = read_local_credentials_at(&token, None).expect("read credentials");

    let rendered = format!("{:?}", credentials);

    assert!(!rendered.contains("super-secret-token"));
    assert!(rendered.contains("<redacted>"));
}

#[test]
fn a_parse_failure_never_quotes_the_token() {
    let dir = temp_dir("malformed");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"super-secret-token","#,
    );

    match read_local_credentials_at(&token, None) {
        Err(error) => {
            let rendered = format!("{} {:?}", error, error);
            assert!(!rendered.contains("super-secret-token"));
        }
        Ok(_) => panic!("expected a parse failure"),
    }
}

#[test]
fn reading_credentials_never_writes_to_the_files() {
    let dir = temp_dir("readonly");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"kiro-access-token","arn":"arn:aws:codewhisperer:us-east-1:1:profile/A"}"#,
    );
    let before = fs::read(&token).expect("read before");
    let modified_before = fs::metadata(&token)
        .and_then(|meta| meta.modified())
        .expect("mtime before");

    read_local_credentials_at(&token, None).expect("read credentials");

    assert_eq!(before, fs::read(&token).expect("read after"));
    assert_eq!(
        modified_before,
        fs::metadata(&token)
            .and_then(|meta| meta.modified())
            .expect("mtime after")
    );
}

#[test]
fn the_profile_path_is_resolved_for_this_platform() {
    // Each platform puts application data somewhere different. A wrong path
    // here degrades quietly: the ARN falls back to the cached token, which may
    // name a subscription the user has since switched away from.
    let path = local_profile_path().expect("profile path");
    let rendered = path.to_string_lossy().replace('\\', "/");

    assert!(
        rendered.ends_with("Kiro/User/globalStorage/kiro.kiroagent/profile.json"),
        "unexpected profile path: {}",
        rendered
    );
}

#[test]
fn reads_an_expiry_written_as_epoch_seconds() {
    let dir = temp_dir("expiry-epoch");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"t","arn":"arn:aws:codewhisperer:us-east-1:1:profile/A","expiresAt":1790000000}"#,
    );

    let credentials = read_local_credentials_at(&token, None).expect("read credentials");

    assert_eq!(credentials.expires_at, Some(1790000000));
    assert!(credentials.is_expired_at(1790000001));
    assert!(!credentials.is_expired_at(1789999999));
}

#[test]
fn reads_an_expiry_written_as_an_rfc3339_string() {
    // AWS SSO cache entries conventionally use this form.
    let dir = temp_dir("expiry-rfc");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"t","arn":"arn:aws:codewhisperer:us-east-1:1:profile/A","expiresAt":"2026-10-01T00:00:00Z"}"#,
    );

    let credentials = read_local_credentials_at(&token, None).expect("read credentials");

    assert_eq!(credentials.expires_at, Some(1790812800));
}

#[test]
fn an_expiry_in_milliseconds_is_not_read_as_the_year_58000() {
    let dir = temp_dir("expiry-ms");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"t","arn":"arn:aws:codewhisperer:us-east-1:1:profile/A","expiresAt":1790000000000}"#,
    );

    let credentials = read_local_credentials_at(&token, None).expect("read credentials");

    assert_eq!(credentials.expires_at, Some(1790000000));
}

#[test]
fn a_token_with_no_expiry_is_never_treated_as_expired() {
    let dir = temp_dir("expiry-absent");
    let token = write(
        &dir,
        "kiro-auth-token.json",
        r#"{"accessToken":"t","arn":"arn:aws:codewhisperer:us-east-1:1:profile/A"}"#,
    );

    let credentials = read_local_credentials_at(&token, None).expect("read credentials");

    assert_eq!(credentials.expires_at, None);
    assert!(!credentials.is_expired_at(i64::MAX));
}

/// Build a database shaped like the one the Kiro CLI keeps its token in.
fn cli_database(dir: &Path, key: &str, value: &str) -> PathBuf {
    let path = dir.join("data.sqlite3");
    let connection = rusqlite::Connection::open(&path).expect("create db");
    connection
        .execute("CREATE TABLE auth_kv (key TEXT PRIMARY KEY, value TEXT)", [])
        .expect("create table");
    connection
        .execute(
            "INSERT INTO auth_kv (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )
        .expect("insert row");
    path
}

#[test]
fn reads_the_token_the_cli_keeps_in_its_database() {
    // The CLI refreshes here and never touches the SSO cache file, so a machine
    // can have a working CLI and a months-old SSO copy at the same time.
    let dir = temp_dir("cli-db");
    let path = cli_database(
        &dir,
        "kirocli:social:token",
        r#"{"access_token":"kiro-cli-token","expires_at":"2026-10-01T00:00:00Z","refresh_token":"r","provider":"github","profile_arn":"arn:aws:codewhisperer:us-east-1:1:profile/CLI"}"#,
    );

    let credentials = read_cli_credentials_at(&path).expect("read credentials");

    assert_eq!(credentials.access_token, "kiro-cli-token");
    assert_eq!(
        credentials.profile_arn,
        "arn:aws:codewhisperer:us-east-1:1:profile/CLI"
    );
    assert_eq!(credentials.expires_at, Some(1790812800));
}

#[test]
fn matches_the_token_row_whatever_the_login_method_is_called() {
    // The middle segment names the sign-in method, so an enterprise login uses
    // a different key than a social one.
    let dir = temp_dir("cli-db-idc");
    let path = cli_database(
        &dir,
        "kirocli:idc:token",
        r#"{"access_token":"t","profile_arn":"arn:aws:codewhisperer:us-east-1:1:profile/A"}"#,
    );

    assert!(read_cli_credentials_at(&path).is_ok());
}

#[test]
fn a_database_with_no_token_row_is_not_found_rather_than_an_error() {
    let dir = temp_dir("cli-db-empty");
    let path = dir.join("data.sqlite3");
    let connection = rusqlite::Connection::open(&path).expect("create db");
    connection
        .execute("CREATE TABLE auth_kv (key TEXT PRIMARY KEY, value TEXT)", [])
        .expect("create table");

    match read_cli_credentials_at(&path) {
        Err(LocalCredentialError::NotFound) => {}
        other => panic!("expected NotFound, got {:?}", other),
    }
}

#[test]
fn an_absent_database_is_not_found() {
    let dir = temp_dir("cli-db-absent");

    match read_cli_credentials_at(&dir.join("nope.sqlite3")) {
        Err(LocalCredentialError::NotFound) => {}
        other => panic!("expected NotFound, got {:?}", other),
    }
}

#[test]
fn the_cli_database_is_never_opened_for_writing() {
    // The database belongs to the user's own CLI, which writes to it on every
    // refresh. Taking a write lock here could block their session.
    let dir = temp_dir("cli-db-readonly");
    let path = cli_database(
        &dir,
        "kirocli:social:token",
        r#"{"access_token":"t","profile_arn":"arn:aws:codewhisperer:us-east-1:1:profile/A"}"#,
    );
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).expect("set readonly");

    let result = read_cli_credentials_at(&path);

    assert!(
        result.is_ok(),
        "a read-only file must still be readable: {:?}",
        result.err()
    );
}
