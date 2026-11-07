//! Read-only access to the Google credentials Antigravity stores locally.
//!
//! Quota never rewrites this file. Unlike the other providers, an expired
//! token here is not the end of the road: Google hands back a new access token
//! without rotating the refresh token, so the caller can refresh in memory and
//! leave the file exactly as it found it. See `local_usage::fetch_local_usage`.

use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};

/// Antigravity shares the Gemini CLI's credential directory.
pub fn gemini_home() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".gemini"))
}

/// Where Antigravity writes its OAuth credentials.
pub fn local_credentials_path() -> Option<PathBuf> {
    gemini_home().map(|home| home.join("oauth_creds.json"))
}

/// Where the Antigravity CLI writes its OAuth credentials on disk.
pub fn cli_credentials_path_in(gemini_home: &Path) -> PathBuf {
    gemini_home
        .join("antigravity-cli")
        .join("antigravity-oauth-token")
}

pub fn cli_credentials_path() -> Option<PathBuf> {
    gemini_home().map(|home| cli_credentials_path_in(&home))
}

pub fn read_cli_credentials_in(
    gemini_home: &Path,
) -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    let path = cli_credentials_path_in(gemini_home);
    if !path.exists() {
        return Err(LocalCredentialError::NotFound);
    }

    let raw = std::fs::read_to_string(&path)
        .map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;

    parse_keyring_secret(&raw)
}

pub fn read_cli_credentials() -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    let home = gemini_home()
        .ok_or_else(|| LocalCredentialError::Unreadable("no home directory".to_string()))?;
    read_cli_credentials_in(&home)
}

#[derive(Clone)]
pub struct LocalAntigravityCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Milliseconds since the epoch, matching what Google writes.
    pub expiry_date_ms: Option<i64>,
    pub email: Option<String>,
    /// The OAuth client these credentials were issued to, read from the id
    /// token's `aud` claim. Google binds a refresh token to its issuing client,
    /// so refreshing with any other client's id fails with
    /// `unauthorized_client`. Antigravity and the Gemini CLI use different
    /// clients and both write this same file.
    pub issued_to_client_id: Option<String>,
    /// True when the token came from Antigravity's own CLI (its keyring entry
    /// or the token file it falls back to) rather than the shared Gemini file.
    /// The current keyring payload names the account but carries no id token,
    /// so it cannot name its OAuth client; a CLI-owned token can safely be
    /// attributed to Antigravity's single registered client.
    pub cli_owned: bool,
}

impl LocalAntigravityCredentials {
    /// True when the stored expiry is within a minute of `now_ms`, matching the
    /// desktop app's margin. A token about to expire mid-request is no better
    /// than one already expired.
    pub fn needs_refresh_at_ms(&self, now_ms: i64) -> bool {
        match self.expiry_date_ms {
            Some(expiry) => expiry <= now_ms + 60_000,
            None => false,
        }
    }
}

/// Deliberately hand-written so no token can reach the Debug output.
impl fmt::Debug for LocalAntigravityCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalAntigravityCredentials")
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("expiry_date_ms", &self.expiry_date_ms)
            .field("email", &self.email)
            .field("issued_to_client_id", &self.issued_to_client_id)
            .field("cli_owned", &self.cli_owned)
            .finish()
    }
}

#[derive(Debug)]
pub enum LocalCredentialError {
    NotFound,
    Unreadable(String),
    Malformed(String),
}

impl fmt::Display for LocalCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(
                formatter,
                "No Antigravity credentials found. Sign in to Antigravity first."
            ),
            Self::Unreadable(detail) => {
                write!(
                    formatter,
                    "Could not read Antigravity credentials: {}",
                    detail
                )
            }
            Self::Malformed(detail) => write!(
                formatter,
                "Antigravity credentials are not in the expected shape: {}",
                detail
            ),
        }
    }
}

#[derive(Deserialize)]
struct OauthCreds {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expiry_date: Option<i64>,
}

pub fn read_local_credentials_in(
    gemini_home: &Path,
) -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    let path = gemini_home.join("oauth_creds.json");
    if !path.exists() {
        return Err(LocalCredentialError::NotFound);
    }

    let raw = std::fs::read_to_string(&path)
        .map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;

    // Only position information from the serde error is kept. Its message can
    // quote the input it failed on, which here could be a token.
    let creds: OauthCreds = serde_json::from_str(&raw).map_err(|err| {
        LocalCredentialError::Malformed(format!(
            "JSON error at line {} column {}",
            err.line(),
            err.column()
        ))
    })?;

    let access_token = creds
        .access_token
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .ok_or_else(|| LocalCredentialError::Malformed("no access token".to_string()))?;

    Ok(LocalAntigravityCredentials {
        access_token,
        refresh_token: creds
            .refresh_token
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty()),
        expiry_date_ms: creds.expiry_date,
        email: read_active_google_email(gemini_home),
        issued_to_client_id: creds
            .id_token
            .as_deref()
            .and_then(|token| id_token_claim(token, "aud")),
        cli_owned: false,
    })
}

/// Convenience wrapper over the default directory.
pub fn read_local_credentials() -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    let home = gemini_home()
        .ok_or_else(|| LocalCredentialError::Unreadable("no home directory".to_string()))?;
    read_local_credentials_in(&home)
}

/// Read one string claim out of an id token. `aud` names the OAuth client the
/// credentials belong to, and `email` the account. A token that will not decode
/// is not fatal; the caller reports what it could not determine.
fn id_token_claim(token: &str, claim: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    claims
        .get(claim)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

/// The signed-in address, used only as a display label. A missing or unreadable
/// accounts file is not an error, since usage does not depend on it.
pub fn read_active_google_email(gemini_home: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(gemini_home.join("google_accounts.json")).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    ["active", "activeEmail", "current"]
        .iter()
        .filter_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .find(|item| !item.is_empty())
        .map(str::to_string)
}

// ---------------------------------------------------------------------------
// Keyring source
// ---------------------------------------------------------------------------
//
// The Antigravity CLI keeps its token in the OS keyring, not in a file. That is
// why `~/.gemini/oauth_creds.json` can sit untouched for months while the CLI
// works: the file belongs to Gemini Code Assist, and the two can even be signed
// in as different accounts. The Herdr pane runs the CLI, so the keyring is the
// source that matches what the sidebar is reporting on.

/// Where the Antigravity CLI stores its token, as written by Go's `go-keyring`.
pub const KEYRING_SERVICE: &str = "gemini";
pub const KEYRING_USER: &str = "antigravity";

/// The Credential Manager target name go-keyring writes on Windows, which is
/// `service:user`. The Rust store would otherwise look up `user.service`
/// (`antigravity.gemini`) and never find the CLI's entry.
pub const WINDOWS_KEYRING_TARGET: &str = "gemini:antigravity";

/// Parse the keyring payload. Split from the keyring call so the shape can be
/// tested without a keyring, a session bus, or an unlock prompt.
pub fn parse_keyring_secret(
    raw: &str,
) -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    #[derive(Deserialize)]
    struct KeyringSecret {
        /// Older payloads carry the id token, which names the account and the
        /// OAuth client.
        id_token: Option<String>,
        /// The current payload carries the address on its own and no id token.
        id_token_email: Option<String>,
        token: Option<KeyringToken>,
    }

    #[derive(Deserialize)]
    struct KeyringToken {
        access_token: Option<String>,
        refresh_token: Option<String>,
        /// An RFC 3339 timestamp, unlike the file source's epoch milliseconds.
        expiry: Option<String>,
    }

    let decoded_storage: String;
    let json_str = if let Some(stripped) = raw.strip_prefix("go-keyring-base64:") {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(stripped.trim())
            .map_err(|_| {
                LocalCredentialError::Malformed("invalid base64 in keyring secret".to_string())
            })?;
        decoded_storage = String::from_utf8(bytes).map_err(|_| {
            LocalCredentialError::Malformed("keyring secret was not valid UTF-8".to_string())
        })?;
        &decoded_storage
    } else {
        raw
    };

    // Only position information from the serde error is kept, since its message
    // can quote the input it failed on.
    let secret: KeyringSecret = serde_json::from_str(json_str).map_err(|err| {
        LocalCredentialError::Malformed(format!(
            "JSON error at line {} column {}",
            err.line(),
            err.column()
        ))
    })?;

    let token = secret
        .token
        .ok_or_else(|| LocalCredentialError::Malformed("no token block".to_string()))?;

    let access_token = token
        .access_token
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| LocalCredentialError::Malformed("no access token".to_string()))?;

    Ok(LocalAntigravityCredentials {
        access_token,
        refresh_token: token
            .refresh_token
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        expiry_date_ms: token.expiry.as_deref().and_then(parse_rfc3339_ms),
        email: secret
            .id_token
            .as_deref()
            .and_then(|token| id_token_claim(token, "email"))
            .or_else(|| {
                secret
                    .id_token_email
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            }),
        issued_to_client_id: secret
            .id_token
            .as_deref()
            .and_then(|token| id_token_claim(token, "aud")),
        cli_owned: true,
    })
}

fn parse_rfc3339_ms(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|stamp| stamp.timestamp_millis())
}

/// Read the Antigravity CLI's credentials out of the OS keyring. Read-only:
/// nothing is ever stored or deleted.
pub fn read_keyring_credentials() -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    read_keyring_secret().and_then(|secret| parse_keyring_secret(&secret))
}

/// Read the raw keyring payload on macOS through `/usr/bin/security`.
///
/// The CLI writes the entry with that same tool (macOS go-keyring shells out to
/// `security`), so the item's access list already allows this read and no
/// unlock prompt appears. The `keyring` crate talks to the Security framework
/// as a different client and does prompt, so macOS does not use it here.
#[cfg(target_os = "macos")]
fn read_keyring_secret() -> Result<String, LocalCredentialError> {
    // `security` exits with 44 when no item matches the service.
    const ITEM_NOT_FOUND: i32 = 44;

    let output = std::process::Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-s",
            KEYRING_SERVICE,
            "-a",
            KEYRING_USER,
            "-w",
        ])
        .output()
        .map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;

    if !output.status.success() {
        return Err(match output.status.code() {
            Some(ITEM_NOT_FOUND) => LocalCredentialError::NotFound,
            _ => {
                LocalCredentialError::Unreadable(format!("security exited with {}", output.status))
            }
        });
    }

    let raw = String::from_utf8(output.stdout)
        .map_err(|_| LocalCredentialError::Malformed("Keychain item is not UTF-8".to_string()))?;
    Ok(raw.trim_end_matches(['\r', '\n']).to_string())
}

#[cfg(not(target_os = "macos"))]
fn read_keyring_secret() -> Result<String, LocalCredentialError> {
    let entry = keyring_entry().map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;

    match read_keyring_text(&entry) {
        Ok(secret) => Ok(secret),
        Err(keyring::Error::NoEntry) => Err(LocalCredentialError::NotFound),
        Err(err) => Err(LocalCredentialError::Unreadable(err.to_string())),
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn read_keyring_text(entry: &keyring::Entry) -> keyring::Result<String> {
    entry.get_password()
}

/// The Windows store decodes passwords as UTF-16, but go-keyring writes the
/// Go string's raw UTF-8 bytes, so the blob is read as bytes and decoded here.
#[cfg(windows)]
fn read_keyring_text(entry: &keyring_core::Entry) -> keyring::Result<String> {
    decode_keyring_blob(entry.get_secret()?)
}

/// Decode a keyring blob written by go-keyring. On failure the error carries
/// no secret bytes, only the fact that the blob was not UTF-8.
pub fn decode_keyring_blob(blob: Vec<u8>) -> keyring::Result<String> {
    String::from_utf8(blob).map_err(|_| keyring::Error::BadEncoding(Vec::new()))
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn keyring_entry() -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
}

/// The `keyring` wrapper has no way to pass a target name, so the entry is
/// built through `keyring-core` once the wrapper has set up the default store.
#[cfg(windows)]
fn keyring_entry() -> keyring::Result<keyring_core::Entry> {
    if keyring::Entry::store_status().is_err() {
        return Err(keyring::Error::NoDefaultStore);
    }
    let modifiers = std::collections::HashMap::from([("target", WINDOWS_KEYRING_TARGET)]);
    keyring_core::Entry::new_with_modifiers(KEYRING_SERVICE, KEYRING_USER, &modifiers)
}

/// Credentials for whichever Antigravity sign-in we can find.
pub fn read_any_credentials() -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    choose_credentials(
        read_cli_credentials(),
        read_keyring_credentials,
        read_local_credentials,
        chrono::Utc::now().timestamp_millis(),
    )
}

/// Picks between the three Antigravity sources. The CLI token file wins while
/// its token is still good, so macOS never touches the keychain (and its
/// authorization prompt) when the file is fresh. Otherwise the keyring comes
/// next, since newer `agy` builds on Linux keep the live token there and leave
/// the file behind. An expired file is still better than nothing, because the
/// refresh token in it usually works. The Gemini file is the last resort.
///
/// When nothing is usable, a broken CLI file is reported over the keyring's
/// error so it does not hide behind a misleading "not found".
pub fn choose_credentials(
    cli: Result<LocalAntigravityCredentials, LocalCredentialError>,
    keyring: impl FnOnce() -> Result<LocalAntigravityCredentials, LocalCredentialError>,
    gemini_file: impl FnOnce() -> Result<LocalAntigravityCredentials, LocalCredentialError>,
    now_ms: i64,
) -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    if let Ok(credentials) = &cli {
        if !credentials.needs_refresh_at_ms(now_ms) {
            return cli;
        }
    }
    let keyring_result = keyring();
    if keyring_result.is_ok() {
        return keyring_result;
    }
    if cli.is_ok() {
        return cli;
    }
    if let Ok(credentials) = gemini_file() {
        return Ok(credentials);
    }
    match cli {
        Err(LocalCredentialError::NotFound) => keyring_result,
        broken => broken,
    }
}
