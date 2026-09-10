//! Fetch Claude usage with the locally stored Claude Code token and normalize
//! the response into the shared usage model.

use crate::usage::{ProviderUsage, UsageWindow};
use serde_json::Value;

use super::local::{read_local_credentials, LocalCredentialError};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BETA_HEADER: &str = "oauth-2025-04-20";

/// Convert the raw usage payload into the two windows the sidebar shows.
/// Model-specific blocks in the response are deliberately ignored.
pub fn normalize_usage_response(raw: &Value, account_label: Option<String>) -> ProviderUsage {
    ProviderUsage {
        provider: "claude".to_string(),
        account_label,
        windows: vec![
            window_from("5h", raw.get("five_hour")),
            window_from("Wk", raw.get("seven_day")),
        ],
        note: None,
    }
}

fn window_from(label: &str, block: Option<&Value>) -> UsageWindow {
    UsageWindow {
        label: label.to_string(),
        remaining_percent: block
            .and_then(|item| item.get("utilization"))
            .and_then(Value::as_f64)
            .filter(|used| used.is_finite())
            .map(|used| (100.0 - used.round()).clamp(0.0, 100.0) as i32),
        reset_at: block
            .and_then(|item| item.get("resets_at"))
            .and_then(Value::as_str)
            .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
            .map(|stamp| stamp.timestamp()),
    }
}

#[derive(Debug)]
pub enum LocalUsageError {
    Credentials(LocalCredentialError),
    Unauthorized,
    Request(String),
}

impl std::fmt::Display for LocalUsageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credentials(inner) => write!(formatter, "{}", inner),
            Self::Unauthorized => write!(
                formatter,
                "Claude rejected the stored token. Run the Claude Code CLI to refresh it."
            ),
            Self::Request(detail) => write!(formatter, "Claude usage request failed: {}", detail),
        }
    }
}

/// Read local credentials and fetch current usage. Never writes to disk.
pub async fn fetch_local_usage() -> Result<ProviderUsage, LocalUsageError> {
    let credentials = read_local_credentials().map_err(LocalUsageError::Credentials)?;

    let response = reqwest::Client::new()
        .get(USAGE_URL)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", credentials.access_token),
        )
        .header("anthropic-beta", BETA_HEADER)
        .header(reqwest::header::USER_AGENT, "quota")
        .send()
        .await
        .map_err(|err| LocalUsageError::Request(err.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| LocalUsageError::Request(err.to_string()))?;

    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(LocalUsageError::Unauthorized);
    }
    if !status.is_success() {
        return Err(LocalUsageError::Request(format!(
            "status={} body_length={}",
            status.as_u16(),
            body.len()
        )));
    }

    let raw: Value =
        serde_json::from_str(&body).map_err(|err| LocalUsageError::Request(err.to_string()))?;

    Ok(normalize_usage_response(&raw, credentials.subscription_type))
}
