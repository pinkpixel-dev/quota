//! Fetch OpenCode Go usage with the locally stored console token and normalize
//! the response into the shared usage model.
//!
//! The endpoint is undocumented, but it is the same one the OpenCode console
//! reads, and it accepts the console's own token as the bearer. The desktop
//! app calls it with a pasted Go API key; this path needs no key at all.

use crate::usage::{ProviderUsage, UsageWindow};
use chrono::Utc;
use serde_json::Value;

use super::local::{LocalCredentialError, LocalOpenCodeCredentials};

const USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";

/// A rolling window that has not been opened yet comes back as 0% used with a
/// placeholder reset a full five hours out. A window that has really started
/// resets sooner than that.
const ROLLING_WINDOW_SECONDS: i64 = 5 * 60 * 60;
/// Allows for the request round trip and small clock differences.
const ROLLING_PLACEHOLDER_TOLERANCE_SECONDS: i64 = 120;

#[derive(Debug)]
pub enum LocalUsageError {
    Credentials(LocalCredentialError),
    Unauthorized,
    NoSubscription,
    Request(String),
}

impl std::fmt::Display for LocalUsageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credentials(inner) => write!(formatter, "{}", inner),
            Self::Unauthorized => write!(
                formatter,
                "OpenCode rejected the stored console token. Sign in with the OpenCode console \
                 again: npx @opencode/console login."
            ),
            Self::NoSubscription => {
                write!(formatter, "This account has no OpenCode Go subscription.")
            }
            Self::Request(detail) => {
                write!(formatter, "OpenCode Go usage request failed: {}", detail)
            }
        }
    }
}

/// Turn the raw usage payload into the shared model. `now` is in seconds since
/// the epoch, so the placeholder-reset rule for an unstarted rolling window
/// can be tested without a clock.
pub fn normalize_usage_response(raw: &Value, now: i64) -> ProviderUsage {
    let usage = raw.get("usage");

    let mut windows = vec![
        window_from(usage.and_then(|u| u.get("rolling")), "5h"),
        window_from(usage.and_then(|u| u.get("weekly")), "Wk"),
        window_from(usage.and_then(|u| u.get("monthly")), "Mo"),
    ];
    mark_unstarted_rolling(&mut windows[0], now);
    windows.retain(|w| w.remaining_percent.is_some());

    // A body with none of the three windows is a change in the undocumented
    // response shape, and saying so beats rendering a blank pane.
    let note = if windows.is_empty() {
        Some("no usage windows reported".to_string())
    } else {
        None
    };

    ProviderUsage {
        provider: "opencode-go".to_string(),
        account_label: None,
        windows,
        note,
    }
}

/// Build one window from its raw block. The endpoint reports how much has
/// been used, so the number is inverted into the remaining percent the shared
/// model stores.
fn window_from(block: Option<&Value>, label: &str) -> UsageWindow {
    let used = block
        .and_then(|b| b.get("percent"))
        .and_then(Value::as_f64)
        .filter(|used| used.is_finite());

    UsageWindow {
        label: label.to_string(),
        short_label: None,
        remaining_percent: used.map(|used| (100.0 - used.round()).clamp(0.0, 100.0) as i32),
        reset_at: block
            .and_then(|b| b.get("resetsAt"))
            .and_then(Value::as_str)
            .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
            .map(|parsed| parsed.timestamp()),
    }
}

/// An unused rolling window (nothing used, reset a full five hours out) has
/// not actually started, so there is nothing to roll over yet. Keeping the
/// placeholder would tell the user a window is closing when it is not.
fn mark_unstarted_rolling(window: &mut UsageWindow, now: i64) {
    let unused = window
        .remaining_percent
        .is_some_and(|remaining| remaining == 100);
    let full_window_ahead = window.reset_at.is_some_and(|reset| {
        reset - now >= ROLLING_WINDOW_SECONDS - ROLLING_PLACEHOLDER_TOLERANCE_SECONDS
    });
    if unused && full_window_ahead {
        window.reset_at = None;
    }
}

/// Read local credentials and fetch current usage. Never writes to disk.
pub async fn fetch_local_usage() -> Result<ProviderUsage, LocalUsageError> {
    let credentials = LocalOpenCodeCredentials::read().map_err(LocalUsageError::Credentials)?;
    fetch_usage_at(USAGE_URL, &credentials.access_token).await
}

/// The request itself, split out so tests can point it at a local listener.
async fn fetch_usage_at(url: &str, access_token: &str) -> Result<ProviderUsage, LocalUsageError> {
    let response = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", access_token),
        )
        // The endpoint sits behind Cloudflare, which bans the default
        // python-urllib User-Agent; every other reader here sends "quota".
        .header(reqwest::header::USER_AGENT, "quota")
        .send()
        .await
        .map_err(|err| LocalUsageError::Request(err.to_string()))?;

    let status = response.status();
    if status.as_u16() == 401 {
        return Err(LocalUsageError::Unauthorized);
    }
    if status.as_u16() == 403 {
        return Err(LocalUsageError::NoSubscription);
    }
    if !status.is_success() {
        return Err(LocalUsageError::Request(format!(
            "HTTP {}",
            status.as_u16()
        )));
    }

    let body = response
        .bytes()
        .await
        .map_err(|_| LocalUsageError::Request("could not read response".to_string()))?;
    let raw: Value = serde_json::from_slice(&body)
        .map_err(|_| LocalUsageError::Request("invalid JSON response".to_string()))?;

    Ok(normalize_usage_response(&raw, Utc::now().timestamp()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    fn request(status: u16, body: &str) -> Result<ProviderUsage, LocalUsageError> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/usage", listener.local_addr().unwrap());
        let body = body.to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let count = stream.read(&mut chunk).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&chunk[..count]);
            }
            let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
            assert!(request.starts_with("get /usage http/1.1\r\n"));
            assert!(request.contains("authorization: bearer test-secret\r\n"));
            assert!(request.contains("accept: application/json\r\n"));
            assert!(request.contains("user-agent: quota\r\n"));
            write!(
                stream,
                "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                body.len(),
                body
            )
            .unwrap();
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(fetch_usage_at(&url, "test-secret"));
        server.join().unwrap();
        result
    }

    #[test]
    fn fetches_with_bearer_auth_and_normalizes_the_response() {
        let usage = request(
            200,
            r#"{"usage":{"rolling":{"status":"ok","percent":35,
                    "resetsAt":"2026-11-20T19:15:17.000Z"},
                "weekly":{"status":"ok","percent":50,
                    "resetsAt":"2026-11-23T00:00:00.000Z"}}}"#,
        )
        .unwrap();
        assert_eq!(usage.compact_token().as_deref(), Some("5h 65% · Wk 50%"));
    }

    #[test]
    fn a_rejected_token_asks_for_a_new_sign_in_without_exposing_the_response() {
        let error = request(401, "test-secret response-secret").unwrap_err();
        assert!(matches!(error, LocalUsageError::Unauthorized));
        assert!(error.to_string().contains("npx @opencode/console login"));
        assert!(!format!("{} {:?}", error, error).contains("secret"));
    }

    #[test]
    fn an_account_without_a_go_subscription_is_said_so() {
        // The console answers 403 for a token with no Go subscription. Sending
        // that user to sign in again would fix nothing.
        let error = request(403, "test-secret response-secret").unwrap_err();
        assert!(matches!(error, LocalUsageError::NoSubscription));
        assert!(error.to_string().contains("no OpenCode Go subscription"));
        assert!(!format!("{} {:?}", error, error).contains("secret"));
    }

    #[test]
    fn reports_http_and_json_errors_without_raw_bodies() {
        for (status, body, expected) in [
            (500, "response-secret", "HTTP 500"),
            (302, "response-secret", "HTTP 302"),
            (
                200,
                r#"{"test-secret":"response-secret"#,
                "invalid JSON response",
            ),
        ] {
            let error = request(status, body).unwrap_err();
            let rendered = format!("{} {:?}", error, error);
            assert!(rendered.contains(expected));
            assert!(!rendered.contains("secret"));
        }
    }
}
