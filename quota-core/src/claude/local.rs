//! Read-only access to the credentials the Claude Code CLI stores locally.
//!
//! Quota never refreshes or rewrites this file. Rotating the refresh token here
//! would invalidate the copy Claude Code itself is using and break the user's
//! CLI, so an expired token is reported rather than renewed.

use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// Where the Claude Code CLI writes its credentials.
pub fn local_credentials_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude").join(".credentials.json"))
}

#[derive(Clone)]
pub struct LocalClaudeCredentials {
    pub access_token: String,
    pub expires_at_ms: Option<i64>,
    pub scopes: Vec<String>,
    pub subscription_type: Option<String>,
    pub organization_uuid: Option<String>,
}

impl LocalClaudeCredentials {
    /// True when the stored expiry is at or before `now_ms`. Unknown expiry is
    /// treated as usable, because the endpoint is the real authority.
    pub fn is_expired_at_ms(&self, now_ms: i64) -> bool {
        match self.expires_at_ms {
            Some(expiry) => expiry <= now_ms,
            None => false,
        }
    }
}

/// Deliberately hand-written so a token can never reach the Debug output.
impl fmt::Debug for LocalClaudeCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalClaudeCredentials")
            .field("access_token", &"<redacted>")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("scopes", &self.scopes)
            .field("subscription_type", &self.subscription_type)
            .field("organization_uuid", &self.organization_uuid)
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
                "No Claude Code credentials found. Sign in with the Claude Code CLI first."
            ),
            Self::Unreadable(detail) => {
                write!(formatter, "Could not read Claude Code credentials: {}", detail)
            }
            Self::Malformed(detail) => write!(
                formatter,
                "Claude Code credentials are not in the expected shape: {}",
                detail
            ),
        }
    }
}

#[derive(Deserialize)]
struct CredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<OauthBlock>,
    #[serde(rename = "organizationUuid")]
    organization_uuid: Option<String>,
}

#[derive(Deserialize)]
struct OauthBlock {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
}

pub fn read_local_credentials_at(path: &Path) -> Result<LocalClaudeCredentials, LocalCredentialError> {
    if !path.exists() {
        return Err(LocalCredentialError::NotFound);
    }

    let raw = std::fs::read_to_string(path)
        .map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;

    let parsed: CredentialsFile = serde_json::from_str(&raw)
        .map_err(|err| LocalCredentialError::Malformed(err.to_string()))?;

    let oauth = parsed
        .oauth
        .ok_or_else(|| LocalCredentialError::Malformed("no claudeAiOauth block".to_string()))?;

    let access_token = oauth
        .access_token
        .filter(|token| !token.is_empty())
        .ok_or_else(|| LocalCredentialError::Malformed("no access token".to_string()))?;

    Ok(LocalClaudeCredentials {
        access_token,
        expires_at_ms: oauth.expires_at,
        scopes: oauth.scopes,
        subscription_type: oauth.subscription_type,
        organization_uuid: parsed.organization_uuid,
    })
}

/// Convenience wrapper over the default path.
pub fn read_local_credentials() -> Result<LocalClaudeCredentials, LocalCredentialError> {
    let path = local_credentials_path()
        .ok_or_else(|| LocalCredentialError::Unreadable("no home directory".to_string()))?;
    read_local_credentials_at(&path)
}
