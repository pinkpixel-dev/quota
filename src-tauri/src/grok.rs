use base64::Engine;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const DATA_DIR: &str = ".quota";
const ACCOUNTS_DIR: &str = "grok_accounts";
const ACCOUNTS_INDEX_FILE: &str = "grok_accounts.json";
const GROK_BILLING_ENDPOINT: &str = "https://cli-chat-proxy.grok.com/v1/billing";
const GROK_USER_ENDPOINT: &str = "https://cli-chat-proxy.grok.com/v1/user";
const GROK_SUBSCRIPTIONS_ENDPOINT: &str = "https://grok.com/rest/subscriptions";
const GROK_OAUTH_ISSUER: &str = "https://auth.x.ai";
const GROK_OAUTH_DEVICE_ENDPOINT: &str = "https://auth.x.ai/oauth2/device/code";
const GROK_OAUTH_TOKEN_ENDPOINT: &str = "https://auth.x.ai/oauth2/token";
const GROK_OAUTH_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
const GROK_OAUTH_SCOPES: &str = "openid profile email offline_access grok-cli:access api:access conversations:read conversations:write";
const GROK_OAUTH_CLIENT_SURFACE: &str = "grok-build";
const GROK_OAUTH_TIMEOUT_SECONDS: i64 = 1800;
const GROK_DEVICE_POLL_BUDGET_SECONDS: i64 = 60;
const GROK_REAUTHENTICATION_MESSAGE: &str =
    "Grok authorization is no longer valid. Reconnect Grok to continue.";

static PENDING_GROK_OAUTH: std::sync::LazyLock<Arc<Mutex<Option<PendingGrokOAuth>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(None)));

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokAccountIndex {
    pub version: String,
    pub account_ids: Vec<String>,
}

impl GrokAccountIndex {
    fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            account_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrokTokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredGrokAccount {
    id: String,
    email: String,
    display_name: Option<String>,
    user_id: Option<String>,
    principal_id: Option<String>,
    team_id: Option<String>,
    team_name: Option<String>,
    organization_id: Option<String>,
    organization_name: Option<String>,
    plan: Option<String>,
    tier: Option<i64>,
    #[serde(default)]
    has_grok_code_access: bool,
    oidc_issuer: Option<String>,
    oidc_client_id: Option<String>,
    tokens: Option<GrokTokens>,
    quota: GrokQuotaSummary,
    quota_query_last_error: Option<String>,
    quota_query_last_error_at: Option<i64>,
    #[serde(default)]
    requires_reauthentication: bool,
    usage_updated_at: Option<i64>,
    created_at: i64,
    last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokAccountSummary {
    pub id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub user_id: Option<String>,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
    pub organization_id: Option<String>,
    pub organization_name: Option<String>,
    pub plan: Option<String>,
    pub tier: Option<i64>,
    pub has_grok_code_access: bool,
    pub quota: GrokQuotaSummary,
    pub quota_query_last_error: Option<String>,
    pub quota_query_last_error_at: Option<i64>,
    pub requires_reauthentication: bool,
    pub usage_updated_at: Option<i64>,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokQuotaSummary {
    pub credit_remaining_percent: Option<i32>,
    pub credit_used_percent: Option<f64>,
    pub period_label: Option<String>,
    pub period_start_at: Option<i64>,
    pub period_reset_at: Option<i64>,
    pub period_window_minutes: Option<i64>,
    pub monthly_used: Option<f64>,
    pub monthly_limit: Option<f64>,
    pub monthly_period_start_at: Option<i64>,
    pub monthly_period_end_at: Option<i64>,
    pub on_demand_used: Option<f64>,
    pub on_demand_cap: Option<f64>,
    pub prepaid_balance: Option<f64>,
    #[serde(default)]
    pub product_usage: Vec<GrokProductUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokProductUsage {
    pub product: String,
    pub used_percent: f64,
    pub remaining_percent: i32,
}

#[derive(Debug, Clone)]
pub struct ParsedGrokQuota {
    pub plan: Option<String>,
    pub quota: GrokQuotaSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokOAuthStartResponse {
    pub login_id: String,
    pub auth_url: String,
    pub callback_url: String,
    pub user_code: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
struct PendingGrokOAuth {
    login_id: String,
    device_code: String,
    interval_seconds: i64,
    expires_at: i64,
}

#[derive(Debug, Deserialize)]
struct GrokAuthEntry {
    key: Option<String>,
    refresh_token: Option<String>,
    expires_at: Option<String>,
    email: Option<String>,
    first_name: Option<String>,
    user_id: Option<String>,
    principal_id: Option<String>,
    team_id: Option<String>,
    oidc_issuer: Option<String>,
    oidc_client_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GrokTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<i64>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GrokDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: Option<String>,
    verification_uri_complete: Option<String>,
    expires_in: Option<i64>,
    interval: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GrokJwtPayload {
    sub: Option<String>,
    email: Option<String>,
    principal_id: Option<String>,
    team_id: Option<String>,
    tier: Option<i64>,
    exp: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokUserResponse {
    user_id: Option<String>,
    email: Option<String>,
    first_name: Option<String>,
    principal_id: Option<String>,
    team_id: Option<String>,
    team_name: Option<String>,
    organization_id: Option<String>,
    organization_name: Option<String>,
    has_grok_code_access: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct GrokBillingEnvelope {
    config: Option<GrokBillingConfig>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokBillingConfig {
    current_period: Option<GrokBillingPeriod>,
    credit_usage_percent: Option<f64>,
    product_usage: Option<Vec<GrokBillingProduct>>,
    on_demand_cap: Option<GrokAmount>,
    on_demand_used: Option<GrokAmount>,
    prepaid_balance: Option<GrokAmount>,
    monthly_limit: Option<GrokAmount>,
    used: Option<GrokAmount>,
    subscription_tier: Option<String>,
    billing_period_start: Option<String>,
    billing_period_end: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokBillingPeriod {
    #[serde(rename = "type")]
    period_type: Option<String>,
    start: Option<String>,
    end: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrokBillingProduct {
    product: Option<String>,
    usage_percent: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct GrokAmount {
    val: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct GrokSubscriptionsResponse {
    subscriptions: Option<Vec<GrokSubscription>>,
}

#[derive(Debug, Deserialize)]
struct GrokSubscription {
    tier: Option<String>,
    status: Option<String>,
}

#[tauri::command]
pub fn list_grok_accounts() -> Result<Vec<GrokAccountSummary>, String> {
    list_accounts_in(&quota_storage_dir()?)
}

#[tauri::command]
pub fn import_grok_from_local() -> Result<Vec<GrokAccountSummary>, String> {
    import_from_auth_dir(&grok_home(), &quota_storage_dir()?)
}

#[tauri::command]
pub async fn grok_oauth_login_start() -> Result<GrokOAuthStartResponse, String> {
    let login_id = generate_base64url_token();
    let device = request_device_code().await?;
    let start = build_oauth_start(&login_id, &device);
    set_pending_oauth(Some(PendingGrokOAuth {
        login_id: start.login_id.clone(),
        device_code: device.device_code,
        interval_seconds: device.interval.unwrap_or(5).max(1),
        expires_at: start.expires_at,
    }));
    Ok(start)
}

#[tauri::command]
pub async fn grok_oauth_login_complete(login_id: String) -> Result<GrokAccountSummary, String> {
    let pending = pending_oauth_for(&login_id)?;
    if pending.expires_at <= now_timestamp() {
        set_pending_oauth(None);
        return Err("Grok device login expired. Start again.".to_string());
    }

    let response = poll_device_token(&pending).await?;
    let storage_dir = quota_storage_dir()?;
    let summary = upsert_token_response_in(&storage_dir, &response).await?;
    set_pending_oauth(None);
    refresh_account_in(&storage_dir, &summary.id).await
}

#[tauri::command]
pub fn grok_oauth_login_cancel(login_id: Option<String>) -> Result<(), String> {
    if let Some(login_id) = login_id {
        if pending_oauth()
            .as_ref()
            .map(|pending| pending.login_id.as_str())
            == Some(login_id.as_str())
        {
            set_pending_oauth(None);
        }
    } else {
        set_pending_oauth(None);
    }
    Ok(())
}

#[tauri::command]
pub async fn refresh_grok_account(account_id: String) -> Result<GrokAccountSummary, String> {
    refresh_account_in(&quota_storage_dir()?, &account_id).await
}

#[tauri::command]
pub async fn refresh_all_grok_accounts() -> Result<Vec<GrokAccountSummary>, String> {
    let storage_dir = quota_storage_dir()?;
    let account_ids = load_index_in(&storage_dir)?.account_ids;
    for account_id in account_ids {
        let _ = refresh_account_in(&storage_dir, &account_id).await;
    }
    list_accounts_in(&storage_dir)
}

#[tauri::command]
pub fn delete_grok_account(account_id: String) -> Result<(), String> {
    let storage_dir = quota_storage_dir()?;
    let mut index = load_index_in(&storage_dir)?;
    index.account_ids.retain(|id| id != &account_id);
    save_index_in(&storage_dir, &index)?;

    let path = account_path_in(&storage_dir, &account_id);
    if path.exists() {
        fs::remove_file(&path).map_err(|err| format!("Could not delete Grok account: {}", err))?;
    }
    Ok(())
}

pub fn import_grok_from_auth_dir_for_test(
    auth_dir: &Path,
    storage_dir: &Path,
) -> Result<Vec<GrokAccountSummary>, String> {
    import_from_auth_dir(auth_dir, storage_dir)
}

pub fn parse_grok_quota_for_test(
    credits: &serde_json::Value,
    history: Option<&serde_json::Value>,
) -> Result<ParsedGrokQuota, String> {
    parse_quota_from_values(credits, history)
}

pub fn classify_grok_refresh_failure_for_test(status: u16, body: &str) -> (String, bool) {
    let error = classify_token_refresh_failure(status, body);
    (
        error.message().to_string(),
        error.requires_reauthentication(),
    )
}

fn now_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

fn generate_base64url_token() -> String {
    let bytes: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn quota_storage_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not locate home directory".to_string())?;
    let dir = home.join(DATA_DIR);
    fs::create_dir_all(&dir).map_err(|err| format!("Could not create data directory: {}", err))?;
    Ok(dir)
}

fn grok_home() -> PathBuf {
    if let Some(from_env) = std::env::var("GROK_HOME")
        .ok()
        .map(|raw| raw.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|raw| !raw.is_empty())
    {
        return PathBuf::from(from_env);
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".grok")
}

fn accounts_dir_in(storage_dir: &Path) -> PathBuf {
    storage_dir.join(ACCOUNTS_DIR)
}

fn index_path_in(storage_dir: &Path) -> PathBuf {
    storage_dir.join(ACCOUNTS_INDEX_FILE)
}

fn account_path_in(storage_dir: &Path, account_id: &str) -> PathBuf {
    accounts_dir_in(storage_dir).join(format!("{}.json", account_id))
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

fn load_index_in(storage_dir: &Path) -> Result<GrokAccountIndex, String> {
    let path = index_path_in(storage_dir);
    if !path.exists() {
        return Ok(GrokAccountIndex::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|err| format!("Could not read Grok account index: {}", err))?;
    if content.trim().is_empty() {
        return Ok(GrokAccountIndex::new());
    }
    serde_json::from_str(&content)
        .map_err(|err| format!("Could not parse Grok account index: {}", err))
}

fn save_index_in(storage_dir: &Path, index: &GrokAccountIndex) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index)
        .map_err(|err| format!("Could not encode Grok account index: {}", err))?;
    write_string_atomic(&index_path_in(storage_dir), &content)
}

fn load_account_in(storage_dir: &Path, account_id: &str) -> Result<StoredGrokAccount, String> {
    let content = fs::read_to_string(account_path_in(storage_dir, account_id))
        .map_err(|err| format!("Could not read Grok account: {}", err))?;
    serde_json::from_str(&content).map_err(|err| format!("Could not parse Grok account: {}", err))
}

fn save_account_in(storage_dir: &Path, account: &StoredGrokAccount) -> Result<(), String> {
    let content = serde_json::to_string_pretty(account)
        .map_err(|err| format!("Could not encode Grok account: {}", err))?;
    write_string_atomic(&account_path_in(storage_dir, &account.id), &content)
}

fn list_accounts_in(storage_dir: &Path) -> Result<Vec<GrokAccountSummary>, String> {
    let index = load_index_in(storage_dir)?;
    Ok(index
        .account_ids
        .iter()
        .filter_map(|account_id| load_account_in(storage_dir, account_id).ok())
        .map(|account| account.to_summary())
        .collect())
}

fn import_from_auth_dir(
    auth_dir: &Path,
    storage_dir: &Path,
) -> Result<Vec<GrokAccountSummary>, String> {
    let auth_path = auth_dir.join("auth.json");
    if !auth_path.exists() {
        return Err(format!(
            "Could not find Grok auth file: {}",
            auth_path.display()
        ));
    }

    let content = fs::read_to_string(&auth_path)
        .map_err(|err| format!("Could not read Grok auth file: {}", err))?;
    let entries: BTreeMap<String, GrokAuthEntry> = serde_json::from_str(&content)
        .map_err(|err| format!("Could not parse Grok auth file: {}", err))?;

    let mut summaries = Vec::new();
    let mut last_error: Option<String> = None;
    for (issuer_key, entry) in entries {
        match build_account_from_auth_entry(&issuer_key, entry) {
            Ok(account) => summaries.push(upsert_account_in(storage_dir, account)?),
            Err(error) => last_error = Some(error),
        }
    }

    if summaries.is_empty() {
        return Err(last_error
            .unwrap_or_else(|| "Grok auth file has no importable credentials".to_string()));
    }
    Ok(summaries)
}

fn build_account_from_auth_entry(
    issuer_key: &str,
    entry: GrokAuthEntry,
) -> Result<StoredGrokAccount, String> {
    let access_token = normalize_optional(entry.key)
        .ok_or_else(|| "Grok auth entry does not include an access token".to_string())?;
    let payload = decode_jwt_payload(&access_token).ok();

    let email = normalize_optional(entry.email)
        .or_else(|| payload.as_ref().and_then(|item| item.email.clone()))
        .ok_or_else(|| "Grok auth entry does not include an email".to_string())?;
    let principal_id = normalize_optional(entry.principal_id)
        .or_else(|| payload.as_ref().and_then(|item| item.principal_id.clone()))
        .or_else(|| payload.as_ref().and_then(|item| item.sub.clone()));
    let user_id = normalize_optional(entry.user_id).or_else(|| principal_id.clone());
    let team_id = normalize_optional(entry.team_id)
        .or_else(|| payload.as_ref().and_then(|item| item.team_id.clone()));
    let tier = payload.as_ref().and_then(|item| item.tier);
    let expires_at = normalize_optional(entry.expires_at)
        .and_then(|value| parse_timestamp(&value))
        .or_else(|| payload.as_ref().and_then(|item| item.exp));

    let (issuer, client_id) = split_issuer_key(issuer_key);
    let now = now_timestamp();

    Ok(StoredGrokAccount {
        id: build_account_id(&email, principal_id.as_deref()),
        email,
        display_name: normalize_optional(entry.first_name),
        user_id,
        principal_id,
        team_id,
        team_name: None,
        organization_id: None,
        organization_name: None,
        plan: None,
        tier,
        has_grok_code_access: false,
        oidc_issuer: normalize_optional(entry.oidc_issuer).or(issuer),
        oidc_client_id: normalize_optional(entry.oidc_client_id).or(client_id),
        tokens: Some(GrokTokens {
            access_token,
            refresh_token: normalize_optional(entry.refresh_token),
            expires_at,
        }),
        quota: GrokQuotaSummary::default(),
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        requires_reauthentication: false,
        usage_updated_at: None,
        created_at: now,
        last_used: now,
    })
}

fn split_issuer_key(issuer_key: &str) -> (Option<String>, Option<String>) {
    match issuer_key.split_once("::") {
        Some((issuer, client_id)) => (
            normalize_optional(Some(issuer.to_string())),
            normalize_optional(Some(client_id.to_string())),
        ),
        None => (normalize_optional(Some(issuer_key.to_string())), None),
    }
}

fn build_oauth_start(login_id: &str, device: &GrokDeviceCodeResponse) -> GrokOAuthStartResponse {
    let verification_uri = device
        .verification_uri
        .clone()
        .unwrap_or_else(|| "https://accounts.x.ai/oauth2/device".to_string());
    let auth_url = device.verification_uri_complete.clone().unwrap_or_else(|| {
        format!(
            "{}?user_code={}",
            verification_uri,
            urlencoding::encode(&device.user_code)
        )
    });

    GrokOAuthStartResponse {
        login_id: login_id.to_string(),
        auth_url,
        callback_url: verification_uri,
        user_code: device.user_code.clone(),
        expires_at: now_timestamp()
            + device
                .expires_in
                .unwrap_or(GROK_OAUTH_TIMEOUT_SECONDS)
                .clamp(60, GROK_OAUTH_TIMEOUT_SECONDS),
    }
}

fn pending_oauth() -> Option<PendingGrokOAuth> {
    PENDING_GROK_OAUTH
        .lock()
        .ok()
        .and_then(|state| state.clone())
}

fn set_pending_oauth(pending: Option<PendingGrokOAuth>) {
    if let Ok(mut state) = PENDING_GROK_OAUTH.lock() {
        *state = pending;
    }
}

fn pending_oauth_for(login_id: &str) -> Result<PendingGrokOAuth, String> {
    let pending = pending_oauth()
        .ok_or_else(|| "Grok device login was cancelled. Start again.".to_string())?;
    if pending.login_id != login_id {
        return Err("Grok device login session changed. Start again.".to_string());
    }
    Ok(pending)
}

async fn request_device_code() -> Result<GrokDeviceCodeResponse, String> {
    let response = reqwest::Client::new()
        .post(GROK_OAUTH_DEVICE_ENDPOINT)
        .header(ACCEPT, "application/json")
        .header("x-grok-client-surface", GROK_OAUTH_CLIENT_SURFACE)
        .form(&[
            ("client_id", GROK_OAUTH_CLIENT_ID),
            ("scope", GROK_OAUTH_SCOPES),
        ])
        .send()
        .await
        .map_err(|err| format!("Grok device code request failed: {}", err))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("Could not read Grok device code response: {}", err))?;
    if !status.is_success() {
        return Err(format!(
            "Grok device code request returned {} with body length {}",
            status,
            body.len()
        ));
    }
    serde_json::from_str(&body)
        .map_err(|err| format!("Could not parse Grok device code response: {}", err))
}

async fn poll_device_token(pending: &PendingGrokOAuth) -> Result<GrokTokenResponse, String> {
    let remaining = (pending.expires_at - now_timestamp()).max(0);
    let deadline = now_timestamp() + GROK_DEVICE_POLL_BUDGET_SECONDS.min(remaining);
    let mut interval = pending.interval_seconds.max(1);

    loop {
        let response = request_device_token(&pending.device_code).await?;
        match response.error.as_deref() {
            None => return Ok(response),
            Some("authorization_pending") => {}
            Some("slow_down") => interval += 5,
            Some("expired_token") => {
                set_pending_oauth(None);
                return Err("Grok device login expired. Start again.".to_string());
            }
            Some("access_denied") => {
                set_pending_oauth(None);
                return Err("Grok device login was denied in the browser.".to_string());
            }
            Some(other) => {
                let description = response
                    .error_description
                    .unwrap_or_else(|| other.to_string());
                return Err(format!("Grok device login failed: {}", description));
            }
        }

        if now_timestamp() + interval > deadline {
            return Err("Grok device login is still pending. Approve it in the browser, then select Complete connection again.".to_string());
        }
        tokio::time::sleep(std::time::Duration::from_secs(interval as u64)).await;
    }
}

async fn request_device_token(device_code: &str) -> Result<GrokTokenResponse, String> {
    let response = reqwest::Client::new()
        .post(GROK_OAUTH_TOKEN_ENDPOINT)
        .header(ACCEPT, "application/json")
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", GROK_OAUTH_CLIENT_ID),
            ("device_code", device_code),
        ])
        .send()
        .await
        .map_err(|err| format!("Grok device token request failed: {}", err))?;
    let body = response
        .text()
        .await
        .map_err(|err| format!("Could not read Grok device token response: {}", err))?;
    serde_json::from_str(&body)
        .map_err(|err| format!("Could not parse Grok device token response: {}", err))
}

fn upsert_account_in(
    storage_dir: &Path,
    mut account: StoredGrokAccount,
) -> Result<GrokAccountSummary, String> {
    let mut index = load_index_in(storage_dir)?;
    if let Ok(existing) = load_account_in(storage_dir, &account.id) {
        account.created_at = existing.created_at;
        if account.quota == GrokQuotaSummary::default() {
            account.quota = existing.quota;
            account.usage_updated_at = existing.usage_updated_at;
        }
        if account.plan.is_none() {
            account.plan = existing.plan;
        }
        if account.team_name.is_none() {
            account.team_name = existing.team_name;
        }
        if account.organization_name.is_none() {
            account.organization_name = existing.organization_name;
        }
        if account
            .tokens
            .as_ref()
            .and_then(|tokens| tokens.refresh_token.as_ref())
            .is_none()
        {
            if let Some(refresh_token) = existing
                .tokens
                .and_then(|tokens| tokens.refresh_token)
                .filter(|value| !value.trim().is_empty())
            {
                if let Some(tokens) = account.tokens.as_mut() {
                    tokens.refresh_token = Some(refresh_token);
                }
            }
        }
    }
    account.last_used = now_timestamp();
    save_account_in(storage_dir, &account)?;
    if !index.account_ids.iter().any(|id| id == &account.id) {
        index.account_ids.insert(0, account.id.clone());
    }
    save_index_in(storage_dir, &index)?;
    Ok(account.to_summary())
}

async fn upsert_token_response_in(
    storage_dir: &Path,
    response: &GrokTokenResponse,
) -> Result<GrokAccountSummary, String> {
    if let Some(error) = response.error.as_deref() {
        let description = response
            .error_description
            .clone()
            .unwrap_or_else(|| error.to_string());
        return Err(format!("Grok token response error: {}", description));
    }

    let access_token = normalize_optional(response.access_token.clone())
        .ok_or_else(|| "Grok token response did not include an access_token".to_string())?;
    let payload = decode_jwt_payload(&access_token).ok().or_else(|| {
        normalize_optional(response.id_token.clone())
            .and_then(|token| decode_jwt_payload(&token).ok())
    });

    let profile = fetch_user_profile(&access_token).await.ok();
    let email = profile
        .as_ref()
        .and_then(|item| normalize_optional(item.email.clone()))
        .or_else(|| payload.as_ref().and_then(|item| item.email.clone()))
        .ok_or_else(|| "Grok account does not expose an email".to_string())?;
    let principal_id = profile
        .as_ref()
        .and_then(|item| normalize_optional(item.principal_id.clone()))
        .or_else(|| payload.as_ref().and_then(|item| item.principal_id.clone()))
        .or_else(|| payload.as_ref().and_then(|item| item.sub.clone()));
    let now = now_timestamp();

    let account = StoredGrokAccount {
        id: build_account_id(&email, principal_id.as_deref()),
        email,
        display_name: profile
            .as_ref()
            .and_then(|item| normalize_optional(item.first_name.clone())),
        user_id: profile
            .as_ref()
            .and_then(|item| normalize_optional(item.user_id.clone()))
            .or_else(|| principal_id.clone()),
        principal_id,
        team_id: profile
            .as_ref()
            .and_then(|item| normalize_optional(item.team_id.clone()))
            .or_else(|| payload.as_ref().and_then(|item| item.team_id.clone())),
        team_name: profile
            .as_ref()
            .and_then(|item| normalize_optional(item.team_name.clone())),
        organization_id: profile
            .as_ref()
            .and_then(|item| normalize_optional(item.organization_id.clone())),
        organization_name: profile
            .as_ref()
            .and_then(|item| normalize_optional(item.organization_name.clone())),
        plan: None,
        tier: payload.as_ref().and_then(|item| item.tier),
        has_grok_code_access: profile
            .as_ref()
            .and_then(|item| item.has_grok_code_access)
            .unwrap_or(false),
        oidc_issuer: Some(GROK_OAUTH_ISSUER.to_string()),
        oidc_client_id: Some(GROK_OAUTH_CLIENT_ID.to_string()),
        tokens: Some(GrokTokens {
            access_token,
            refresh_token: normalize_optional(response.refresh_token.clone()),
            expires_at: response
                .expires_in
                .map(|seconds| now + seconds)
                .or_else(|| payload.as_ref().and_then(|item| item.exp)),
        }),
        quota: GrokQuotaSummary::default(),
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        requires_reauthentication: false,
        usage_updated_at: None,
        created_at: now,
        last_used: now,
    };

    upsert_account_in(storage_dir, account)
}

async fn refresh_account_in(
    storage_dir: &Path,
    account_id: &str,
) -> Result<GrokAccountSummary, String> {
    let mut account = load_account_in(storage_dir, account_id)?;

    match fetch_usage(&account).await {
        Ok(parsed) => apply_usage(storage_dir, account, parsed),
        Err(GrokUsageFetchError::Forbidden(_)) => record_refresh_error_in(
            storage_dir,
            account_id,
            GROK_REAUTHENTICATION_MESSAGE.to_string(),
            true,
        ),
        Err(GrokUsageFetchError::Unauthorized(_)) => {
            account = match refresh_account_tokens(storage_dir, account).await {
                Ok(account) => account,
                Err(error) => {
                    return record_refresh_error_in(
                        storage_dir,
                        account_id,
                        error.message().to_string(),
                        error.requires_reauthentication(),
                    );
                }
            };
            match fetch_usage(&account).await {
                Ok(parsed) => apply_usage(storage_dir, account, parsed),
                Err(GrokUsageFetchError::Unauthorized(_))
                | Err(GrokUsageFetchError::Forbidden(_)) => record_refresh_error_in(
                    storage_dir,
                    &account.id,
                    GROK_REAUTHENTICATION_MESSAGE.to_string(),
                    true,
                ),
                Err(error) => record_usage_error(storage_dir, account, error.to_string()),
            }
        }
        Err(error) => record_usage_error(storage_dir, account, error.to_string()),
    }
}

fn apply_usage(
    storage_dir: &Path,
    mut account: StoredGrokAccount,
    parsed: ParsedGrokQuota,
) -> Result<GrokAccountSummary, String> {
    account.plan = parsed.plan.or(account.plan);
    account.quota = parsed.quota;
    account.quota_query_last_error = None;
    account.quota_query_last_error_at = None;
    account.requires_reauthentication = false;
    account.usage_updated_at = Some(now_timestamp());
    account.last_used = now_timestamp();
    save_account_in(storage_dir, &account)?;
    Ok(account.to_summary())
}

fn record_usage_error(
    storage_dir: &Path,
    mut account: StoredGrokAccount,
    message: String,
) -> Result<GrokAccountSummary, String> {
    account.quota_query_last_error = Some(message.clone());
    account.quota_query_last_error_at = Some(now_timestamp());
    account.requires_reauthentication = false;
    save_account_in(storage_dir, &account)?;
    Err(message)
}

fn record_refresh_error_in(
    storage_dir: &Path,
    account_id: &str,
    message: String,
    requires_reauthentication: bool,
) -> Result<GrokAccountSummary, String> {
    let mut account = load_account_in(storage_dir, account_id)?;
    account.quota_query_last_error = Some(message);
    account.quota_query_last_error_at = Some(now_timestamp());
    account.requires_reauthentication = requires_reauthentication;
    account.last_used = now_timestamp();
    save_account_in(storage_dir, &account)?;
    Ok(account.to_summary())
}

#[derive(Debug)]
enum GrokUsageFetchError {
    Unauthorized(String),
    Forbidden(String),
    Other(String),
}

impl std::fmt::Display for GrokUsageFetchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized(message) | Self::Forbidden(message) | Self::Other(message) => {
                formatter.write_str(message)
            }
        }
    }
}

async fn fetch_usage(account: &StoredGrokAccount) -> Result<ParsedGrokQuota, GrokUsageFetchError> {
    let token = account
        .tokens
        .as_ref()
        .map(|tokens| tokens.access_token.trim())
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            GrokUsageFetchError::Other("Grok account does not have an access token".to_string())
        })?;

    let credits = fetch_billing(token, Some("credits")).await?;
    let history = fetch_billing(token, None).await.ok();
    let mut parsed =
        parse_quota_from_values(&credits, history.as_ref()).map_err(GrokUsageFetchError::Other)?;
    if parsed.plan.is_none() {
        parsed.plan = fetch_subscription_tier(token).await;
    }
    Ok(parsed)
}

async fn fetch_billing(
    token: &str,
    format: Option<&str>,
) -> Result<serde_json::Value, GrokUsageFetchError> {
    let url = match format {
        Some(format) => format!("{}?format={}", GROK_BILLING_ENDPOINT, format),
        None => GROK_BILLING_ENDPOINT.to_string(),
    };
    let response = reqwest::Client::new()
        .get(&url)
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .map_err(|err| GrokUsageFetchError::Other(format!("Grok billing request failed: {}", err)))?;
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        GrokUsageFetchError::Other(format!("Could not read Grok billing response: {}", err))
    })?;

    if !status.is_success() {
        let message = format!(
            "Grok billing API returned {} with body length {}",
            status,
            body.len()
        );
        if status.as_u16() == 401 {
            return Err(GrokUsageFetchError::Unauthorized(message));
        }
        if status.as_u16() == 403 {
            // A valid token without the grok-cli/api scopes. Refreshing reissues the
            // same scopes, so the account has to be connected again.
            return Err(GrokUsageFetchError::Forbidden(message));
        }
        return Err(GrokUsageFetchError::Other(message));
    }

    serde_json::from_str(&body).map_err(|err| {
        GrokUsageFetchError::Other(format!("Could not parse Grok billing JSON: {}", err))
    })
}

async fn fetch_user_profile(token: &str) -> Result<GrokUserResponse, String> {
    let response = reqwest::Client::new()
        .get(GROK_USER_ENDPOINT)
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .map_err(|err| format!("Grok user request failed: {}", err))?;
    if !response.status().is_success() {
        return Err(format!(
            "Grok user API returned {}",
            response.status().as_u16()
        ));
    }
    response
        .json::<GrokUserResponse>()
        .await
        .map_err(|err| format!("Could not parse Grok user response: {}", err))
}

async fn fetch_subscription_tier(token: &str) -> Option<String> {
    let response = reqwest::Client::new()
        .get(GROK_SUBSCRIPTIONS_ENDPOINT)
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload = response.json::<GrokSubscriptionsResponse>().await.ok()?;
    payload
        .subscriptions?
        .into_iter()
        .find(|item| {
            item.status
                .as_deref()
                .map(|status| status.contains("ACTIVE"))
                .unwrap_or(false)
        })
        .and_then(|item| item.tier)
        .map(|tier| humanize_subscription_tier(&tier))
}

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
    let lower = value.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

async fn refresh_account_tokens(
    storage_dir: &Path,
    account: StoredGrokAccount,
) -> Result<StoredGrokAccount, GrokTokenRefreshError> {
    let refresh_token = account
        .tokens
        .as_ref()
        .and_then(|tokens| normalize_optional(tokens.refresh_token.clone()))
        .ok_or(GrokTokenRefreshError::ReauthenticationRequired)?;
    let response = request_token_refresh(&refresh_token).await?;

    let access_token = normalize_optional(response.access_token.clone()).ok_or_else(|| {
        GrokTokenRefreshError::Other(
            "Grok token refresh did not include an access_token".to_string(),
        )
    })?;

    let mut account = account;
    let payload = decode_jwt_payload(&access_token).ok();
    account.tier = payload.as_ref().and_then(|item| item.tier).or(account.tier);
    account.tokens = Some(GrokTokens {
        access_token,
        refresh_token: normalize_optional(response.refresh_token.clone()).or(Some(refresh_token)),
        expires_at: response
            .expires_in
            .map(|seconds| now_timestamp() + seconds)
            .or_else(|| payload.as_ref().and_then(|item| item.exp)),
    });
    account.last_used = now_timestamp();
    save_account_in(storage_dir, &account).map_err(GrokTokenRefreshError::Other)?;
    Ok(account)
}

async fn request_token_refresh(
    refresh_token: &str,
) -> Result<GrokTokenResponse, GrokTokenRefreshError> {
    let response = reqwest::Client::new()
        .post(GROK_OAUTH_TOKEN_ENDPOINT)
        .header(ACCEPT, "application/json")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", GROK_OAUTH_CLIENT_ID),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|err| {
            GrokTokenRefreshError::Other(format!("Grok token refresh request failed: {}", err))
        })?;
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        GrokTokenRefreshError::Other(format!(
            "Could not read Grok token refresh response: {}",
            err
        ))
    })?;
    if !status.is_success() {
        return Err(classify_token_refresh_failure(status.as_u16(), &body));
    }
    let parsed: GrokTokenResponse = serde_json::from_str(&body).map_err(|err| {
        GrokTokenRefreshError::Other(format!("Could not parse Grok token response: {}", err))
    })?;
    if let Some(error) = parsed.error.as_deref() {
        return Err(classify_token_refresh_failure(400, error));
    }
    Ok(parsed)
}

#[derive(Debug)]
enum GrokTokenRefreshError {
    ReauthenticationRequired,
    Other(String),
}

impl GrokTokenRefreshError {
    fn message(&self) -> &str {
        match self {
            Self::ReauthenticationRequired => GROK_REAUTHENTICATION_MESSAGE,
            Self::Other(message) => message,
        }
    }

    fn requires_reauthentication(&self) -> bool {
        matches!(self, Self::ReauthenticationRequired)
    }
}

fn classify_token_refresh_failure(status: u16, body: &str) -> GrokTokenRefreshError {
    let normalized = body.to_lowercase();
    let rejected_refresh_token = status == 401
        || status == 403
        || (status == 400
            && [
                "invalid_grant",
                "invalid_token",
                "refresh token expired",
                "refresh_token_expired",
                "refresh token revoked",
                "refresh_token_revoked",
            ]
            .iter()
            .any(|signal| normalized.contains(signal)));

    if rejected_refresh_token {
        GrokTokenRefreshError::ReauthenticationRequired
    } else {
        GrokTokenRefreshError::Other(format!(
            "Grok token refresh returned {} with body length {}",
            status,
            body.len()
        ))
    }
}

fn parse_quota_from_values(
    credits: &serde_json::Value,
    history: Option<&serde_json::Value>,
) -> Result<ParsedGrokQuota, String> {
    let credits: GrokBillingEnvelope = serde_json::from_value(credits.clone())
        .map_err(|err| format!("Could not parse Grok billing payload: {}", err))?;
    let credits = credits.config.unwrap_or_default();

    let history = history
        .and_then(|value| serde_json::from_value::<GrokBillingEnvelope>(value.clone()).ok())
        .and_then(|envelope| envelope.config)
        .unwrap_or_default();

    let period = credits.current_period.as_ref();
    let period_start_at = period
        .and_then(|item| item.start.as_deref())
        .and_then(parse_timestamp);
    let period_reset_at = period
        .and_then(|item| item.end.as_deref())
        .and_then(parse_timestamp)
        .or_else(|| {
            credits
                .billing_period_end
                .as_deref()
                .and_then(parse_timestamp)
        });

    let product_usage = credits
        .product_usage
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let product = normalize_optional(item.product)?;
            let used_percent = item.usage_percent?;
            Some(GrokProductUsage {
                product,
                used_percent,
                remaining_percent: remaining_percent(used_percent),
            })
        })
        .collect::<Vec<_>>();

    let credit_used_percent = credits.credit_usage_percent.or_else(|| {
        product_usage
            .iter()
            .map(|item| item.used_percent)
            .fold(None::<f64>, |acc, value| {
                Some(acc.map_or(value, |current| current.max(value)))
            })
    });

    Ok(ParsedGrokQuota {
        plan: normalize_optional(credits.subscription_tier)
            .map(|tier| humanize_subscription_tier(&tier)),
        quota: GrokQuotaSummary {
            credit_remaining_percent: credit_used_percent.map(remaining_percent),
            credit_used_percent,
            period_label: normalize_optional(period.and_then(|item| item.period_type.clone()))
                .map(|value| humanize_period_type(&value)),
            period_start_at,
            period_reset_at,
            period_window_minutes: window_minutes(period_start_at, period_reset_at),
            monthly_used: amount_value(history.used.as_ref()),
            monthly_limit: amount_value(history.monthly_limit.as_ref())
                .filter(|value| *value > 0.0),
            monthly_period_start_at: history
                .billing_period_start
                .as_deref()
                .and_then(parse_timestamp),
            monthly_period_end_at: history
                .billing_period_end
                .as_deref()
                .and_then(parse_timestamp),
            on_demand_used: amount_value(
                credits
                    .on_demand_used
                    .as_ref()
                    .or(history.on_demand_used.as_ref()),
            ),
            on_demand_cap: amount_value(
                credits
                    .on_demand_cap
                    .as_ref()
                    .or(history.on_demand_cap.as_ref()),
            )
            .filter(|value| *value > 0.0),
            prepaid_balance: amount_value(credits.prepaid_balance.as_ref()),
            product_usage,
        },
    })
}

fn amount_value(amount: Option<&GrokAmount>) -> Option<f64> {
    amount.and_then(|item| item.val)
}

fn remaining_percent(used_percent: f64) -> i32 {
    let used = used_percent.round().clamp(0.0, 100.0) as i32;
    100 - used
}

fn humanize_period_type(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches("USAGE_PERIOD_TYPE_");
    if trimmed.is_empty() {
        return raw.trim().to_string();
    }
    capitalize(trimmed)
}

fn window_minutes(start_at: Option<i64>, end_at: Option<i64>) -> Option<i64> {
    let start = start_at?;
    let end = end_at?;
    let seconds = end - start;
    if seconds <= 0 {
        return None;
    }
    Some((seconds + 59) / 60)
}

fn parse_timestamp(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Some(parsed.timestamp());
    }
    trimmed.parse::<i64>().ok().map(|raw| {
        if raw > 10_000_000_000 {
            raw / 1000
        } else {
            raw
        }
    })
}

fn decode_jwt_payload(token: &str) -> Result<GrokJwtPayload, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return Err("Invalid Grok JWT token format".to_string());
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|err| format!("Could not decode Grok JWT payload: {}", err))?;
    serde_json::from_slice(&bytes).map_err(|err| format!("Could not parse Grok JWT JSON: {}", err))
}

fn build_account_id(email: &str, principal_id: Option<&str>) -> String {
    let mut seed = email.trim().to_lowercase();
    if let Some(value) = principal_id.and_then(|value| normalize_optional(Some(value.to_string()))) {
        seed.push('|');
        seed.push_str(&value);
    }
    format!("grok_{:x}", md5::compute(seed.as_bytes()))
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

impl StoredGrokAccount {
    fn to_summary(&self) -> GrokAccountSummary {
        GrokAccountSummary {
            id: self.id.clone(),
            email: self.email.clone(),
            display_name: self.display_name.clone(),
            user_id: self.user_id.clone(),
            team_id: self.team_id.clone(),
            team_name: self.team_name.clone(),
            organization_id: self.organization_id.clone(),
            organization_name: self.organization_name.clone(),
            plan: self.plan.clone(),
            tier: self.tier,
            has_grok_code_access: self.has_grok_code_access,
            quota: self.quota.clone(),
            quota_query_last_error: self.quota_query_last_error.clone(),
            quota_query_last_error_at: self.quota_query_last_error_at,
            requires_reauthentication: self.requires_reauthentication,
            usage_updated_at: self.usage_updated_at,
            created_at: self.created_at,
            last_used: self.last_used,
        }
    }
}
