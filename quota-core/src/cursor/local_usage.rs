//! Fetch Cursor usage with the locally stored Cursor CLI token and normalize
//! the response into the shared usage model.

use crate::usage::{ProviderUsage, UsageWindow};
use serde_json::Value;

use super::local::{read_local_credentials, LocalCredentialError};

const USAGE_URL: &str = "https://cursor.com/api/usage-summary";

/// Cursor bills on one plan cycle rather than rolling windows, so there is a
/// single window and its label says so.
const WINDOW_LABEL: &str = "Plan";

/// Convert the raw usage payload into the single window the sidebar shows.
///
/// The API reports `totalPercentUsed`, so the number is inverted into the
/// remaining percent the shared model stores. This inversion happens here and
/// nowhere else.
pub fn normalize_usage_response(raw: &Value, account_label: Option<String>) -> ProviderUsage {
    let plan = get_path(raw, &["individualUsage", "plan"])
        .or_else(|| get_path(raw, &["individual_usage", "plan"]))
        .or_else(|| raw.get("planUsage"))
        .or_else(|| raw.get("plan_usage"));

    let remaining_percent = pick_f64(plan, &["totalPercentUsed", "total_percent_used"])
        .map(|used| (100.0 - used.round()).clamp(0.0, 100.0) as i32);

    ProviderUsage {
        provider: "cursor".to_string(),
        account_label,
        windows: vec![UsageWindow {
            label: WINDOW_LABEL.to_string(),
            remaining_percent,
            reset_at: billing_cycle_end(raw),
        }],
    }
}

fn get_path<'a>(root: &'a Value, parts: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for part in parts {
        current = current.as_object()?.get(*part)?;
    }
    Some(current)
}

/// Cursor sends these numbers as JSON numbers in some responses and as strings
/// in others, so both are accepted here.
fn pick_f64(object: Option<&Value>, keys: &[&str]) -> Option<f64> {
    let object = object?.as_object()?;
    for key in keys {
        let Some(value) = object.get(*key) else {
            continue;
        };
        let parsed = value
            .as_f64()
            .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()));
        if let Some(number) = parsed.filter(|number: &f64| number.is_finite()) {
            return Some(number);
        }
    }
    None
}

/// The plan cycle end doubles as the window reset. Cursor sends it as RFC 3339.
fn billing_cycle_end(raw: &Value) -> Option<i64> {
    let value = raw
        .get("billingCycleEnd")
        .or_else(|| raw.get("billing_cycle_end"))?
        .as_str()?;
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.timestamp())
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
                "Cursor rejected the stored token. Run the Cursor CLI to sign in again."
            ),
            Self::Request(detail) => write!(formatter, "Cursor usage request failed: {}", detail),
        }
    }
}

/// Read local credentials and fetch current usage. Never writes to disk.
///
/// The stored file carries no expiry, so a signed-out session can only show up
/// as a rejection from this call rather than a cheap local check.
pub async fn fetch_local_usage() -> Result<ProviderUsage, LocalUsageError> {
    let credentials = read_local_credentials().map_err(LocalUsageError::Credentials)?;

    let cookie = credentials.session_cookie().ok_or_else(|| {
        LocalUsageError::Credentials(LocalCredentialError::Malformed(
            "access token is not a WorkOS session JWT".to_string(),
        ))
    })?;

    // Cursor's web API answers to the browser session cookie, not to a bearer
    // token, and it is picky about the user agent it will serve.
    let response = reqwest::Client::new()
        .get(USAGE_URL)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::COOKIE, cookie)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        )
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

    Ok(normalize_usage_response(&raw, None))
}
