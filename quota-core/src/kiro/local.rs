//! Read-only access to the credentials the Kiro CLI and IDE store locally.
//!
//! Kiro signs in through AWS SSO and leaves its token in the shared SSO cache
//! at `~/.aws/sso/cache/kiro-auth-token.json`, not in a directory of its own.
//! The usage call also needs a profile ARN, which the IDE writes to a
//! `profile.json` under its own settings directory and which the token itself
//! usually carries too.
//!
//! Quota never refreshes or rewrites either file. The refresh token here backs
//! the user's own Kiro session, so a rejected token is reported rather than
//! renewed.

use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};

/// The AWS SSO cache entry Kiro writes its token into.
pub fn local_auth_token_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        home.join(".aws")
            .join("sso")
            .join("cache")
            .join("kiro-auth-token.json")
    })
}

/// Where the Kiro IDE stores the signed-in profile. Each platform puts its
/// application data somewhere different, so each gets its own branch.
pub fn local_profile_path() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        return dirs::home_dir()
            .map(|home| home.join(".config/Kiro/User/globalStorage/kiro.kiroagent/profile.json"));
    }

    #[cfg(target_os = "macos")]
    {
        return dirs::home_dir().map(|home| {
            home.join(
                "Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/profile.json",
            )
        });
    }

    #[cfg(target_os = "windows")]
    {
        return std::env::var("APPDATA").ok().map(|appdata| {
            PathBuf::from(appdata).join("Kiro/User/globalStorage/kiro.kiroagent/profile.json")
        });
    }

    #[allow(unreachable_code)]
    None
}

#[derive(Clone)]
pub struct LocalKiroCredentials {
    pub access_token: String,
    /// Identifies the subscription the usage call reports on, and carries the
    /// AWS region in its third segment.
    pub profile_arn: String,
    /// When the cached token stops being valid, if the file says.
    pub expires_at: Option<i64>,
}

impl LocalKiroCredentials {
    /// Whether the cached token has already lapsed.
    ///
    /// Worth checking before spending a request: Kiro answers a lapsed token
    /// the same way it answers a disabled account, so without this the user is
    /// told their account may be disabled when they only need to sign in again.
    pub fn is_expired_at(&self, now: i64) -> bool {
        self.expires_at.is_some_and(|expiry| expiry <= now)
    }
}

/// Deliberately hand-written so a token can never reach the Debug output.
impl fmt::Debug for LocalKiroCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalKiroCredentials")
            .field("access_token", &"<redacted>")
            .field("profile_arn", &self.profile_arn)
            .finish()
    }
}

#[derive(Debug)]
pub enum LocalCredentialError {
    NotFound,
    Unreadable(String),
    Malformed(String),
    /// Signed in, but nothing on disk says which subscription to ask about.
    NoProfileArn,
}

impl fmt::Display for LocalCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(
                formatter,
                "No Kiro credentials found. Sign in to Kiro first."
            ),
            Self::Unreadable(detail) => {
                write!(formatter, "Could not read Kiro credentials: {}", detail)
            }
            Self::Malformed(detail) => write!(
                formatter,
                "Kiro credentials are not in the expected shape: {}",
                detail
            ),
            Self::NoProfileArn => write!(
                formatter,
                "Kiro is signed in but no profile ARN was found, so there is no subscription to report on."
            ),
        }
    }
}

/// Read credentials from an explicit pair of paths.
///
/// The profile is optional: it is the better source for the ARN, but a token
/// that carries its own is enough, and on a CLI-only machine the IDE's profile
/// file never exists.
pub fn read_local_credentials_at(
    auth_token_path: &Path,
    profile_path: Option<&Path>,
) -> Result<LocalKiroCredentials, LocalCredentialError> {
    if !auth_token_path.exists() {
        return Err(LocalCredentialError::NotFound);
    }

    let auth_token = read_json(auth_token_path)?;
    let profile = profile_path
        .filter(|path| path.exists())
        .map(read_json)
        .transpose()?;

    let access_token = pick_string(Some(&auth_token), &[&["accessToken"], &["access_token"]])
        .ok_or_else(|| LocalCredentialError::Malformed("no access token".to_string()))?;

    // The profile is preferred because the IDE rewrites it when the user
    // switches subscription, while the cached token can lag behind.
    let profile_arn = pick_string(profile.as_ref(), &[&["arn"], &["profileArn"]])
        .or_else(|| {
            pick_string(
                Some(&auth_token),
                &[&["profileArn"], &["profile_arn"], &["arn"]],
            )
        })
        .ok_or(LocalCredentialError::NoProfileArn)?;

    let expires_at = pick_timestamp(
        Some(&auth_token),
        &[&["expiresAt"], &["expires_at"]],
    );

    Ok(LocalKiroCredentials {
        access_token,
        profile_arn,
        expires_at,
    })
}

/// Convenience wrapper over the default paths.
pub fn read_local_credentials() -> Result<LocalKiroCredentials, LocalCredentialError> {
    let auth_token_path = local_auth_token_path()
        .ok_or_else(|| LocalCredentialError::Unreadable("no home directory".to_string()))?;
    let profile_path = local_profile_path();
    read_local_credentials_at(&auth_token_path, profile_path.as_deref())
}

fn read_json(path: &Path) -> Result<Value, LocalCredentialError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;

    // Only position information from the serde error is kept. Its message can
    // quote the input it failed on, which here would be a token.
    serde_json::from_str(&raw).map_err(|err| {
        LocalCredentialError::Malformed(format!(
            "JSON error at line {} column {}",
            err.line(),
            err.column()
        ))
    })
}

/// Read an expiry that may arrive as epoch seconds, epoch milliseconds, or an
/// RFC 3339 string. The desktop app writes a number, but AWS SSO cache entries
/// conventionally carry an RFC 3339 string, so both are accepted.
fn pick_timestamp(root: Option<&Value>, paths: &[&[&str]]) -> Option<i64> {
    let root = root?;
    for path in paths {
        let mut current = root;
        let mut found = true;
        for part in *path {
            match current.get(*part) {
                Some(next) => current = next,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if !found {
            continue;
        }
        if let Some(number) = current.as_i64() {
            return Some(normalize_epoch(number));
        }
        if let Some(text) = current.as_str() {
            let trimmed = text.trim();
            if let Ok(number) = trimmed.parse::<i64>() {
                return Some(normalize_epoch(number));
            }
            if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(trimmed) {
                return Some(parsed.timestamp());
            }
        }
    }
    None
}

/// Values past roughly the year 33658 are milliseconds, not seconds.
fn normalize_epoch(value: i64) -> i64 {
    if value > 1_000_000_000_000 {
        value / 1000
    } else {
        value
    }
}

fn pick_string(root: Option<&Value>, paths: &[&[&str]]) -> Option<String> {
    let root = root?;
    for path in paths {
        let mut current = root;
        let mut found = true;
        for part in *path {
            match current.get(*part) {
                Some(next) => current = next,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if found {
            if let Some(text) = current.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}
