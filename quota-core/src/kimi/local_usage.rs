//! Normalize the current Kimi Code `/usages` response. Contract reference:
//! MoonshotAI/kimi-code, packages/oauth/src/managed-usage.ts.

use crate::usage::{ProviderUsage, UsageWindow};
use serde_json::Value;
use std::time::Duration;

use super::local::{read_local_credentials, LocalCredentialError};

const DEFAULT_USAGE_BASE_URL: &str = "https://api.kimi.com/coding/v1";

pub fn normalize_usage_response(raw: &Value) -> ProviderUsage {
    let windows: Vec<UsageWindow> = [
        ("limit_5h", "5h"),
        ("limit_7d", "Wk"),
        ("limit_month_total", "Mo"),
        ("limit_month_code", "Code"),
    ]
    .into_iter()
    .filter_map(|(key, label)| {
        let entry = raw.get("usages")?.get(key)?;
        let value = entry.get("used_ratio")?;
        let used = value
            .as_f64()
            .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
            .filter(|value| value.is_finite())?;
        let reset_at = entry
            .get("reset_time")
            .and_then(Value::as_str)
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.timestamp());
        Some(UsageWindow {
            label: label.to_string(),
            short_label: None,
            remaining_percent: Some(((1.0 - used) * 100.0).round().clamp(0.0, 100.0) as i32),
            reset_at,
        })
    })
    .collect();
    let note = windows.is_empty().then(|| "No usage data".to_string());

    ProviderUsage {
        provider: "kimi".to_string(),
        account_label: None,
        windows,
        note,
    }
}

#[derive(Debug)]
pub enum LocalUsageError {
    Credentials(LocalCredentialError),
    Unauthorized,
    Request(String),
}

/// Fixed detail for a rejected `KIMI_CODE_BASE_URL`. Carries nothing from the
/// configured value, so a mistyped URL with an embedded token stays out of
/// logs. No trailing period: `Request`'s Display only prefixes this detail.
const INVALID_BASE_URL_DETAIL: &str = "KIMI_CODE_BASE_URL is not an official Kimi endpoint. Only \
https://api.kimi.com/coding/v1 and https://api.kimi.ai/coding/v1 are \
allowed; fix or unset the variable";

fn invalid_base_url() -> LocalUsageError {
    LocalUsageError::Request(INVALID_BASE_URL_DETAIL.to_string())
}

impl std::fmt::Display for LocalUsageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credentials(inner) => write!(formatter, "{}", inner),
            Self::Unauthorized => write!(
                formatter,
                "Kimi Code rejected the stored token. Run Kimi Code to refresh it, or use /login."
            ),
            Self::Request(detail) => {
                write!(formatter, "Kimi Code usage request failed: {}", detail)
            }
        }
    }
}

/// Resolve the `/usages` URL, honoring `KIMI_CODE_BASE_URL`. Unset or blank
/// values fall back to the default. The override exists only to follow Kimi
/// Code between its two official endpoints; any other value is rejected
/// before a request exists, so the OAuth token can never leave Moonshot
/// infrastructure. The returned URL is rebuilt from the parsed `Url`, so it
/// is always canonical: lowercase host, no default port.
fn resolve_usage_base(raw: Option<&str>) -> Result<String, LocalUsageError> {
    let raw = match raw {
        Some(value) if !value.trim().is_empty() => value.trim(),
        _ => DEFAULT_USAGE_BASE_URL,
    };
    let mut url = reqwest::Url::parse(raw.trim_end_matches('/')).map_err(|_| invalid_base_url())?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if url.scheme() != "https"
        || !matches!(host.as_str(), "api.kimi.com" | "api.kimi.ai")
        || url.port_or_known_default() != Some(443)
        || url.path() != "/coding/v1"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_base_url());
    }
    url.set_path("/coding/v1/usages");
    Ok(url.into())
}

fn usage_url_from_env() -> Result<String, LocalUsageError> {
    match std::env::var("KIMI_CODE_BASE_URL") {
        Ok(raw) => resolve_usage_base(Some(&raw)),
        Err(std::env::VarError::NotPresent) => resolve_usage_base(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(invalid_base_url()),
    }
}

pub async fn fetch_local_usage() -> Result<ProviderUsage, LocalUsageError> {
    let url = usage_url_from_env()?;
    let credentials = read_local_credentials().map_err(LocalUsageError::Credentials)?;
    fetch_usage_at(&url, &credentials.access_token).await
}

// Test-only seam: unit tests inject a local server URL directly, bypassing
// `usage_url_from_env`, which only ever yields the two official endpoints.
async fn fetch_usage_at(url: &str, access_token: &str) -> Result<ProviderUsage, LocalUsageError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| LocalUsageError::Request("could not create HTTP client".to_string()))?;
    let response = client
        .get(url)
        .bearer_auth(access_token)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|err| {
            LocalUsageError::Request(
                if err.is_timeout() {
                    "request timed out"
                } else {
                    "could not send request"
                }
                .to_string(),
            )
        })?;

    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err(LocalUsageError::Unauthorized);
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
    Ok(normalize_usage_response(&raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn request(status: u16, body: &str) -> Result<ProviderUsage, LocalUsageError> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/usages", listener.local_addr().unwrap());
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
            assert!(request.starts_with("get /usages http/1.1\r\n"));
            assert!(request.contains("authorization: bearer test-secret\r\n"));
            assert!(request.contains("accept: application/json\r\n"));
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
        let usage = request(200, r#"{"usages":{"limit_5h":{"used_ratio":0.18}}}"#).unwrap();
        assert_eq!(usage.compact_token().as_deref(), Some("5h 82%"));
    }

    #[test]
    fn rejected_tokens_ask_the_cli_to_renew_without_exposing_the_response() {
        for status in [401, 403] {
            let error = request(status, "test-secret response-secret").unwrap_err();
            assert!(matches!(error, LocalUsageError::Unauthorized));
            assert!(error.to_string().contains("/login"));
            assert!(!format!("{} {:?}", error, error).contains("secret"));
        }
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

    #[test]
    fn usage_url_resolver_accepts_official_endpoints_only() {
        for (raw, expected) in [
            (None, "https://api.kimi.com/coding/v1/usages"),
            (Some(""), "https://api.kimi.com/coding/v1/usages"),
            (Some("   \t\n "), "https://api.kimi.com/coding/v1/usages"),
            (
                Some("https://api.kimi.com/coding/v1"),
                "https://api.kimi.com/coding/v1/usages",
            ),
            (
                Some("https://api.kimi.ai/coding/v1"),
                "https://api.kimi.ai/coding/v1/usages",
            ),
            (
                Some("https://api.kimi.com/coding/v1/"),
                "https://api.kimi.com/coding/v1/usages",
            ),
            (
                Some("https://api.kimi.com/coding/v1///"),
                "https://api.kimi.com/coding/v1/usages",
            ),
            (
                Some("  https://api.kimi.ai/coding/v1/  "),
                "https://api.kimi.ai/coding/v1/usages",
            ),
            // Output is canonical: mixed-case hosts normalize to lowercase
            // and an explicit default port is dropped.
            (
                Some("https://API.KIMI.COM/coding/v1"),
                "https://api.kimi.com/coding/v1/usages",
            ),
            (
                Some("https://api.kimi.ai:443/coding/v1"),
                "https://api.kimi.ai/coding/v1/usages",
            ),
        ] {
            assert_eq!(resolve_usage_base(raw).unwrap(), expected, "input: {raw:?}");
        }
    }

    #[test]
    fn usage_url_resolver_rejects_everything_unofficial_without_echoing_it() {
        for raw in [
            "http://api.kimi.com/coding/v1",
            "https://kimi.com/coding/v1",
            "https://api.kimi.com.evil.example/coding/v1",
            "https://sub.api.kimi.com/coding/v1",
            "https://api.kimi.io/coding/v1",
            "https://api.kimi.com:444/coding/v1",
            "https://api.kimi.com:8443/coding/v1",
            "https://api.kimi.com/",
            "https://api.kimi.com/coding",
            "https://api.kimi.com/coding/v1/extra",
            "https://user@api.kimi.com/coding/v1",
            "https://user:password@api.kimi.com/coding/v1",
            "https://api.kimi.com/coding/v1?debug=1",
            "https://api.kimi.com/coding/v1#section",
            "https://api.kimi.com/coding%2Fv1",
            "api.kimi.com/coding/v1",
            "not a url at all",
            "ftp://api.kimi.com/coding/v1",
        ] {
            let error = resolve_usage_base(Some(raw)).unwrap_err();
            assert!(
                matches!(&error, LocalUsageError::Request(detail) if detail == INVALID_BASE_URL_DETAIL),
                "input: {raw:?}"
            );
            // Sanitization contract: the error is fixed text wrapped in
            // `Request`, so no configured value (or anything it embeds) can
            // appear, even when the value shares a prefix with an allowed
            // endpoint.
            assert_eq!(
                error.to_string(),
                format!("Kimi Code usage request failed: {INVALID_BASE_URL_DETAIL}"),
                "input: {raw:?}"
            );
            assert_eq!(
                format!("{error:?}"),
                format!("Request({INVALID_BASE_URL_DETAIL:?})"),
                "input: {raw:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn usage_url_env_override_is_read_and_sanitized_end_to_end() {
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap();
        use std::os::unix::ffi::OsStrExt;

        // Restore whatever the developer's shell had, even on panic.
        struct EnvRestore {
            original: Option<std::ffi::OsString>,
        }
        impl Drop for EnvRestore {
            fn drop(&mut self) {
                match &self.original {
                    Some(value) => std::env::set_var("KIMI_CODE_BASE_URL", value),
                    None => std::env::remove_var("KIMI_CODE_BASE_URL"),
                }
            }
        }
        let _restore = EnvRestore {
            original: std::env::var_os("KIMI_CODE_BASE_URL"),
        };

        std::env::remove_var("KIMI_CODE_BASE_URL");
        assert_eq!(
            usage_url_from_env().unwrap(),
            format!("{DEFAULT_USAGE_BASE_URL}/usages")
        );

        std::env::set_var("KIMI_CODE_BASE_URL", " https://API.KIMI.AI:443/coding/v1/ ");
        assert_eq!(
            usage_url_from_env().unwrap(),
            "https://api.kimi.ai/coding/v1/usages"
        );

        for raw in [
            "http://api.kimi.com/coding/v1",
            "https://token@evil.example/",
        ] {
            std::env::set_var("KIMI_CODE_BASE_URL", raw);
            let error = usage_url_from_env().unwrap_err();
            assert!(
                matches!(&error, LocalUsageError::Request(detail) if detail == INVALID_BASE_URL_DETAIL)
            );
            let rendered = format!("{} {:?}", error, error);
            assert!(rendered.contains("KIMI_CODE_BASE_URL"));
            assert!(!rendered.contains(raw), "echoes value: {raw:?}");
        }

        std::env::set_var(
            "KIMI_CODE_BASE_URL",
            std::ffi::OsStr::from_bytes(b"https://api.kimi.com/coding/v1/\xFF"),
        );
        let error = usage_url_from_env().unwrap_err();
        assert!(matches!(error, LocalUsageError::Request(_)));
        assert!(!format!("{} {:?}", error, error).contains('\u{FFFD}'));
    }
}
