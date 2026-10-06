use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DATA_DIR: &str = ".quota";
const ACCOUNTS_DIR: &str = "opencode_go_accounts";
const ACCOUNTS_INDEX_FILE: &str = "opencode_go_accounts.json";
// Undocumented, but it is what the OpenCode console reads, and it accepts the
// same Go API key users paste into OpenCode's `/connect`.
const OPENCODE_GO_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
const REQUEST_TIMEOUT_SECONDS: u64 = 15;
const ROLLING_WINDOW_SECONDS: i64 = 5 * 60 * 60;
// Allows for the request round trip and small clock differences.
const ROLLING_PLACEHOLDER_TOLERANCE_SECONDS: i64 = 120;

#[derive(Clone, Serialize, Deserialize)]
struct StoredOpenCodeGoAccount {
    id: String,
    label: String,
    api_key: String,
    usage: OpenCodeGoUsage,
    quota_query_last_error: Option<String>,
    quota_query_last_error_at: Option<i64>,
    usage_updated_at: Option<i64>,
    created_at: i64,
    last_used: i64,
}

// Hand-written so the API key can never reach a log line.
impl std::fmt::Debug for StoredOpenCodeGoAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredOpenCodeGoAccount")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("api_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeGoWindow {
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    /// Unix seconds, matching the other providers' reset fields.
    pub reset_at: Option<i64>,
    /// The rolling window hasn't been opened yet. The console shows "Starts on
    /// first use" here, and `reset_at` is cleared because the API only sends a
    /// placeholder of now plus five hours.
    #[serde(default)]
    pub starts_on_first_use: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeGoUsage {
    pub five_hour: OpenCodeGoWindow,
    pub weekly: OpenCodeGoWindow,
    pub monthly: OpenCodeGoWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeGoAccountSummary {
    pub id: String,
    pub label: String,
    pub usage: OpenCodeGoUsage,
    pub quota_query_last_error: Option<String>,
    pub quota_query_last_error_at: Option<i64>,
    pub usage_updated_at: Option<i64>,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenCodeGoAccountIndex {
    version: String,
    account_ids: Vec<String>,
}

impl OpenCodeGoAccountIndex {
    fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            account_ids: Vec::new(),
        }
    }
}

impl StoredOpenCodeGoAccount {
    fn to_summary(&self) -> OpenCodeGoAccountSummary {
        OpenCodeGoAccountSummary {
            id: self.id.clone(),
            label: self.label.clone(),
            usage: self.usage.clone(),
            quota_query_last_error: self.quota_query_last_error.clone(),
            quota_query_last_error_at: self.quota_query_last_error_at,
            usage_updated_at: self.usage_updated_at,
            created_at: self.created_at,
            last_used: self.last_used,
        }
    }
}

// ── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_opencode_go_accounts() -> Result<Vec<OpenCodeGoAccountSummary>, String> {
    list_accounts_in(&quota_storage_dir()?)
}

#[tauri::command]
pub async fn add_opencode_go_account(
    api_key: String,
    name: Option<String>,
) -> Result<OpenCodeGoAccountSummary, String> {
    let api_key = api_key.trim().to_string();
    if api_key.is_empty() {
        return Err("Enter an OpenCode Go API key.".to_string());
    }

    // Checked before anything is saved, so a mistyped key never becomes an account.
    let usage = fetch_usage(&api_key).await?;
    let storage_dir = quota_storage_dir()?;
    let id = account_id_for_key(&api_key);
    let existing = load_account_in(&storage_dir, &id).ok();
    let now = now_timestamp();
    let label = name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| mask_api_key(&api_key));

    upsert_account_in(
        &storage_dir,
        StoredOpenCodeGoAccount {
            id,
            label,
            api_key,
            usage,
            quota_query_last_error: None,
            quota_query_last_error_at: None,
            usage_updated_at: Some(now_timestamp_ms()),
            created_at: existing.map(|account| account.created_at).unwrap_or(now),
            last_used: now,
        },
    )
}

#[tauri::command]
pub async fn refresh_opencode_go_account(
    account_id: String,
) -> Result<OpenCodeGoAccountSummary, String> {
    refresh_account_in(&quota_storage_dir()?, &account_id).await
}

#[tauri::command]
pub async fn refresh_all_opencode_go_accounts() -> Result<Vec<OpenCodeGoAccountSummary>, String> {
    let storage_dir = quota_storage_dir()?;
    for account_id in load_index_in(&storage_dir)?.account_ids {
        let _ = refresh_account_in(&storage_dir, &account_id).await;
    }
    list_accounts_in(&storage_dir)
}

#[tauri::command]
pub fn delete_opencode_go_account(account_id: String) -> Result<(), String> {
    delete_account_in(&quota_storage_dir()?, &account_id)
}

// ── Usage API ───────────────────────────────────────────────────────────────

async fn fetch_usage(api_key: &str) -> Result<OpenCodeGoUsage, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        // Never forward the key to a redirect target.
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("quota-desktop/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("Could not create HTTP client: {}", e))?;

    let response = client
        .get(OPENCODE_GO_USAGE_URL)
        .bearer_auth(api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|_| {
            "Could not reach OpenCode Go. Check your connection and try again.".to_string()
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(usage_error_message(status.as_u16()));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|_| "OpenCode Go usage response was not valid JSON.".to_string())?;
    parse_usage(&body, now_timestamp())
}

/// A user-readable message for a failed usage request. Never includes the body.
fn usage_error_message(status: u16) -> String {
    match status {
        401 => {
            "OpenCode Go rejected this API key. Check the key in the OpenCode console.".to_string()
        }
        403 => "This API key has no OpenCode Go subscription.".to_string(),
        429 => "OpenCode Go is rate limiting usage requests. Try again in a minute.".to_string(),
        _ => format!("OpenCode Go usage returned {}.", status),
    }
}

fn parse_window(raw: Option<&Value>) -> Option<OpenCodeGoWindow> {
    let raw = raw?.as_object()?;
    let used = raw
        .get("percent")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 100.0));
    let reset_at = raw
        .get("resetsAt")
        .and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp());

    Some(OpenCodeGoWindow {
        used_percent: used,
        remaining_percent: used.map(|value| 100.0 - value),
        reset_at,
        starts_on_first_use: false,
    })
}

/// An unused rolling window comes back as 0% with a reset a full five hours
/// out. A window that has really started resets sooner than that.
fn mark_unstarted_rolling(window: &mut OpenCodeGoWindow, now: i64) {
    let unused = window.used_percent.is_none_or(|value| value == 0.0);
    let full_window_ahead = window.reset_at.is_some_and(|reset| {
        reset - now >= ROLLING_WINDOW_SECONDS - ROLLING_PLACEHOLDER_TOLERANCE_SECONDS
    });
    if unused && full_window_ahead {
        window.reset_at = None;
        window.starts_on_first_use = true;
    }
}

/// Parse `GET /zen/go/v1/usage`. Every field is optional because the endpoint
/// is undocumented, but a body with none of the three windows is an error
/// rather than a silent 0% used.
fn parse_usage(body: &Value, now: i64) -> Result<OpenCodeGoUsage, String> {
    let missing = || "OpenCode Go usage response did not include usage windows.".to_string();
    let usage = body
        .get("usage")
        .filter(|value| value.is_object())
        .ok_or_else(missing)?;

    let mut five_hour = parse_window(usage.get("rolling"));
    if let Some(window) = five_hour.as_mut() {
        mark_unstarted_rolling(window, now);
    }
    let weekly = parse_window(usage.get("weekly"));
    let monthly = parse_window(usage.get("monthly"));
    if five_hour.is_none() && weekly.is_none() && monthly.is_none() {
        return Err(missing());
    }

    Ok(OpenCodeGoUsage {
        five_hour: five_hour.unwrap_or_default(),
        weekly: weekly.unwrap_or_default(),
        monthly: monthly.unwrap_or_default(),
    })
}

fn account_id_for_key(api_key: &str) -> String {
    let digest = Sha256::digest(api_key.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

/// Show only the last four characters, for labels.
fn mask_api_key(api_key: &str) -> String {
    let chars: Vec<char> = api_key.trim().chars().collect();
    let tail: String = chars[chars.len().saturating_sub(4)..].iter().collect();
    format!("Go key ••••{}", tail)
}

// ── Storage helpers ─────────────────────────────────────────────────────────

fn now_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

fn now_timestamp_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn quota_storage_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not locate home directory".to_string())?;
    let dir = home.join(DATA_DIR);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create Quota data directory: {}", e))?;
    Ok(dir)
}

fn index_path_in(storage_dir: &Path) -> PathBuf {
    storage_dir.join(ACCOUNTS_INDEX_FILE)
}

fn account_path_in(storage_dir: &Path, account_id: &str) -> PathBuf {
    storage_dir
        .join(ACCOUNTS_DIR)
        .join(format!("{}.json", account_id))
}

/// Write through a temp file, readable only by the owner on Unix, since
/// account files hold the API key.
fn write_private_atomic(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "No parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("Could not create directory: {}", e))?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("opencode_go"),
        std::process::id()
    ));
    fs::write(&tmp, content).map_err(|e| format!("Could not write temp file: {}", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Could not set file permissions: {}", e))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("Could not replace file: {}", e))?;
    Ok(())
}

fn load_index_in(storage_dir: &Path) -> Result<OpenCodeGoAccountIndex, String> {
    let path = index_path_in(storage_dir);
    if !path.exists() {
        return Ok(OpenCodeGoAccountIndex::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read OpenCode Go account index: {}", e))?;
    if content.trim().is_empty() {
        return Ok(OpenCodeGoAccountIndex::new());
    }
    serde_json::from_str(&content)
        .map_err(|e| format!("Could not parse OpenCode Go account index: {}", e))
}

fn save_index_in(storage_dir: &Path, index: &OpenCodeGoAccountIndex) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index)
        .map_err(|e| format!("Could not encode OpenCode Go account index: {}", e))?;
    write_private_atomic(&index_path_in(storage_dir), &content)
}

fn load_account_in(
    storage_dir: &Path,
    account_id: &str,
) -> Result<StoredOpenCodeGoAccount, String> {
    let content = fs::read_to_string(account_path_in(storage_dir, account_id))
        .map_err(|e| format!("Could not read OpenCode Go account: {}", e))?;
    // serde's message can quote the input, which holds the key, so report position only.
    serde_json::from_str(&content).map_err(|e| {
        format!(
            "Could not parse OpenCode Go account (line {}, column {}).",
            e.line(),
            e.column()
        )
    })
}

fn save_account_in(storage_dir: &Path, account: &StoredOpenCodeGoAccount) -> Result<(), String> {
    let content = serde_json::to_string_pretty(account)
        .map_err(|e| format!("Could not encode OpenCode Go account: {}", e))?;
    write_private_atomic(&account_path_in(storage_dir, &account.id), &content)
}

fn list_accounts_in(storage_dir: &Path) -> Result<Vec<OpenCodeGoAccountSummary>, String> {
    let index = load_index_in(storage_dir)?;
    Ok(index
        .account_ids
        .iter()
        .filter_map(|id| load_account_in(storage_dir, id).ok())
        .map(|account| account.to_summary())
        .collect())
}

fn upsert_account_in(
    storage_dir: &Path,
    account: StoredOpenCodeGoAccount,
) -> Result<OpenCodeGoAccountSummary, String> {
    let mut index = load_index_in(storage_dir)?;
    if !index.account_ids.iter().any(|id| id == &account.id) {
        index.account_ids.insert(0, account.id.clone());
    }
    save_account_in(storage_dir, &account)?;
    save_index_in(storage_dir, &index)?;
    Ok(account.to_summary())
}

fn delete_account_in(storage_dir: &Path, account_id: &str) -> Result<(), String> {
    let mut index = load_index_in(storage_dir)?;
    index.account_ids.retain(|id| id != account_id);
    save_index_in(storage_dir, &index)?;
    let path = account_path_in(storage_dir, account_id);
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Could not delete OpenCode Go account: {}", e))?;
    }
    Ok(())
}

async fn refresh_account_in(
    storage_dir: &Path,
    account_id: &str,
) -> Result<OpenCodeGoAccountSummary, String> {
    let mut account = load_account_in(storage_dir, account_id)?;
    match fetch_usage(&account.api_key).await {
        Ok(usage) => {
            account.usage = usage;
            account.quota_query_last_error = None;
            account.quota_query_last_error_at = None;
            account.usage_updated_at = Some(now_timestamp_ms());
            account.last_used = now_timestamp();
        }
        Err(message) => {
            account.quota_query_last_error = Some(message);
            account.quota_query_last_error_at = Some(now_timestamp_ms());
        }
    }
    upsert_account_in(storage_dir, account)
}

#[cfg(test)]
#[path = "opencode_go_tests.rs"]
mod tests;
