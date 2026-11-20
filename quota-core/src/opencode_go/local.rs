//! Read-only access to the credentials the OpenCode console stores locally.
//!
//! Quota never refreshes or rewrites this file. Rotating the refresh token
//! here would invalidate the copy the console itself is using and break the
//! user's sign-in, so an unusable token is reported rather than renewed.

use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// Where the OpenCode console writes its credential after
/// `npx @opencode/console login`. `XDG_DATA_HOME` relocates it, the way the
/// console's own file layout does.
pub fn local_credentials_path() -> Option<PathBuf> {
    crate::local_paths::data_home()
        .map(|data| data.join("opencode-console").join("credential.json"))
}

#[derive(Clone)]
pub struct LocalOpenCodeCredentials {
    /// The bearer token the usage endpoint accepts. The same key material the
    /// console itself sends, so no separate Go API key is needed.
    pub access_token: String,
}

impl LocalOpenCodeCredentials {
    /// Read the credential file at `path`. Never writes to disk.
    pub fn read_at(path: &Path) -> Result<Self, LocalCredentialError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;
        // Serde's own message can quote the input it failed on, and this file
        // holds a token. Report only the position, never the content.
        let parsed: StoredCredential = serde_json::from_str(&raw).map_err(|err| {
            LocalCredentialError::Malformed(format!(
                "JSON error at line {} column {}",
                err.line(),
                err.column()
            ))
        })?;
        let access_token = parsed.access.trim().to_string();
        if access_token.is_empty() {
            return Err(LocalCredentialError::NoUsableToken);
        }
        Ok(Self { access_token })
    }

    /// Read the credential from its default location.
    pub fn read() -> Result<Self, LocalCredentialError> {
        Self::read_at(&local_credentials_path().ok_or(LocalCredentialError::NotFound)?)
    }
}

#[derive(Deserialize)]
struct StoredCredential {
    #[serde(default)]
    access: String,
}

/// Deliberately hand-written so a token can never reach the Debug output.
impl fmt::Debug for LocalOpenCodeCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalOpenCodeCredentials")
            .field("access_token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum LocalCredentialError {
    NotFound,
    Unreadable(String),
    Malformed(String),
    NoUsableToken,
}

impl fmt::Display for LocalCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(
                formatter,
                "No OpenCode credentials found. Sign in with the OpenCode console first: \
                 npx @opencode/console login."
            ),
            Self::Unreadable(detail) => {
                write!(formatter, "Could not read OpenCode credentials: {}", detail)
            }
            Self::Malformed(detail) => write!(
                formatter,
                "OpenCode credentials are not in the expected shape: {}",
                detail
            ),
            Self::NoUsableToken => write!(
                formatter,
                "OpenCode credentials hold no usable token. Sign in with the OpenCode console \
                 again: npx @opencode/console login."
            ),
        }
    }
}
