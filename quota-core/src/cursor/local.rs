//! Read-only access to the credentials the Cursor CLI stores locally.
//!
//! Cursor keeps two separate credential stores. The CLI writes a flat JSON file
//! at `~/.config/cursor/auth.json`, while the IDE keeps its token in the SQLite
//! database under `~/.config/Cursor`, note the capital C. They are written by
//! different sign-ins and can hold different accounts. Quota's sidebar reports
//! on the agent running in a Herdr pane, and that agent is the CLI, so this
//! reads the CLI's file and leaves the IDE store to the desktop app.
//!
//! Quota never refreshes or rewrites this file. It holds a refresh token the
//! Cursor CLI is also using, and rotating it would sign the user's own CLI out,
//! so a rejected token is reported rather than renewed. The refresh token is
//! deliberately not carried on the credentials struct: nothing here may use it,
//! and a field that exists invites a future caller to try.

use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// Where the Cursor CLI writes its credentials. `XDG_CONFIG_HOME` relocates it.
pub fn local_credentials_path() -> Option<PathBuf> {
    crate::local_paths::config_home().map(|config| config.join("cursor").join("auth.json"))
}

#[derive(Clone)]
pub struct LocalCursorCredentials {
    pub access_token: String,
}

/// Deliberately hand-written so a token can never reach the Debug output.
impl fmt::Debug for LocalCursorCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalCursorCredentials")
            .field("access_token", &"<redacted>")
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
                "No Cursor credentials found. Sign in with the Cursor CLI first."
            ),
            Self::Unreadable(detail) => {
                write!(formatter, "Could not read Cursor credentials: {}", detail)
            }
            Self::Malformed(detail) => write!(
                formatter,
                "Cursor credentials are not in the expected shape: {}",
                detail
            ),
        }
    }
}

/// The CLI writes camelCase. The snake_case aliases cost nothing and keep a
/// format change from turning into an unexplained "no access token".
#[derive(Deserialize)]
struct AuthFile {
    #[serde(alias = "accessToken")]
    access_token: Option<String>,
}

pub fn read_local_credentials_at(
    path: &Path,
) -> Result<LocalCursorCredentials, LocalCredentialError> {
    if !path.exists() {
        return Err(LocalCredentialError::NotFound);
    }

    let raw = std::fs::read_to_string(path)
        .map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;

    // Only position information from the serde error is kept. Its message can
    // quote the input it failed on, which here would be a token.
    let parsed: AuthFile = serde_json::from_str(&raw).map_err(|err| {
        LocalCredentialError::Malformed(format!(
            "JSON error at line {} column {}",
            err.line(),
            err.column()
        ))
    })?;

    let access_token = parsed
        .access_token
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .ok_or_else(|| LocalCredentialError::Malformed("no access token".to_string()))?;

    Ok(LocalCursorCredentials { access_token })
}

/// Convenience wrapper over the default path.
pub fn read_local_credentials() -> Result<LocalCursorCredentials, LocalCredentialError> {
    let path = local_credentials_path()
        .ok_or_else(|| LocalCredentialError::Unreadable("no home directory".to_string()))?;
    read_local_credentials_at(&path)
}
