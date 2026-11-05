//! Read-only access to Kimi Code's OAuth credentials. Token renewal belongs to
//! Kimi Code so Quota cannot invalidate the CLI's refresh token.

use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

pub fn local_credentials_path() -> Option<PathBuf> {
    crate::local_paths::provider_home("KIMI_CODE_HOME", ".kimi-code")
        .map(|home| home.join("credentials").join("kimi-code.json"))
}

#[derive(Clone)]
pub struct LocalKimiCredentials {
    pub access_token: String,
    pub expires_at: f64,
}

impl fmt::Debug for LocalKimiCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalKimiCredentials")
            .field("access_token", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Debug)]
pub enum LocalCredentialError {
    NotFound,
    Unreadable(String),
    Malformed(String),
    Expired,
    SignedOut,
}

impl fmt::Display for LocalCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(
                formatter,
                "No Kimi Code OAuth credentials found. Run /login in Kimi Code first."
            ),
            Self::Unreadable(detail) => {
                write!(
                    formatter,
                    "Could not read Kimi Code credentials: {}",
                    detail
                )
            }
            Self::Malformed(detail) => write!(
                formatter,
                "Kimi Code credentials are not in the expected shape: {}. Run /login in Kimi Code.",
                detail
            ),
            Self::Expired => write!(
                formatter,
                "Kimi Code's stored token has expired. Run Kimi Code to refresh it, or use /login."
            ),
            Self::SignedOut => write!(
                formatter,
                "Kimi Code is signed out. Run /login in Kimi Code."
            ),
        }
    }
}

#[derive(Deserialize)]
struct AuthFile {
    access_token: String,
    expires_at: f64,
}

pub fn read_local_credentials_at(
    path: &Path,
) -> Result<LocalKimiCredentials, LocalCredentialError> {
    let raw = std::fs::read_to_string(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            LocalCredentialError::NotFound
        } else {
            LocalCredentialError::Unreadable(err.to_string())
        }
    })?;

    // serde's full error can quote a credential value; retain only its location.
    let parsed: AuthFile = serde_json::from_str(&raw).map_err(|err| {
        LocalCredentialError::Malformed(format!(
            "JSON error at line {} column {}",
            err.line(),
            err.column()
        ))
    })?;
    let access_token = parsed.access_token.trim().to_string();
    if access_token.is_empty() {
        return Err(LocalCredentialError::SignedOut);
    }
    if !parsed.expires_at.is_finite() || parsed.expires_at <= 0.0 {
        return Err(LocalCredentialError::Malformed(
            "invalid expiry timestamp".to_string(),
        ));
    }
    if parsed.expires_at <= chrono::Utc::now().timestamp() as f64 {
        return Err(LocalCredentialError::Expired);
    }

    Ok(LocalKimiCredentials {
        access_token,
        expires_at: parsed.expires_at,
    })
}

pub fn read_local_credentials() -> Result<LocalKimiCredentials, LocalCredentialError> {
    let path = local_credentials_path()
        .ok_or_else(|| LocalCredentialError::Unreadable("no home directory".to_string()))?;
    read_local_credentials_at(&path)
}
