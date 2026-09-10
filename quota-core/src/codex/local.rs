//! Read-only access to the credentials the Codex CLI stores locally.
//!
//! Quota never refreshes or rewrites this file. Rotating the refresh token here
//! would invalidate the copy the Codex CLI itself is using and break the user's
//! CLI, so a rejected token is reported rather than renewed.

use base64::Engine;
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// Where the Codex CLI writes its credentials. `CODEX_HOME` relocates it.
pub fn local_credentials_path() -> Option<PathBuf> {
    crate::local_paths::provider_home("CODEX_HOME", ".codex").map(|home| home.join("auth.json"))
}

#[derive(Clone)]
pub struct LocalCodexCredentials {
    pub access_token: String,
    /// Sent as the `ChatGPT-Account-Id` header when present.
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub plan: Option<String>,
}

/// Deliberately hand-written so a token can never reach the Debug output.
impl fmt::Debug for LocalCodexCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalCodexCredentials")
            .field("access_token", &"<redacted>")
            .field("account_id", &self.account_id)
            .field("email", &self.email)
            .field("plan", &self.plan)
            .finish()
    }
}

#[derive(Debug)]
pub enum LocalCredentialError {
    NotFound,
    Unreadable(String),
    Malformed(String),
    /// The file holds an API key rather than an OAuth login. The usage endpoint
    /// only answers for OAuth sessions, so there is nothing to report.
    ApiKeyOnly,
}

impl fmt::Display for LocalCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(
                formatter,
                "No Codex credentials found. Sign in with the Codex CLI first."
            ),
            Self::Unreadable(detail) => {
                write!(formatter, "Could not read Codex credentials: {}", detail)
            }
            Self::Malformed(detail) => write!(
                formatter,
                "Codex credentials are not in the expected shape: {}",
                detail
            ),
            Self::ApiKeyOnly => write!(
                formatter,
                "Codex is signed in with an API key, which has no usage window to report."
            ),
        }
    }
}

#[derive(Deserialize)]
struct AuthFile {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<serde_json::Value>,
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    id_token: Option<String>,
    access_token: Option<String>,
    account_id: Option<String>,
}

#[derive(Deserialize)]
struct JwtPayload {
    email: Option<String>,
    #[serde(rename = "https://api.openai.com/auth")]
    auth_data: Option<JwtAuthData>,
    #[serde(rename = "https://api.openai.com/profile")]
    profile_data: Option<JwtProfileData>,
}

#[derive(Deserialize)]
struct JwtProfileData {
    email: Option<String>,
}

#[derive(Deserialize)]
struct JwtAuthData {
    chatgpt_plan_type: Option<String>,
    account_id: Option<String>,
    chatgpt_account_id: Option<String>,
}

pub fn read_local_credentials_at(path: &Path) -> Result<LocalCodexCredentials, LocalCredentialError> {
    if !path.exists() {
        return Err(LocalCredentialError::NotFound);
    }

    let raw = std::fs::read_to_string(path)
        .map_err(|err| LocalCredentialError::Unreadable(err.to_string()))?;

    // Only position information from the serde error is kept. Its message can
    // quote the input it failed on, which here could be a token.
    let parsed: AuthFile = serde_json::from_str(&raw).map_err(|err| {
        LocalCredentialError::Malformed(format!(
            "JSON error at line {} column {}",
            err.line(),
            err.column()
        ))
    })?;

    let tokens = match parsed.tokens {
        Some(tokens) => tokens,
        None => {
            if has_api_key(parsed.openai_api_key.as_ref()) {
                return Err(LocalCredentialError::ApiKeyOnly);
            }
            return Err(LocalCredentialError::Malformed(
                "no tokens block".to_string(),
            ));
        }
    };

    let access_token = tokens
        .access_token
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .ok_or_else(|| LocalCredentialError::Malformed("no access token".to_string()))?;

    // The id token carries the account id the usage endpoint wants. A token we
    // cannot decode is not fatal, since the header is optional.
    let claims = tokens
        .id_token
        .as_deref()
        .and_then(|token| decode_jwt_claims(token).ok());

    let auth_data = claims.as_ref().and_then(|claims| claims.auth_data.as_ref());

    let account_id = auth_data
        .and_then(|data| {
            normalize(data.account_id.clone()).or_else(|| normalize(data.chatgpt_account_id.clone()))
        })
        .or_else(|| normalize(tokens.account_id.clone()));

    let email = claims.as_ref().and_then(|claims| {
        normalize(claims.email.clone()).or_else(|| {
            claims
                .profile_data
                .as_ref()
                .and_then(|profile| normalize(profile.email.clone()))
        })
    });

    let plan = auth_data.and_then(|data| normalize(data.chatgpt_plan_type.clone()));

    Ok(LocalCodexCredentials {
        access_token,
        account_id,
        email,
        plan,
    })
}

/// Convenience wrapper over the default path.
pub fn read_local_credentials() -> Result<LocalCodexCredentials, LocalCredentialError> {
    let path = local_credentials_path()
        .ok_or_else(|| LocalCredentialError::Unreadable("no home directory".to_string()))?;
    read_local_credentials_at(&path)
}

fn has_api_key(value: Option<&serde_json::Value>) -> bool {
    value
        .and_then(serde_json::Value::as_str)
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false)
}

fn decode_jwt_claims(token: &str) -> Result<JwtPayload, ()> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return Err(());
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| ())?;
    serde_json::from_slice(&bytes).map_err(|_| ())
}

fn normalize(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}
