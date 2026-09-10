//! Fetch Kiro usage with the locally stored token and normalize the response
//! into the shared usage model.
//!
//! Kiro bills against a credit pool rather than rolling time windows, and some
//! plans carry a second bonus or free-trial pool alongside it.

use crate::usage::{ProviderUsage, UsageWindow};
use serde_json::Value;

use super::local::{read_local_credentials, LocalCredentialError};

const DEFAULT_RUNTIME_ENDPOINT: &str = "https://q.us-east-1.amazonaws.com";

/// Convert the raw usage payload into the credit windows.
///
/// Kiro reports totals and amounts consumed rather than a percentage, so the
/// remaining percent is computed here. A missing or zero total yields no
/// number: dividing by it would either panic or report a full pool for an
/// account that has none.
pub fn normalize_usage_response(raw: &Value, account_label: Option<String>) -> ProviderUsage {
    let breakdown = primary_breakdown(raw);

    let credits_total = pick_number(
        Some(raw),
        &[
            &["estimatedUsage", "total"],
            &["usageBreakdowns", "plan", "totalCredits"],
        ],
    )
    .or_else(|| {
        pick_number(
            breakdown,
            &[
                &["usageLimitWithPrecision"],
                &["usageLimit"],
                &["limit"],
                &["total"],
                &["totalCredits"],
            ],
        )
    });

    let credits_used = pick_number(
        Some(raw),
        &[
            &["estimatedUsage", "used"],
            &["usageBreakdowns", "plan", "usedCredits"],
        ],
    )
    .or_else(|| {
        pick_number(
            breakdown,
            &[
                &["currentUsageWithPrecision"],
                &["currentUsage"],
                &["used"],
                &["usedCredits"],
            ],
        )
    });

    let free_trial = breakdown.and_then(|item| {
        item.get("freeTrialUsage")
            .or_else(|| item.get("freeTrialInfo"))
    });

    let bonus_total = pick_number(
        free_trial,
        &[
            &["usageLimitWithPrecision"],
            &["usageLimit"],
            &["limit"],
            &["total"],
        ],
    )
    .or_else(|| pick_number(Some(raw), &[&["bonusCredits", "total"], &["bonus", "total"]]));

    let bonus_used = pick_number(
        free_trial,
        &[&["currentUsageWithPrecision"], &["currentUsage"], &["used"]],
    )
    .or_else(|| pick_number(Some(raw), &[&["bonusCredits", "used"], &["bonus", "used"]]));

    let mut windows = vec![UsageWindow {
        label: "Credits".to_string(),
        remaining_percent: remaining_percent(credits_total, credits_used),
        reset_at: None,
    }];

    // Only when there is actually a bonus pool. An absent one must not add an
    // empty row to every account's sidebar token.
    if bonus_total.is_some_and(|total| total > 0.0) {
        windows.push(UsageWindow {
            label: "Bonus".to_string(),
            remaining_percent: remaining_percent(bonus_total, bonus_used),
            reset_at: None,
        });
    }

    let label = account_label.or_else(|| {
        pick_string(
            breakdown,
            &[&["displayName"], &["displayNamePlural"], &["type"]],
        )
    });

    ProviderUsage {
        provider: "kiro".to_string(),
        account_label: label,
        windows,
        note: None,
    }
}

/// Remaining share of a pool, or `None` when the pool cannot be measured.
fn remaining_percent(total: Option<f64>, used: Option<f64>) -> Option<i32> {
    let total = total?;
    if total <= 0.0 {
        return None;
    }
    let used = used.unwrap_or(0.0).max(0.0);
    Some((100.0 - (used / total * 100.0)).clamp(0.0, 100.0).round() as i32)
}

/// Kiro sends a list of pools. The credit one is the subscription; anything
/// else is a fallback so an unfamiliar payload still reports something.
fn primary_breakdown(raw: &Value) -> Option<&Value> {
    let list = raw
        .get("usageBreakdownList")
        .and_then(Value::as_array)
        .or_else(|| raw.get("usageBreakdowns").and_then(Value::as_array))?;

    list.iter()
        .find(|item| {
            item.get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("credit"))
        })
        .or_else(|| list.first())
}

/// Kiro sends these amounts as JSON numbers in some payloads and as strings in
/// others, so both are accepted.
fn pick_number(root: Option<&Value>, paths: &[&[&str]]) -> Option<f64> {
    let root = root?;
    for path in paths {
        let Some(value) = get_path(root, path) else {
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

fn pick_string(root: Option<&Value>, paths: &[&[&str]]) -> Option<String> {
    let root = root?;
    for path in paths {
        if let Some(text) = get_path(root, path).and_then(Value::as_str) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn get_path<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for part in path {
        current = current.get(*part)?;
    }
    Some(current)
}

/// The region lives in the third segment of the profile ARN.
fn region_from_profile_arn(arn: &str) -> Option<String> {
    let mut parts = arn.split(':');
    if !parts.next()?.trim().eq_ignore_ascii_case("arn") {
        return None;
    }
    parts.next()?; // partition
    parts.next()?; // service
    let region = parts.next()?.trim();
    (!region.is_empty()).then(|| region.to_string())
}

fn runtime_endpoint_for_region(region: Option<&str>) -> String {
    match region.unwrap_or("us-east-1").trim().to_ascii_lowercase().as_str() {
        "eu-central-1" => "https://q.eu-central-1.amazonaws.com".to_string(),
        _ => DEFAULT_RUNTIME_ENDPOINT.to_string(),
    }
}

/// Build the usage URL for a profile. Public so a test can check the region
/// routing without making a request.
pub fn usage_url_for_profile_arn(profile_arn: &str) -> String {
    let endpoint = runtime_endpoint_for_region(region_from_profile_arn(profile_arn).as_deref());
    format!(
        "{}/getUsageLimits?origin=AI_EDITOR&profileArn={}&resourceType=AGENTIC_REQUEST&isEmailRequired=true",
        endpoint.trim_end_matches('/'),
        urlencoding::encode(profile_arn),
    )
}

#[derive(Debug)]
pub enum LocalUsageError {
    Credentials(LocalCredentialError),
    /// The cached token lapsed. Reported without a request, because Kiro
    /// answers a lapsed token with the same 403 it uses for a disabled account.
    Expired,
    Unauthorized,
    /// Kiro answers 403 for an account that has been disabled, which is worth
    /// telling apart from an expired token the user can simply refresh.
    Forbidden,
    Request(String),
}

impl std::fmt::Display for LocalUsageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credentials(inner) => write!(formatter, "{}", inner),
            Self::Expired => write!(
                formatter,
                "The stored Kiro token has expired. Sign in to Kiro again."
            ),
            Self::Unauthorized => write!(
                formatter,
                "Kiro rejected the stored token. Sign in to Kiro again."
            ),
            Self::Forbidden => write!(
                formatter,
                "Kiro refused the request for this account. It may be disabled."
            ),
            Self::Request(detail) => write!(formatter, "Kiro usage request failed: {}", detail),
        }
    }
}

/// Read local credentials and fetch current usage. Never writes to disk.
pub async fn fetch_local_usage() -> Result<ProviderUsage, LocalUsageError> {
    let credentials = read_local_credentials().map_err(LocalUsageError::Credentials)?;

    if credentials.is_expired_at(chrono::Utc::now().timestamp()) {
        return Err(LocalUsageError::Expired);
    }

    let response = reqwest::Client::new()
        .get(usage_url_for_profile_arn(&credentials.profile_arn))
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

    if status.as_u16() == 401 {
        return Err(LocalUsageError::Unauthorized);
    }
    if status.as_u16() == 403 {
        return Err(LocalUsageError::Forbidden);
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
