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
            .field("refresh_token", &self.refresh_token.as_ref().map(|_| "<redacted>"))
            .field("expiry_date_ms", &self.expiry_date_ms)
            .field("email", &self.email)
            .field("issued_to_client_id", &self.issued_to_client_id)
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
                write!(formatter, "Could not read Antigravity credentials: {}", detail)
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
    })
}

/// Convenience wrapper over the default directory.
pub fn read_local_credentials() -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    let home =
        gemini_home().ok_or_else(|| LocalCredentialError::Unreadable("no home directory".to_string()))?;
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

/// Parse the keyring payload. Split from the keyring call so the shape can be
/// tested without a keyring, a session bus, or an unlock prompt.
pub fn parse_keyring_secret(
    raw: &str,
) -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    #[derive(Deserialize)]
    struct KeyringSecret {
        id_token: Option<String>,
        token: Option<KeyringToken>,
    }

    #[derive(Deserialize)]
    struct KeyringToken {
        access_token: Option<String>,
        refresh_token: Option<String>,
        /// An RFC 3339 timestamp, unlike the file source's epoch milliseconds.
        expiry: Option<String>,
    }

    // Only position information from the serde error is kept, since its message
    // can quote the input it failed on.
    let secret: KeyringSecret = serde_json::from_str(raw).map_err(|err| {
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
        expiry_date_ms: token
            .expiry
            .as_deref()
            .and_then(parse_rfc3339_ms),
        email: secret
            .id_token
            .as_deref()
            .and_then(|token| id_token_claim(token, "email")),
        issued_to_client_id: secret
            .id_token
            .as_deref()
            .and_then(|token| id_token_claim(token, "aud")),
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
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;

    match entry.get_password() {
        Ok(secret) => parse_keyring_secret(&secret),
        Err(keyring::Error::NoEntry) => Err(LocalCredentialError::NotFound),
        Err(err) => Err(LocalCredentialError::Unreadable(err.to_string())),
    }
}

/// Credentials for whichever Antigravity sign-in we can find, preferring the
/// CLI's keyring entry over the Gemini file. When neither is present the
/// keyring's error is the one reported, since it is the source that matters for
/// the sidebar.
pub fn read_any_credentials() -> Result<LocalAntigravityCredentials, LocalCredentialError> {
    let keyring_result = read_keyring_credentials();
    if keyring_result.is_ok() {
        return keyring_result;
    }
    match read_local_credentials() {
        Ok(credentials) => Ok(credentials),
        Err(_) => keyring_result,
    }
}
