use base64::Engine;
use rand::Rng;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const DATA_DIR: &str = ".quota";
const ACCOUNTS_DIR: &str = "github_copilot_accounts";
const ACCOUNTS_INDEX_FILE: &str = "github_copilot_accounts.json";
const GITHUB_DEVICE_CODE_ENDPOINT: &str = "https://github.com/login/device/code";
const GITHUB_DEVICE_TOKEN_ENDPOINT: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_ENDPOINT: &str = "https://api.github.com/user";
const GITHUB_USER_EMAILS_ENDPOINT: &str = "https://api.github.com/user/emails";
const GITHUB_COPILOT_TOKEN_ENDPOINT: &str = "https://api.github.com/copilot_internal/v2/token";
const GITHUB_COPILOT_USER_INFO_ENDPOINT: &str = "https://api.github.com/copilot_internal/user";
const GITHUB_OAUTH_CLIENT_ID: &str = "01ab8ac9400c4e429b23";
const GITHUB_OAUTH_SCOPE: &str = "read:user user:email repo workflow";
const APP_USER_AGENT: &str = "quota";

#[derive(Debug, Clone)]
struct PendingDeviceLogin {
    login_id: String,
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    interval_seconds: u64,
    expires_at: i64,
}

static PENDING_DEVICE_LOGIN: std::sync::LazyLock<Arc<Mutex<Option<PendingDeviceLogin>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(None)));

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    access_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    id: u64,
    login: String,
    name: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubEmail {
    email: String,
    primary: Option<bool>,
    verified: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CopilotTokenResponse {
    token: Option<String>,
    expires_at: Option<i64>,
    refresh_in: Option<i64>,
    sku: Option<String>,
    chat_enabled: Option<bool>,
    limited_user_quotas: Option<serde_json::Value>,
    limited_user_reset_date: Option<i64>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CopilotUserInfoResponse {
    copilot_plan: Option<String>,
    quota_snapshots: Option<serde_json::Value>,
    quota_reset_date: Option<String>,
}

#[derive(Debug, Clone)]
struct CopilotTokenBundle {
    token: String,
    plan: Option<String>,
    chat_enabled: Option<bool>,
    expires_at: Option<i64>,
    refresh_in: Option<i64>,
    quota_snapshots: Option<serde_json::Value>,
    quota_reset_date: Option<String>,
    limited_user_quotas: Option<serde_json::Value>,
    limited_user_reset_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredGitHubCopilotAccount {
    id: String,
    github_login: String,
    github_id: u64,
    github_name: Option<String>,
    github_email: Option<String>,
    github_access_token: String,
    github_token_type: Option<String>,
    github_scope: Option<String>,
    copilot_token: String,
    copilot_plan: Option<String>,
    copilot_chat_enabled: Option<bool>,
    copilot_expires_at: Option<i64>,
    copilot_refresh_in: Option<i64>,
    copilot_quota_snapshots: Option<serde_json::Value>,
    copilot_quota_reset_date: Option<String>,
    copilot_limited_user_quotas: Option<serde_json::Value>,
    copilot_limited_user_reset_date: Option<i64>,
    quota_query_last_error: Option<String>,
    quota_query_last_error_at: Option<i64>,
    usage_updated_at: Option<i64>,
    created_at: i64,
    last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitHubCopilotAccountIndex {
    version: String,
    account_ids: Vec<String>,
}

impl GitHubCopilotAccountIndex {
    fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            account_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCopilotAccountSummary {
    id: String,
    github_login: String,
    github_name: Option<String>,
    github_email: Option<String>,
    plan: Option<String>,
    chat_enabled: Option<bool>,
    usage: GitHubCopilotUsageSummary,
    usage_updated_at: Option<i64>,
    quota_query_last_error: Option<String>,
    quota_query_last_error_at: Option<i64>,
    created_at: i64,
    last_used: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCopilotUsageSummary {
    inline_suggestions_used_percent: Option<i64>,
    chat_messages_used_percent: Option<i64>,
    premium_requests_used_percent: Option<i64>,
    inline_included: bool,
    chat_included: bool,
    premium_included: bool,
    remaining_completions: Option<i64>,
    remaining_chat: Option<i64>,
    remaining_premium_requests: Option<i64>,
    total_completions: Option<i64>,
    total_chat: Option<i64>,
    total_premium_requests: Option<i64>,
    used_premium_requests: Option<i64>,
    allowance_reset_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCopilotOAuthStartResponse {
    login_id: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval_seconds: u64,
}

#[derive(Debug, Clone)]
struct GitHubCopilotOAuthCompletePayload {
    github_login: String,
    github_id: u64,
    github_name: Option<String>,
    github_email: Option<String>,
    github_access_token: String,
    github_token_type: Option<String>,
    github_scope: Option<String>,
    copilot_token: String,
    copilot_plan: Option<String>,
    copilot_chat_enabled: Option<bool>,
    copilot_expires_at: Option<i64>,
    copilot_refresh_in: Option<i64>,
    copilot_quota_snapshots: Option<serde_json::Value>,
    copilot_quota_reset_date: Option<String>,
    copilot_limited_user_quotas: Option<serde_json::Value>,
    copilot_limited_user_reset_date: Option<i64>,
}

fn now_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

fn generate_login_id() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..24).map(|_| rng.gen::<u8>()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn data_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not locate home directory".to_string())?;
    let dir = home.join(DATA_DIR);
    fs::create_dir_all(&dir).map_err(|err| format!("Could not create data directory: {}", err))?;
    Ok(dir)
}

fn accounts_dir() -> Result<PathBuf, String> {
    let dir = data_dir()?.join(ACCOUNTS_DIR);
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Could not create GitHub Copilot account directory: {}", err))?;
    Ok(dir)
}

fn accounts_index_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join(ACCOUNTS_INDEX_FILE))
}

fn account_path(account_id: &str) -> Result<PathBuf, String> {
    Ok(accounts_dir()?.join(format!("{}.json", account_id)))
}

fn write_string_atomic(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Could not locate parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("Could not create parent directory: {}", err))?;
    let temp_path = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|item| item.to_str())
            .unwrap_or("quota"),
        std::process::id()
    ));
    fs::write(&temp_path, content).map_err(|err| format!("Could not write temp file: {}", err))?;
    fs::rename(&temp_path, path).map_err(|err| format!("Could not replace file: {}", err))?;
    Ok(())
}

fn load_index() -> Result<GitHubCopilotAccountIndex, String> {
    let path = accounts_index_path()?;
    if !path.exists() {
        return Ok(GitHubCopilotAccountIndex::new());
    }
    let content =
        fs::read_to_string(&path).map_err(|err| format!("Could not read index: {}", err))?;
    if content.trim().is_empty() {
        return Ok(GitHubCopilotAccountIndex::new());
    }
    serde_json::from_str(&content).map_err(|err| format!("Could not parse account index: {}", err))
}

fn save_index(index: &GitHubCopilotAccountIndex) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index)
        .map_err(|err| format!("Could not encode index: {}", err))?;
    write_string_atomic(&accounts_index_path()?, &content)
}

fn load_account(account_id: &str) -> Result<StoredGitHubCopilotAccount, String> {
    let path = account_path(account_id)?;
    let content =
        fs::read_to_string(&path).map_err(|err| format!("Could not read account file: {}", err))?;
    serde_json::from_str(&content).map_err(|err| format!("Could not parse account file: {}", err))
}

fn save_account(account: &StoredGitHubCopilotAccount) -> Result<(), String> {
    let content = serde_json::to_string_pretty(account)
        .map_err(|err| format!("Could not encode account: {}", err))?;
    write_string_atomic(&account_path(&account.id)?, &content)
}

fn upsert_account(
    payload: GitHubCopilotOAuthCompletePayload,
) -> Result<StoredGitHubCopilotAccount, String> {
    let now = now_timestamp();
    let mut index = load_index()?;
    let generated_id = format!(
        "ghcp_{:x}",
        md5::compute(format!("{}:{}", payload.github_login, payload.github_id))
    );
    let account_id = index
        .account_ids
        .iter()
        .filter_map(|id| load_account(id).ok())
        .find(|account| account.github_id == payload.github_id)
        .map(|account| account.id)
        .unwrap_or(generated_id);
    let existing = load_account(&account_id).ok();
    let created_at = existing
        .as_ref()
        .map(|account| account.created_at)
        .unwrap_or(now);

    let account = StoredGitHubCopilotAccount {
        id: account_id,
        github_login: payload.github_login,
        github_id: payload.github_id,
        github_name: payload.github_name,
        github_email: payload.github_email,
        github_access_token: payload.github_access_token,
        github_token_type: payload.github_token_type,
        github_scope: payload.github_scope,
        copilot_token: payload.copilot_token,
        copilot_plan: payload.copilot_plan,
        copilot_chat_enabled: payload.copilot_chat_enabled,
        copilot_expires_at: payload.copilot_expires_at,
        copilot_refresh_in: payload.copilot_refresh_in,
        copilot_quota_snapshots: payload.copilot_quota_snapshots,
        copilot_quota_reset_date: payload.copilot_quota_reset_date,
        copilot_limited_user_quotas: payload.copilot_limited_user_quotas,
        copilot_limited_user_reset_date: payload.copilot_limited_user_reset_date,
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        usage_updated_at: Some(now),
        created_at,
        last_used: now,
    };

    save_account(&account)?;
    if !index.account_ids.iter().any(|id| id == &account.id) {
        index.account_ids.push(account.id.clone());
    }
    save_index(&index)?;
    Ok(account)
}

fn account_summary(account: StoredGitHubCopilotAccount) -> GitHubCopilotAccountSummary {
    GitHubCopilotAccountSummary {
        id: account.id.clone(),
        github_login: account.github_login.clone(),
        github_name: account.github_name.clone(),
        github_email: account.github_email.clone(),
        plan: account.copilot_plan.clone(),
        chat_enabled: account.copilot_chat_enabled,
        usage: usage_summary(&account),
        usage_updated_at: account.usage_updated_at,
        quota_query_last_error: account.quota_query_last_error.clone(),
        quota_query_last_error_at: account.quota_query_last_error_at,
        created_at: account.created_at,
        last_used: account.last_used,
    }
}

fn get_number(value: Option<&serde_json::Value>) -> Option<i64> {
    match value {
        Some(serde_json::Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|n| n.round() as i64)),
        Some(serde_json::Value::String(text)) => text.parse::<f64>().ok().map(|n| n.round() as i64),
        _ => None,
    }
}

fn token_value<'a>(token: &'a str, key: &str) -> Option<&'a str> {
    let prefix = token.split(':').next().unwrap_or(token);
    prefix.split(';').find_map(|part| {
        let (part_key, part_value) = part.split_once('=')?;
        if part_key.trim() == key {
            Some(part_value.trim())
        } else {
            None
        }
    })
}

fn reset_from_token(token: &str) -> Option<i64> {
    let value = token_value(token, "rd")?;
    let head = value.split(':').next().unwrap_or(value).trim();
    head.parse::<i64>().ok()
}

fn parse_reset_date(date: &str) -> Option<i64> {
    let trimmed = date.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Some(parsed.timestamp());
    }

    chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|datetime| datetime.and_utc().timestamp())
}

fn snapshot<'a>(
    account: &'a StoredGitHubCopilotAccount,
    key: &str,
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    account
        .copilot_quota_snapshots
        .as_ref()
        .and_then(|value| value.as_object())
        .and_then(|snapshots| snapshots.get(key))
        .and_then(|value| value.as_object())
}

fn limited_quota(account: &StoredGitHubCopilotAccount, key: &str) -> Option<i64> {
    account
        .copilot_limited_user_quotas
        .as_ref()
        .and_then(|value| value.as_object())
        .and_then(|quotas| get_number(quotas.get(key)))
}

fn percent_from_snapshot(snapshot: &serde_json::Map<String, serde_json::Value>) -> Option<i64> {
    if snapshot.get("unlimited").and_then(|value| value.as_bool()) == Some(true) {
        return Some(0);
    }
    let entitlement = get_number(snapshot.get("entitlement"));
    if entitlement.is_some_and(|value| value < 0) {
        return Some(0);
    }
    let percent_remaining = get_number(snapshot.get("percent_remaining"))?;
    Some((100 - percent_remaining).clamp(0, 100))
}

fn included_from_snapshot(snapshot: Option<&serde_json::Map<String, serde_json::Value>>) -> bool {
    let Some(snapshot) = snapshot else {
        return false;
    };
    if snapshot.get("unlimited").and_then(|value| value.as_bool()) == Some(true) {
        return true;
    }
    get_number(snapshot.get("entitlement")).is_some_and(|value| value < 0)
}

fn remaining_from_snapshot(snapshot: &serde_json::Map<String, serde_json::Value>) -> Option<i64> {
    if let Some(remaining) = get_number(snapshot.get("remaining")) {
        return Some(remaining);
    }
    let entitlement = get_number(snapshot.get("entitlement"))?;
    let percent_remaining = get_number(snapshot.get("percent_remaining"))?;
    if entitlement <= 0 {
        return None;
    }
    Some(((entitlement as f64 * percent_remaining as f64) / 100.0).round() as i64)
}

fn used_percent(total: Option<i64>, remaining: Option<i64>) -> Option<i64> {
    let total = total?;
    let remaining = remaining?;
    if total <= 0 {
        return None;
    }
    let used = (total - remaining).max(0);
    Some(
        ((used as f64 / total as f64) * 100.0)
            .round()
            .clamp(0.0, 100.0) as i64,
    )
}

fn usage_summary(account: &StoredGitHubCopilotAccount) -> GitHubCopilotUsageSummary {
    let completions_snapshot = snapshot(account, "completions");
    let chat_snapshot = snapshot(account, "chat");
    let premium_snapshot =
        snapshot(account, "premium_interactions").or_else(|| snapshot(account, "premium_models"));

    let remaining_completions = completions_snapshot
        .and_then(remaining_from_snapshot)
        .or_else(|| limited_quota(account, "completions"));
    let remaining_chat = chat_snapshot
        .and_then(remaining_from_snapshot)
        .or_else(|| limited_quota(account, "chat"));
    let remaining_premium = premium_snapshot.and_then(remaining_from_snapshot);

    let total_completions = completions_snapshot
        .and_then(|item| get_number(item.get("entitlement")))
        .or(remaining_completions);
    let total_chat = chat_snapshot
        .and_then(|item| get_number(item.get("entitlement")))
        .or(remaining_chat);
    let total_premium = premium_snapshot
        .and_then(|item| get_number(item.get("entitlement")))
        .or(remaining_premium);
    let exact_remaining_premium =
        premium_snapshot.and_then(|item| get_number(item.get("remaining")));

    let allowance_reset_at = account
        .copilot_limited_user_reset_date
        .or_else(|| {
            account
                .copilot_quota_reset_date
                .as_deref()
                .and_then(parse_reset_date)
        })
        .or_else(|| reset_from_token(&account.copilot_token));

    GitHubCopilotUsageSummary {
        inline_suggestions_used_percent: completions_snapshot
            .and_then(percent_from_snapshot)
            .or_else(|| used_percent(total_completions, remaining_completions)),
        chat_messages_used_percent: chat_snapshot
            .and_then(percent_from_snapshot)
            .or_else(|| used_percent(total_chat, remaining_chat)),
        premium_requests_used_percent: premium_snapshot
            .and_then(percent_from_snapshot)
            .or_else(|| used_percent(total_premium, remaining_premium)),
        inline_included: included_from_snapshot(completions_snapshot),
        chat_included: included_from_snapshot(chat_snapshot),
        premium_included: included_from_snapshot(premium_snapshot),
        remaining_completions,
        remaining_chat,
        remaining_premium_requests: exact_remaining_premium.or(remaining_premium),
        total_completions,
        total_chat,
        total_premium_requests: total_premium,
        used_premium_requests: total_premium
            .zip(exact_remaining_premium)
            .map(|(total, remaining)| (total - remaining).max(0)),
        allowance_reset_at,
    }
}

fn pending_login() -> Option<PendingDeviceLogin> {
    PENDING_DEVICE_LOGIN
        .lock()
        .ok()
        .and_then(|state| state.clone())
}

fn set_pending_login(login: Option<PendingDeviceLogin>) {
    if let Ok(mut state) = PENDING_DEVICE_LOGIN.lock() {
        *state = login;
    }
}

fn pending_login_for(login_id: &str) -> Result<PendingDeviceLogin, String> {
    let login =
        pending_login().ok_or_else(|| "Login flow was cancelled. Start again.".to_string())?;
    if login.login_id != login_id {
        return Err("Login session changed. Start again.".to_string());
    }
    Ok(login)
}

fn to_start_response(login: &PendingDeviceLogin) -> GitHubCopilotOAuthStartResponse {
    GitHubCopilotOAuthStartResponse {
        login_id: login.login_id.clone(),
        user_code: login.user_code.clone(),
        verification_uri: login.verification_uri.clone(),
        verification_uri_complete: login.verification_uri_complete.clone(),
        expires_in: (login.expires_at - now_timestamp()).max(0) as u64,
        interval_seconds: login.interval_seconds,
    }
}

async fn sleep_with_cancel_check(login_id: &str, total_secs: u64) -> Result<(), String> {
    let ticks = (total_secs.max(1) * 5) as usize;
    for _ in 0..ticks {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let _ = pending_login_for(login_id)?;
    }
    Ok(())
}

async fn request_device_code() -> Result<DeviceCodeResponse, String> {
    let client = reqwest::Client::new();
    let response = client
        .post(GITHUB_DEVICE_CODE_ENDPOINT)
        .header(USER_AGENT, APP_USER_AGENT)
        .header(ACCEPT, "application/json")
        .form(&[
            ("client_id", GITHUB_OAUTH_CLIENT_ID),
            ("scope", GITHUB_OAUTH_SCOPE),
        ])
        .send()
        .await
        .map_err(|err| format!("Could not request GitHub device code: {}", err))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub device code request failed: {}",
            response.status()
        ));
    }

    response
        .json::<DeviceCodeResponse>()
        .await
        .map_err(|err| format!("Could not parse GitHub device code response: {}", err))
}

async fn exchange_device_token(
    client: &reqwest::Client,
    device_code: &str,
) -> Result<DeviceTokenResponse, String> {
    let response = client
        .post(GITHUB_DEVICE_TOKEN_ENDPOINT)
        .header(USER_AGENT, APP_USER_AGENT)
        .header(ACCEPT, "application/json")
        .form(&[
            ("client_id", GITHUB_OAUTH_CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|err| format!("Could not request GitHub access token: {}", err))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub access token request failed: {}",
            response.status()
        ));
    }

    response
        .json::<DeviceTokenResponse>()
        .await
        .map_err(|err| format!("Could not parse GitHub access token response: {}", err))
}

async fn fetch_github_user(
    client: &reqwest::Client,
    github_access_token: &str,
) -> Result<GitHubUser, String> {
    let response = client
        .get(GITHUB_USER_ENDPOINT)
        .header(USER_AGENT, APP_USER_AGENT)
        .header(ACCEPT, "application/vnd.github+json")
        .header(AUTHORIZATION, format!("Bearer {}", github_access_token))
        .send()
        .await
        .map_err(|err| format!("Could not request GitHub user: {}", err))?;

    if !response.status().is_success() {
        return Err(format!("GitHub user request failed: {}", response.status()));
    }

    response
        .json::<GitHubUser>()
        .await
        .map_err(|err| format!("Could not parse GitHub user response: {}", err))
}

async fn fetch_github_email(
    client: &reqwest::Client,
    github_access_token: &str,
) -> Result<Option<String>, String> {
    let response = client
        .get(GITHUB_USER_EMAILS_ENDPOINT)
        .header(USER_AGENT, APP_USER_AGENT)
        .header(ACCEPT, "application/vnd.github+json")
        .header(AUTHORIZATION, format!("Bearer {}", github_access_token))
        .send()
        .await
        .map_err(|err| format!("Could not request GitHub emails: {}", err))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub email request failed: {}",
            response.status()
        ));
    }

    let emails = response
        .json::<Vec<GitHubEmail>>()
        .await
        .map_err(|err| format!("Could not parse GitHub emails response: {}", err))?;
    Ok(emails
        .iter()
        .find(|item| item.primary.unwrap_or(false) && item.verified.unwrap_or(false))
        .or_else(|| emails.iter().find(|item| item.verified.unwrap_or(false)))
        .map(|item| item.email.clone()))
}

async fn fetch_copilot_user_info(
    client: &reqwest::Client,
    github_access_token: &str,
) -> Result<CopilotUserInfoResponse, String> {
    let response = client
        .get(GITHUB_COPILOT_USER_INFO_ENDPOINT)
        .header(USER_AGENT, APP_USER_AGENT)
        .header(ACCEPT, "application/json")
        .header("X-GitHub-Api-Version", "2025-04-01")
        .header(AUTHORIZATION, format!("token {}", github_access_token))
        .send()
        .await
        .map_err(|err| format!("Could not request Copilot user info: {}", err))?;

    if !response.status().is_success() {
        return Err(format!(
            "Copilot user info request failed: {}",
            response.status()
        ));
    }

    response
        .json::<CopilotUserInfoResponse>()
        .await
        .map_err(|err| format!("Could not parse Copilot user info response: {}", err))
}

async fn fetch_copilot_token(
    client: &reqwest::Client,
    github_access_token: &str,
) -> Result<CopilotTokenBundle, String> {
    let response = client
        .get(GITHUB_COPILOT_TOKEN_ENDPOINT)
        .header(USER_AGENT, APP_USER_AGENT)
        .header(ACCEPT, "application/json")
        .header("X-GitHub-Api-Version", "2025-04-01")
        .header(AUTHORIZATION, format!("token {}", github_access_token))
        .send()
        .await
        .map_err(|err| format!("Could not request Copilot token: {}", err))?;

    if !response.status().is_success() {
        return Err(format!(
            "Copilot token request failed: {}",
            response.status()
        ));
    }

    let payload = response
        .json::<CopilotTokenResponse>()
        .await
        .map_err(|err| format!("Could not parse Copilot token response: {}", err))?;
    let token = payload.token.ok_or_else(|| {
        payload
            .message
            .unwrap_or_else(|| "Copilot token missing".to_string())
    })?;
    let user_info = fetch_copilot_user_info(client, github_access_token)
        .await
        .ok();

    Ok(CopilotTokenBundle {
        token,
        plan: user_info
            .as_ref()
            .and_then(|info| info.copilot_plan.clone())
            .or(payload.sku),
        chat_enabled: payload.chat_enabled,
        expires_at: payload.expires_at,
        refresh_in: payload.refresh_in,
        quota_snapshots: user_info
            .as_ref()
            .and_then(|info| info.quota_snapshots.clone()),
        quota_reset_date: user_info
            .as_ref()
            .and_then(|info| info.quota_reset_date.clone()),
        limited_user_quotas: payload.limited_user_quotas,
        limited_user_reset_date: payload.limited_user_reset_date,
    })
}

async fn build_payload_from_github_access_token(
    github_access_token: String,
    token_type: Option<String>,
    scope: Option<String>,
) -> Result<GitHubCopilotOAuthCompletePayload, String> {
    let client = reqwest::Client::new();
    let user = fetch_github_user(&client, &github_access_token).await?;
    let email = fetch_github_email(&client, &github_access_token)
        .await
        .ok()
        .flatten()
        .or(user.email);
    let copilot = fetch_copilot_token(&client, &github_access_token).await?;

    Ok(GitHubCopilotOAuthCompletePayload {
        github_login: user.login,
        github_id: user.id,
        github_name: user.name,
        github_email: email,
        github_access_token,
        github_token_type: token_type,
        github_scope: scope,
        copilot_token: copilot.token,
        copilot_plan: copilot.plan,
        copilot_chat_enabled: copilot.chat_enabled,
        copilot_expires_at: copilot.expires_at,
        copilot_refresh_in: copilot.refresh_in,
        copilot_quota_snapshots: copilot.quota_snapshots,
        copilot_quota_reset_date: copilot.quota_reset_date,
        copilot_limited_user_quotas: copilot.limited_user_quotas,
        copilot_limited_user_reset_date: copilot.limited_user_reset_date,
    })
}

#[tauri::command]
pub async fn github_copilot_oauth_login_start() -> Result<GitHubCopilotOAuthStartResponse, String> {
    let payload = request_device_code().await?;
    let login = PendingDeviceLogin {
        login_id: generate_login_id(),
        device_code: payload.device_code,
        user_code: payload.user_code,
        verification_uri: payload.verification_uri,
        verification_uri_complete: payload.verification_uri_complete,
        interval_seconds: payload.interval.unwrap_or(5).max(1),
        expires_at: now_timestamp() + payload.expires_in as i64,
    };
    let response = to_start_response(&login);
    set_pending_login(Some(login));
    Ok(response)
}

#[tauri::command]
pub async fn github_copilot_oauth_login_complete(
    login_id: String,
) -> Result<GitHubCopilotAccountSummary, String> {
    let client = reqwest::Client::new();

    loop {
        let pending = pending_login_for(&login_id)?;
        if now_timestamp() > pending.expires_at {
            set_pending_login(None);
            return Err("GitHub authorization expired. Start again.".to_string());
        }

        let response = exchange_device_token(&client, &pending.device_code).await?;
        if let Some(error) = response.error.as_deref() {
            match error {
                "authorization_pending" => {
                    sleep_with_cancel_check(&login_id, pending.interval_seconds).await?;
                    continue;
                }
                "slow_down" => {
                    sleep_with_cancel_check(&login_id, pending.interval_seconds + 5).await?;
                    continue;
                }
                "expired_token" => {
                    set_pending_login(None);
                    return Err("GitHub authorization expired. Start again.".to_string());
                }
                "access_denied" => {
                    set_pending_login(None);
                    return Err("GitHub authorization was denied.".to_string());
                }
                _ => {
                    return Err(response
                        .error_description
                        .unwrap_or_else(|| format!("GitHub authorization failed: {}", error)));
                }
            }
        }

        let access_token = response
            .access_token
            .ok_or_else(|| "GitHub access token missing".to_string())?;
        set_pending_login(None);
        let payload = build_payload_from_github_access_token(
            access_token,
            response.token_type,
            response.scope,
        )
        .await?;
        return Ok(account_summary(upsert_account(payload)?));
    }
}

#[tauri::command]
pub fn github_copilot_oauth_login_cancel(login_id: Option<String>) -> Result<(), String> {
    if let Some(login_id) = login_id {
        if pending_login()
            .as_ref()
            .map(|login| login.login_id.as_str())
            == Some(login_id.as_str())
        {
            set_pending_login(None);
        }
    } else {
        set_pending_login(None);
    }
    Ok(())
}

#[tauri::command]
pub fn list_github_copilot_accounts() -> Result<Vec<GitHubCopilotAccountSummary>, String> {
    let index = load_index()?;
    Ok(index
        .account_ids
        .iter()
        .filter_map(|id| load_account(id).ok())
        .map(account_summary)
        .collect())
}

#[tauri::command]
pub async fn refresh_github_copilot_account(
    account_id: String,
) -> Result<GitHubCopilotAccountSummary, String> {
    let mut account = load_account(&account_id)?;
    let bundle = fetch_copilot_token(&reqwest::Client::new(), &account.github_access_token).await?;
    let now = now_timestamp();

    account.copilot_token = bundle.token;
    account.copilot_plan = bundle.plan;
    account.copilot_chat_enabled = bundle.chat_enabled;
    account.copilot_expires_at = bundle.expires_at;
    account.copilot_refresh_in = bundle.refresh_in;
    account.copilot_quota_snapshots = bundle.quota_snapshots;
    account.copilot_quota_reset_date = bundle.quota_reset_date;
    account.copilot_limited_user_quotas = bundle.limited_user_quotas;
    account.copilot_limited_user_reset_date = bundle.limited_user_reset_date;
    account.quota_query_last_error = None;
    account.quota_query_last_error_at = None;
    account.usage_updated_at = Some(now);
    account.last_used = now;
    save_account(&account)?;
    Ok(account_summary(account))
}

#[tauri::command]
pub async fn refresh_all_github_copilot_accounts(
) -> Result<Vec<GitHubCopilotAccountSummary>, String> {
    let account_ids = load_index()?.account_ids;
    let mut refreshed = Vec::new();
    for account_id in account_ids {
        if let Ok(account) = refresh_github_copilot_account(account_id).await {
            refreshed.push(account);
        }
    }
    Ok(refreshed)
}

#[tauri::command]
pub fn delete_github_copilot_account(account_id: String) -> Result<(), String> {
    let path = account_path(&account_id)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|err| format!("Could not delete account: {}", err))?;
    }
    let mut index = load_index()?;
    index.account_ids.retain(|id| id != &account_id);
    save_index(&index)
}
