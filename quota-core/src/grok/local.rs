//! Read-only access to the credentials the Grok CLI stores locally.
//!
//! Quota never refreshes or rewrites this file. Rotating the refresh token here
//! would invalidate the copy the Grok CLI itself is using and break the user's
//! CLI, so an expired token is reported rather than renewed.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Where the Grok CLI writes its credentials. `GROK_HOME` relocates it.
pub fn local_credentials_path() -> Option<PathBuf> {
    crate::local_paths::provider_home("GROK_HOME", ".grok").map(|home| home.join("auth.json"))
}

#[derive(Clone)]
pub struct LocalGrokCredentials {
    pub access_token: String,
    pub email: Option<String>,
    /// Seconds since the epoch, when the file records an expiry it could parse.
    pub expires_at: Option<i64>,
}

impl LocalGrokCredentials {
    /// True when the stored expiry is at or before `now`. Unknown expiry is
    /// treated as usable, because the endpoint is the real authority.
    pub fn is_expired_at(&self, now: i64) -> bool {
        match self.expires_at {
            Some(expiry) => expiry <= now,
            None => false,
        }
    }
}

/// Deliberately hand-written so a token can never reach the Debug output.
impl fmt::Debug for LocalGrokCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalGrokCredentials")
            .field("access_token", &"<redacted>")
            .field("email", &self.email)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Debug)]
pub enum LocalCredentialError {
    NotFound,
    Unreadable(String),
    Malformed(String),
    /// The file parsed but held no entry with a usable token.
    NoUsableEntry,
}

impl fmt::Display for LocalCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(
                formatter,
                "No Grok credentials found. Sign in with the Grok CLI first."
            ),
            Self::Unreadable(detail) => {
                write!(formatter, "Could not read Grok credentials: {}", detail)
            }
            Self::Malformed(detail) => write!(
                formatter,
                "Grok credentials are not in the expected shape: {}",
                detail
            ),
            Self::NoUsableEntry => write!(
                formatter,
                "Grok credentials hold no usable token. Sign in with the Grok CLI again."
            ),
        }
    }
}

/// One entry in the auth file. Grok keys these by OIDC issuer, so a user who
/// has signed in through more than one issuer has more than one entry.
#[derive(Deserialize)]
struct AuthEntry {
    key: Option<String>,
    expires_at: Option<String>,
    email: Option<String>,
}

pub fn read_local_credentials_at(path: &Path) -> Result<LocalGrokCredentials, LocalCredentialError> {
    read_local_credentials_at_time(path, chrono::Utc::now().timestamp())
}

/// Split out so the entry-selection rule can be tested without waiting for a
/// real token to expire.
pub fn read_local_credentials_at_time(
    path: &Path,
    now: i64,
) -> Result<LocalGrokCredentials, LocalCredentialError> {
    if !path.exists() {
        return Err(LocalCredentialError::NotFound);
    }

    let raw = std::fs::read_to_string(path)
        .map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;

    // Only position information from the serde error is kept. Its message can
    // quote the input it failed on, which here could be a token.
    let entries: BTreeMap<String, AuthEntry> = serde_json::from_str(&raw).map_err(|err| {
        LocalCredentialError::Malformed(format!(
            "JSON error at line {} column {}",
            err.line(),
            err.column()
        ))
    })?;

    let usable: Vec<LocalGrokCredentials> = entries
        .into_values()
        .filter_map(|entry| {
            let access_token = normalize(entry.key)?;
            Some(LocalGrokCredentials {
                access_token,
                email: normalize(entry.email),
                expires_at: entry.expires_at.as_deref().and_then(parse_timestamp),
            })
        })
        .collect();

    if usable.is_empty() {
        return Err(LocalCredentialError::NoUsableEntry);
    }

    // A user signed in through two issuers has two entries and only one of them
    // is current. Prefer whichever is still valid, and fall back to the one
    // that expired most recently so the caller reports a real expiry rather
    // than an arbitrary one.
    let chosen = usable
        .iter()
        .filter(|entry| !entry.is_expired_at(now))
        .max_by_key(|entry| entry.expires_at.unwrap_or(i64::MAX))
        .or_else(|| usable.iter().max_by_key(|entry| entry.expires_at.unwrap_or(0)))
        .expect("usable is not empty");

    Ok(chosen.clone())
}

/// Convenience wrapper over the default path.
pub fn read_local_credentials() -> Result<LocalGrokCredentials, LocalCredentialError> {
    let path = local_credentials_path()
        .ok_or_else(|| LocalCredentialError::Unreadable("no home directory".to_string()))?;
    read_local_credentials_at(&path)
}

/// Grok writes expiry as RFC 3339, or as epoch seconds or milliseconds.
pub(crate) fn parse_timestamp(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Some(parsed.timestamp());
    }
    trimmed.parse::<i64>().ok().map(|raw| {
        if raw > 10_000_000_000 {
            raw / 1000
        } else {
            raw
        }
    })
}

fn normalize(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}
