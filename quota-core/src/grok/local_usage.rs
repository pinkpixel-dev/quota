//! Fetch Grok usage with the locally stored Grok CLI token and normalize the
//! response into the shared usage model.
//!
//! Grok bills against credits rather than rolling time windows, so this
//! produces a single window rather than the pair Claude and Codex produce.

use crate::usage::{ProviderUsage, UsageWindow};
use serde_json::Value;

use super::local::{parse_timestamp, read_local_credentials, LocalCredentialError};

const BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing";

/// Convert the raw billing payload into the single credit window.
///
/// The API reports `creditUsagePercent`, so the number here is inverted into
/// the remaining percent the shared model stores.
pub fn normalize_billing_response(raw: &Value, account_label: Option<String>) -> ProviderUsage {
    let config = raw.get("config");

    let used_percent = config
        .and_then(|item| item.get("creditUsagePercent"))
        .and_then(Value::as_f64)
        .filter(|used| used.is_finite())
        .or_else(|| highest_product_usage(config));

    let label = account_label.or_else(|| {
        config
            .and_then(|item| item.get("subscriptionTier"))
            .and_then(Value::as_str)
            .map(humanize_subscription_tier)
    });

    ProviderUsage {
        provider: "grok".to_string(),
        account_label: label,
        windows: vec![UsageWindow {
            label: "Credit".to_string(),
            remaining_percent: used_percent
                .map(|used| (100.0 - used.round()).clamp(0.0, 100.0) as i32),
            reset_at: period_end(config),
        }],
    }
}

/// When the payload carries no overall percent, the worst product is the
/// honest headline. Reporting the average would hide a product that is
/// already exhausted.
fn highest_product_usage(config: Option<&Value>) -> Option<f64> {
    config?
        .get("productUsage")?
        .as_array()?
        .iter()
        .filter_map(|item| item.get("usagePercent").and_then(Value::as_f64))
        .filter(|used| used.is_finite())
        .fold(None, |acc: Option<f64>, value| {
            Some(acc.map_or(value, |current| current.max(value)))
        })
}

fn period_end(config: Option<&Value>) -> Option<i64> {
    let config = config?;
    let from_period = config
        .get("currentPeriod")
        .and_then(|item| item.get("end"))
        .and_then(Value::as_str)
        .and_then(parse_timestamp);
    from_period.or_else(|| {
        config
            .get("billingPeriodEnd")
            .and_then(Value::as_str)
            .and_then(parse_timestamp)
    })
}

/// Turn `SUBSCRIPTION_TIER_X_PLUS` into `X Plus`, matching what the desktop
/// app shows for the same account.
fn humanize_subscription_tier(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches("SUBSCRIPTION_TIER_");
    if trimmed.is_empty() {
        return raw.trim().to_string();
    }
    trimmed
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| match part {
            "X" => "X".to_string(),
            "PLUS" => "Plus".to_string(),
            other => capitalize(other),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

#[derive(Debug)]
pub enum LocalUsageError {
    Credentials(LocalCredentialError),
    Expired,
    Unauthorized,
    Request(String),
}

impl std::fmt::Display for LocalUsageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credentials(inner) => write!(formatter, "{}", inner),
            Self::Expired => write!(
                formatter,
                "The stored Grok token has expired. Run the Grok CLI to refresh it."
            ),
            Self::Unauthorized => write!(
                formatter,
                "Grok rejected the stored token. Run the Grok CLI to refresh it."
            ),
            Self::Request(detail) => write!(formatter, "Grok billing request failed: {}", detail),
        }
    }
}

/// Read local credentials and fetch current usage. Never writes to disk.
pub async fn fetch_local_usage() -> Result<ProviderUsage, LocalUsageError> {
    let credentials = read_local_credentials().map_err(LocalUsageError::Credentials)?;

    // A known-expired token is reported without a request. Grok would reject it
    // anyway, and the expiry message tells the user what to do about it.
    if credentials.is_expired_at(chrono::Utc::now().timestamp()) {
        return Err(LocalUsageError::Expired);
    }

    let response = reqwest::Client::new()
        .get(format!("{}?format=credits", BILLING_URL))
        .header(reqwest::header::ACCEPT, "application/json")
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", credentials.access_token),
        )
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

    Ok(normalize_billing_response(&raw, None))
}
