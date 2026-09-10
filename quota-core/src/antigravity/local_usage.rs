//! Fetch Antigravity usage with the locally stored Google token and normalize
//! the response into the shared usage model.
//!
//! Antigravity is the one provider Quota refreshes for. Google access tokens
//! last about an hour, so a strict never-refresh rule would leave the sidebar
//! blank almost always. Google does not rotate the refresh token on a standard
//! refresh, so the fresh access token is held in memory for the length of one
//! request and `oauth_creds.json` is never written. Antigravity's own copy of
//! the refresh token stays valid.

use crate::usage::{ProviderUsage, UsageWindow};
use serde_json::{json, Value};

use super::local::{read_any_credentials, LocalAntigravityCredentials, LocalCredentialError};

const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

/// Google binds a refresh token to the OAuth client that issued it, and two
/// different tools write `~/.gemini/oauth_creds.json`: Antigravity itself, and
/// the Gemini CLI it grew out of. Refreshing with the wrong one fails with
/// `unauthorized_client`, so the client is chosen by the id token's `aud`.
///
/// The Code Assist host travels with the client rather than being a single
/// constant, because different clients talk to different hosts.
///
/// Only Antigravity's own client is listed. The Gemini CLI, which writes the
/// same `oauth_creds.json`, uses a different client and a different host, but
/// its credentials are for Gemini Code Assist rather than Antigravity and
/// Quota has no reason to query them. Credentials from an unlisted client are
/// reported by name rather than guessed at, since the client determines both
/// how to refresh the token and which host to send it to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OauthClient {
    pub client_id: &'static str,
    pub client_secret: &'static str,
    pub code_assist_base: &'static str,
}

const OAUTH_CLIENTS: [OauthClient; 1] = [OauthClient {
    client_id: "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com",
    client_secret: "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",
    code_assist_base: "https://daily-cloudcode-pa.googleapis.com",
}];

/// Which client these credentials belong to, if we recognize it.
pub fn oauth_client_for(client_id: Option<&str>) -> Option<OauthClient> {
    let client_id = client_id?;
    OAUTH_CLIENTS
        .iter()
        .find(|client| client.client_id == client_id)
        .copied()
}

const LOAD_ENDPOINT: &str = "v1internal:loadCodeAssist";
const FETCH_MODELS_ENDPOINT: &str = "v1internal:fetchAvailableModels";
const RETRIEVE_QUOTA_ENDPOINT: &str = "v1internal:retrieveUserQuotaSummary";

const IDE_VERSION: &str = "1.20.5";
const GOOGLE_API_NODEJS_CLIENT_VERSION: &str = "10.3.0";
const X_GOOG_API_CLIENT: &str = "gl-node/22.21.1";

/// The two windows the compact token shows, in order.
const TOKEN_BUCKETS: [(&str, &str); 2] = [("gemini-5h", "5h"), ("gemini-weekly", "Wk")];
/// The other two, carried in the full model but kept out of the token. Four
/// numbers stop being compact.
const EXTRA_BUCKETS: [(&str, &str); 2] = [("3p-5h", "3p 5h"), ("3p-weekly", "3p Wk")];

/// Convert the quota summary into windows, Gemini first so the compact token
/// shows the pair people actually run Antigravity for.
///
/// Antigravity is the exception among these providers: it reports
/// `remainingFraction`, which is already remaining rather than used. Nothing is
/// inverted here.
pub fn normalize_quota_response(raw: &Value, account_label: Option<String>) -> ProviderUsage {
    let mut windows = Vec::with_capacity(4);
    for (bucket_id, label) in TOKEN_BUCKETS.iter().chain(EXTRA_BUCKETS.iter()) {
        windows.push(window_from(raw, bucket_id, label));
    }

    ProviderUsage {
        provider: "antigravity".to_string(),
        account_label,
        windows,
    }
}

fn window_from(raw: &Value, bucket_id: &str, label: &str) -> UsageWindow {
    let bucket = find_bucket(raw, bucket_id);
    UsageWindow {
        label: label.to_string(),
        remaining_percent: bucket
            .as_ref()
            .and_then(|item| item.get("remainingFraction"))
            .and_then(as_number)
            .map(|fraction| (fraction * 100.0).round().clamp(0.0, 100.0) as i32),
        reset_at: bucket
            .as_ref()
            .and_then(|item| item.get("resetTime"))
            .and_then(parse_reset_at),
    }
}

/// Buckets are nested inside groups, and the grouping carries no meaning we
/// need, so this flattens and matches on the bucket id.
fn find_bucket(raw: &Value, bucket_id: &str) -> Option<Value> {
    raw.get("groups")?
        .as_array()?
        .iter()
        .filter_map(|group| group.get("buckets")?.as_array())
        .flatten()
        .find(|bucket| bucket.get("bucketId").and_then(Value::as_str) == Some(bucket_id))
        .cloned()
}

/// Numbers sometimes arrive as strings in this API.
fn as_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|item| item.trim().parse::<f64>().ok()))
        .filter(|item| item.is_finite())
}

fn parse_reset_at(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return normalize_epoch(number);
    }
    let raw = value.as_str()?.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(number) = raw.parse::<i64>() {
        return normalize_epoch(number);
    }
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|date| date.timestamp())
}

fn normalize_epoch(number: i64) -> Option<i64> {
    if number <= 0 {
        return None;
    }
    Some(if number > 10_000_000_000 {
        number / 1000
    } else {
        number
    })
}

#[derive(Debug)]
pub enum LocalUsageError {
    Credentials(LocalCredentialError),
    ExpiredWithoutRefreshToken,
    UnknownOauthClient(Option<String>),
    RefreshRejected,
    NoProject,
    Unauthorized,
    Request(String),
}

impl std::fmt::Display for LocalUsageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credentials(inner) => write!(formatter, "{}", inner),
            Self::ExpiredWithoutRefreshToken => write!(
                formatter,
                "The stored Antigravity token has expired and there is no refresh token. Sign in to Antigravity again."
            ),
            Self::UnknownOauthClient(client_id) => match client_id {
                Some(id) => write!(
                    formatter,
                    "These credentials were issued to OAuth client {}, which is not Antigravity's. They are most likely Gemini Code Assist's, which Quota does not report on. Sign in through Antigravity itself.",
                    id
                ),
                None => write!(
                    formatter,
                    "These credentials do not say which OAuth client issued them, so Quota cannot tell whether they are Antigravity's. Sign in through Antigravity again."
                ),
            },
            Self::RefreshRejected => write!(
                formatter,
                "Google rejected the stored Antigravity refresh token. Sign in to Antigravity again."
            ),
            Self::NoProject => write!(
                formatter,
                "Antigravity did not report a project to query usage for."
            ),
            Self::Unauthorized => write!(
                formatter,
                "Antigravity rejected the stored token. Sign in to Antigravity again."
            ),
            Self::Request(detail) => {
                write!(formatter, "Antigravity usage request failed: {}", detail)
            }
        }
    }
}

/// Read local credentials and fetch current usage. Never writes to disk.
pub async fn fetch_local_usage() -> Result<ProviderUsage, LocalUsageError> {
    let credentials = read_any_credentials().map_err(LocalUsageError::Credentials)?;
    let client = reqwest::Client::new();

    let oauth = oauth_client_for(credentials.issued_to_client_id.as_deref()).ok_or_else(|| {
        LocalUsageError::UnknownOauthClient(credentials.issued_to_client_id.clone())
    })?;

    let access_token = resolve_access_token(&client, &credentials, &oauth).await?;

    // loadCodeAssist names the project the quota call needs.
    let load = post_code_assist(
        &client,
        &oauth,
        &access_token,
        LOAD_ENDPOINT,
        &json!({
            "mode": "FULL_ELIGIBILITY_CHECK",
            "metadata": {
                "ideName": "antigravity",
                "ideType": "ANTIGRAVITY",
                "ideVersion": IDE_VERSION,
                "pluginVersion": env!("CARGO_PKG_NAME"),
                "platform": platform_name(),
                "updateChannel": "stable",
                "pluginType": "GEMINI"
            }
        }),
    )
    .await?;

    let project = project_id(&load).ok_or(LocalUsageError::NoProject)?;
    let project_payload = json!({ "project": project });

    // The quota endpoint answers with empty buckets unless the models call has
    // run first, which is why this request is made and its body discarded.
    post_code_assist(
        &client,
        &oauth,
        &access_token,
        FETCH_MODELS_ENDPOINT,
        &project_payload,
    )
    .await?;

    let quota = post_code_assist(
        &client,
        &oauth,
        &access_token,
        RETRIEVE_QUOTA_ENDPOINT,
        &project_payload,
    )
    .await?;

    Ok(normalize_quota_response(
        &quota,
        credentials.email.clone().or_else(|| tier_name(&load)),
    ))
}

/// Use the stored token while it is good, otherwise trade the refresh token for
/// a fresh one that lives only as long as this call.
async fn resolve_access_token(
    client: &reqwest::Client,
    credentials: &LocalAntigravityCredentials,
    oauth: &OauthClient,
) -> Result<String, LocalUsageError> {
    if !credentials.needs_refresh_at_ms(chrono::Utc::now().timestamp_millis()) {
        return Ok(credentials.access_token.clone());
    }

    let refresh_token = credentials
        .refresh_token
        .as_deref()
        .ok_or(LocalUsageError::ExpiredWithoutRefreshToken)?;

    let response = client
        .post(GOOGLE_TOKEN_ENDPOINT)
        .form(&[
            ("client_id", oauth.client_id),
            ("client_secret", oauth.client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|err| LocalUsageError::Request(err.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| LocalUsageError::Request(err.to_string()))?;

    if !status.is_success() {
        // The body of a failed token exchange can echo credential material, so
        // only the status and length are reported.
        if status.as_u16() == 400 || status.as_u16() == 401 {
            return Err(LocalUsageError::RefreshRejected);
        }
        return Err(LocalUsageError::Request(format!(
            "token refresh status={} body_length={}",
            status.as_u16(),
            body.len()
        )));
    }

    let parsed: Value = serde_json::from_str(&body)
        .map_err(|_| LocalUsageError::Request("token refresh returned unreadable JSON".to_string()))?;

    parsed
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|token| !token.is_empty())
        .ok_or(LocalUsageError::RefreshRejected)
}

async fn post_code_assist(
    client: &reqwest::Client,
    oauth: &OauthClient,
    access_token: &str,
    endpoint: &str,
    payload: &Value,
) -> Result<Value, LocalUsageError> {
    let url = format!("{}/{}", oauth.code_assist_base, endpoint);
    let response = client
        .post(&url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", access_token),
        )
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "*/*")
        .header(reqwest::header::USER_AGENT, user_agent(endpoint))
        .header("x-goog-api-client", X_GOOG_API_CLIENT)
        .json(payload)
        .send()
        .await
        .map_err(|err| LocalUsageError::Request(err.to_string()))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| LocalUsageError::Request(err.to_string()))?;

    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(LocalUsageError::Unauthorized);
    }
    if !status.is_success() {
        return Err(LocalUsageError::Request(format!(
            "{} status={} body_length={}",
            endpoint,
            status.as_u16(),
            text.len()
        )));
    }

    if text.trim().is_empty() {
        return Ok(json!({}));
    }

    serde_json::from_str(&text)
        .map_err(|_| LocalUsageError::Request(format!("{} returned unreadable JSON", endpoint)))
}

/// The project id arrives either as a bare string or as an object.
pub fn project_id(load: &Value) -> Option<String> {
    let field = load.get("cloudaicompanionProject")?;
    field
        .as_str()
        .or_else(|| field.get("id").and_then(Value::as_str))
        .or_else(|| field.get("projectId").and_then(Value::as_str))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

/// A display label when no signed-in address is known.
fn tier_name(load: &Value) -> Option<String> {
    ["paidTier", "currentTier"]
        .iter()
        .filter_map(|key| load.get(*key)?.get("name")?.as_str())
        .map(str::trim)
        .find(|item| !item.is_empty())
        .map(str::to_string)
}

fn user_agent(endpoint: &str) -> String {
    let base = format!("antigravity/{} {}/{}", IDE_VERSION, os_name(), arch_name());
    if endpoint.contains(LOAD_ENDPOINT) {
        format!(
            "{} google-api-nodejs-client/{}",
            base, GOOGLE_API_NODEJS_CLIENT_VERSION
        )
    } else {
        base
    }
}

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        _ => "windows",
    }
}

fn arch_name() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        _ => "amd64",
    }
}

fn platform_name() -> &'static str {
    match (os_name(), arch_name()) {
        ("darwin", "amd64") => "DARWIN_AMD64",
        ("darwin", "arm64") => "DARWIN_ARM64",
        ("linux", "amd64") => "LINUX_AMD64",
        ("linux", "arm64") => "LINUX_ARM64",
        ("windows", "amd64") => "WINDOWS_AMD64",
        _ => "PLATFORM_UNSPECIFIED",
    }
}
