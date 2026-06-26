use base64::Engine;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const DATA_DIR: &str = ".quota";
const ACCOUNTS_DIR: &str = "claude_accounts";
const ACCOUNTS_INDEX_FILE: &str = "claude_accounts.json";
const CLAUDE_OAUTH_AUTHORIZE_URL: &str = "https://claude.com/cai/oauth/authorize";
const CLAUDE_OAUTH_CALLBACK_URL: &str = "https://platform.claude.com/oauth/code/callback";
const CLAUDE_OAUTH_TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const CLAUDE_OAUTH_PROFILE_URL: &str = "https://api.anthropic.com/api/oauth/profile";
const CLAUDE_OAUTH_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const CLAUDE_OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const CLAUDE_OAUTH_BETA_HEADER: &str = "oauth-2025-04-20";
const CLAUDE_OAUTH_TIMEOUT_SECONDS: i64 = 600;
const CLAUDE_OAUTH_SCOPES: [&str; 6] = [
    "org:create_api_key",
    "user:profile",
    "user:inference",
    "user:sessions:claude_code",
    "user:mcp_servers",
    "user:file_upload",
];

static PENDING_CLAUDE_OAUTH: std::sync::LazyLock<Arc<Mutex<Option<PendingClaudeOAuth>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(None)));

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAccountIndex {
    pub version: String,
    pub account_ids: Vec<String>,
}

impl ClaudeAccountIndex {
    fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            account_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredClaudeAccount {
    id: String,
    email: String,
    auth_mode: String,
    access_token: String,
    refresh_token: Option<String>,
    token_type: Option<String>,
    scopes: Vec<String>,
    expires_at: Option<i64>,
    account_uuid: Option<String>,
    organization_uuid: Option<String>,
    organization_name: Option<String>,
    display_name: Option<String>,
    avatar_url: Option<String>,
    plan_type: Option<String>,
    quota: ClaudeQuotaSummary,
    quota_query_last_error: Option<String>,
    quota_query_last_error_at: Option<i64>,
    usage_updated_at: Option<i64>,
    profile_updated_at: Option<i64>,
    created_at: i64,
    last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeAccountSummary {
    pub id: String,
    pub email: String,
    pub auth_mode: String,
    pub account_uuid: Option<String>,
    pub organization_uuid: Option<String>,
    pub organization_name: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub plan_type: Option<String>,
    pub quota: ClaudeQuotaSummary,
    pub quota_query_last_error: Option<String>,
    pub quota_query_last_error_at: Option<i64>,
    pub usage_updated_at: Option<i64>,
    pub profile_updated_at: Option<i64>,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeQuotaSummary {
    pub five_hour_remaining_percent: Option<i32>,
    pub five_hour_reset_at: Option<i64>,
    pub weekly_remaining_percent: Option<i32>,
    pub weekly_reset_at: Option<i64>,
    pub weekly_sonnet_remaining_percent: Option<i32>,
    pub weekly_sonnet_reset_at: Option<i64>,
    pub extra_usage_remaining_percent: Option<i32>,
    pub extra_usage_reset_at: Option<i64>,
    pub extra_usage_used_cents: Option<i64>,
    pub extra_usage_limit_cents: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeOAuthStartResponse {
    pub login_id: String,
    pub auth_url: String,
    pub callback_url: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
struct PendingClaudeOAuth {
    login_id: String,
    state: String,
    code_verifier: String,
    expires_at: i64,
}

#[derive(Debug, Deserialize)]
struct ClaudeTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    token_type: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
    error: Option<Value>,
    error_description: Option<String>,
}

#[tauri::command]
pub fn list_claude_accounts() -> Result<Vec<ClaudeAccountSummary>, String> {
    list_accounts_in(&quota_storage_dir()?)
}

#[tauri::command]
pub fn claude_oauth_login_start() -> Result<ClaudeOAuthStartResponse, String> {
    let login_id = generate_base64url_token();
    let state = generate_base64url_token();
    let code_verifier = generate_base64url_token();
    let start = build_oauth_start(&login_id, &state, &code_verifier)?;
    set_pending_oauth(Some(PendingClaudeOAuth {
        login_id: login_id.clone(),
        state,
        code_verifier,
        expires_at: start.expires_at,
    }));
    Ok(start)
}

#[tauri::command]
pub async fn claude_oauth_login_complete(
    login_id: String,
    callback_or_code: String,
    email_hint: Option<String>,
) -> Result<ClaudeAccountSummary, String> {
    let pending = pending_oauth_for(&login_id)?;
    if pending.expires_at <= now_timestamp() {
        set_pending_oauth(None);
        return Err("Claude OAuth login expired. Start again.".to_string());
    }
    let (code, callback_state) = parse_callback_input(&callback_or_code)?;
    if let Some(callback_state) = callback_state {
        if callback_state != pending.state {
            return Err("Claude OAuth callback state did not match. Start again.".to_string());
        }
    }
    let token_response = exchange_oauth_code(&pending, &code).await?;
    let access_token = read_string_path(&token_response, &["access_token"]).ok_or_else(|| {
        "Claude OAuth token response did not include an access token.".to_string()
    })?;
    let profile = request_oauth_profile(&access_token).await.ok();
    let summary = upsert_token_response_in(
        &quota_storage_dir()?,
        &token_response,
        profile.as_ref(),
        email_hint.as_deref(),
    )?;
    set_pending_oauth(None);
    Ok(summary)
}

#[tauri::command]
pub fn claude_oauth_login_cancel(login_id: Option<String>) -> Result<(), String> {
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
pub async fn refresh_claude_account(account_id: String) -> Result<ClaudeAccountSummary, String> {
    refresh_account_in(&quota_storage_dir()?, &account_id).await
}

#[tauri::command]
pub async fn refresh_all_claude_accounts() -> Result<Vec<ClaudeAccountSummary>, String> {
    let storage_dir = quota_storage_dir()?;
    let account_ids = load_index_in(&storage_dir)?.account_ids;
    for account_id in account_ids {
        let _ = refresh_account_in(&storage_dir, &account_id).await;
    }
    list_accounts_in(&storage_dir)
}

#[tauri::command]
pub fn delete_claude_account(account_id: String) -> Result<(), String> {
    let storage_dir = quota_storage_dir()?;
    let mut index = load_index_in(&storage_dir)?;
    index.account_ids.retain(|id| id != &account_id);
    save_index_in(&storage_dir, &index)?;

    let path = account_path_in(&storage_dir, &account_id);
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("Could not delete Claude account: {}", err))?;
    }
    Ok(())
}

pub fn build_claude_oauth_start_for_test(
    login_id: &str,
    state: &str,
    code_verifier: &str,
) -> Result<ClaudeOAuthStartResponse, String> {
    build_oauth_start(login_id, state, code_verifier)
}

pub fn parse_claude_callback_input_for_test(
    input: &str,
) -> Result<(String, Option<String>), String> {
    parse_callback_input(input)
}

pub fn parse_claude_quota_for_test(raw: &Value) -> ClaudeQuotaSummary {
    parse_quota_from_value(raw)
}

pub fn apply_claude_token_response_for_test(
    storage_dir: &Path,
    response: &Value,
    profile: Option<&Value>,
) -> Result<ClaudeAccountSummary, String> {
    upsert_token_response_in(storage_dir, response, profile, None)
}

fn now_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

fn now_timestamp_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
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

fn load_index_in(storage_dir: &Path) -> Result<ClaudeAccountIndex, String> {
    let path = index_path_in(storage_dir);
    if !path.exists() {
        return Ok(ClaudeAccountIndex::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|err| format!("Could not read Claude account index: {}", err))?;
    if content.trim().is_empty() {
        return Ok(ClaudeAccountIndex::new());
    }
    serde_json::from_str(&content)
        .map_err(|err| format!("Could not parse Claude account index: {}", err))
}

fn save_index_in(storage_dir: &Path, index: &ClaudeAccountIndex) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index)
        .map_err(|err| format!("Could not encode Claude account index: {}", err))?;
    write_string_atomic(&index_path_in(storage_dir), &content)
}

fn load_account_in(storage_dir: &Path, account_id: &str) -> Result<StoredClaudeAccount, String> {
    let content = fs::read_to_string(account_path_in(storage_dir, account_id))
        .map_err(|err| format!("Could not read Claude account: {}", err))?;
    serde_json::from_str(&content).map_err(|err| format!("Could not parse Claude account: {}", err))
}

fn save_account_in(storage_dir: &Path, account: &StoredClaudeAccount) -> Result<(), String> {
    let content = serde_json::to_string_pretty(account)
        .map_err(|err| format!("Could not encode Claude account: {}", err))?;
    write_string_atomic(&account_path_in(storage_dir, &account.id), &content)
}

fn list_accounts_in(storage_dir: &Path) -> Result<Vec<ClaudeAccountSummary>, String> {
    let index = load_index_in(storage_dir)?;
    Ok(index
        .account_ids
        .iter()
        .filter_map(|account_id| load_account_in(storage_dir, account_id).ok())
        .map(|account| account.to_summary())
        .collect())
}

fn build_oauth_start(
    login_id: &str,
    state: &str,
    code_verifier: &str,
) -> Result<ClaudeOAuthStartResponse, String> {
    let code_challenge = pkce_challenge(code_verifier);
    let scope = CLAUDE_OAUTH_SCOPES.join(" ");
    let auth_url = format!(
        "{}?code=true&client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        CLAUDE_OAUTH_AUTHORIZE_URL,
        urlencoding::encode(CLAUDE_OAUTH_CLIENT_ID),
        urlencoding::encode(CLAUDE_OAUTH_CALLBACK_URL),
        urlencoding::encode(&scope),
        urlencoding::encode(&code_challenge),
        urlencoding::encode(state)
    );

    Ok(ClaudeOAuthStartResponse {
        login_id: login_id.to_string(),
        auth_url,
        callback_url: CLAUDE_OAUTH_CALLBACK_URL.to_string(),
        expires_at: now_timestamp() + CLAUDE_OAUTH_TIMEOUT_SECONDS,
    })
}

fn pkce_challenge(code_verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn pending_oauth() -> Option<PendingClaudeOAuth> {
    PENDING_CLAUDE_OAUTH
        .lock()
        .ok()
        .and_then(|state| state.clone())
}

fn set_pending_oauth(pending: Option<PendingClaudeOAuth>) {
    if let Ok(mut state) = PENDING_CLAUDE_OAUTH.lock() {
        *state = pending;
    }
}

fn pending_oauth_for(login_id: &str) -> Result<PendingClaudeOAuth, String> {
    let pending = pending_oauth()
        .ok_or_else(|| "Claude OAuth login was cancelled. Start again.".to_string())?;
    if pending.login_id != login_id {
        return Err("Claude OAuth login session changed. Start again.".to_string());
    }
    Ok(pending)
}

fn parse_callback_input(input: &str) -> Result<(String, Option<String>), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Claude OAuth callback URL or code is required.".to_string());
    }

    if let Some(query) = trimmed.split_once('?').map(|(_, query)| query).or_else(|| {
        trimmed
            .strip_prefix('?')
            .map(|query| ("", query))
            .map(|(_, q)| q)
    }) {
        let params = parse_query_params(query);
        if let Some(code) = params.get("code") {
            return Ok(clean_code_and_state(code));
        }
    }

    Ok(clean_code_and_state(trimmed.trim_start_matches("code=")))
}

fn clean_code_and_state(raw: &str) -> (String, Option<String>) {
    let mut code = raw.trim();
    let mut state = None;
    if let Some((before, after)) = code.split_once('#') {
        code = before;
        state = normalize_optional(Some(after.to_string()));
    }
    if let Some((before, _after)) = code.split_once('&') {
        code = before;
    }
    (code.trim().to_string(), state)
}

fn parse_query_params(query: &str) -> std::collections::HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let key = urlencoding::decode(key).ok()?.into_owned();
            let value = urlencoding::decode(value).ok()?.into_owned();
            Some((key, value))
        })
        .collect()
}

async fn exchange_oauth_code(pending: &PendingClaudeOAuth, code: &str) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .post(CLAUDE_OAUTH_TOKEN_URL)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json, text/plain, */*")
        .header(USER_AGENT, "quota")
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": CLAUDE_OAUTH_CLIENT_ID,
            "code": code,
            "redirect_uri": CLAUDE_OAUTH_CALLBACK_URL,
            "code_verifier": pending.code_verifier,
            "state": pending.state
        }))
        .send()
        .await
        .map_err(|err| format!("Claude OAuth token request failed: {}", err))?;
    parse_response_json(response, "Claude OAuth token exchange").await
}

async fn request_oauth_profile(access_token: &str) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .get(CLAUDE_OAUTH_PROFILE_URL)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(CONTENT_TYPE, "application/json")
        .header(USER_AGENT, "quota")
        .send()
        .await
        .map_err(|err| format!("Claude profile request failed: {}", err))?;
    parse_response_json(response, "Claude OAuth profile").await
}

async fn request_usage(access_token: &str) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .get(CLAUDE_OAUTH_USAGE_URL)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header("anthropic-beta", CLAUDE_OAUTH_BETA_HEADER)
        .header(USER_AGENT, "quota")
        .send()
        .await
        .map_err(|err| format!("Claude usage request failed: {}", err))?;
    parse_response_json(response, "Claude usage").await
}

async fn parse_response_json(response: reqwest::Response, label: &str) -> Result<Value, String> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("Could not read {} response: {}", label, err))?;
    if !status.is_success() {
        return Err(format!("{} failed: status={} {}", label, status, body));
    }
    serde_json::from_str(&body).map_err(|err| {
        format!(
            "Could not parse {} response: {} status={} body_length={}",
            label,
            err,
            status,
            body.len()
        )
    })
}

fn upsert_token_response_in(
    storage_dir: &Path,
    response: &Value,
    profile: Option<&Value>,
    email_hint: Option<&str>,
) -> Result<ClaudeAccountSummary, String> {
    let token_response: ClaudeTokenResponse = serde_json::from_value(response.clone())
        .map_err(|err| format!("Could not parse Claude token response: {}", err))?;
    if token_response.error.is_some() {
        return Err(token_response.error_description.unwrap_or_else(|| {
            format!("Claude token response error: {:?}", token_response.error)
        }));
    }

    let access_token = token_response
        .access_token
        .and_then(|value| normalize_optional(Some(value)))
        .ok_or_else(|| "Claude token response did not include an access token.".to_string())?;
    let refresh_token = token_response
        .refresh_token
        .and_then(|value| normalize_optional(Some(value)));
    let account_uuid = first_non_empty([
        profile.and_then(|value| read_string_path(value, &["account", "uuid"])),
        read_string_path(response, &["account", "uuid"]),
    ]);
    let email = first_non_empty([
        profile.and_then(|value| read_string_path(value, &["account", "email"])),
        profile.and_then(|value| read_string_path(value, &["account", "email_address"])),
        read_string_path(response, &["account", "email_address"]),
        email_hint.and_then(|value| normalize_optional(Some(value.to_string()))),
    ])
    .ok_or_else(|| "Claude OAuth response did not include an email.".to_string())?;
    let organization_uuid = first_non_empty([
        profile.and_then(|value| read_string_path(value, &["organization", "uuid"])),
        read_string_path(response, &["organization", "uuid"]),
    ]);
    let organization_name = first_non_empty([
        profile.and_then(|value| read_string_path(value, &["organization", "name"])),
        profile.and_then(|value| read_string_path(value, &["organization", "display_name"])),
        read_string_path(response, &["organization", "name"]),
    ]);
    let display_name =
        profile.and_then(|value| read_string_path(value, &["account", "display_name"]));
    let avatar_url = first_non_empty([
        profile.and_then(|value| read_string_path(value, &["account", "avatar_url"])),
        profile.and_then(|value| read_string_path(value, &["account", "avatarUrl"])),
    ]);
    let plan_type = subscription_type_from_profile(profile);
    let now = now_timestamp();
    let id = build_account_id(
        &email,
        account_uuid.as_deref(),
        organization_uuid.as_deref(),
    );
    let scopes = token_response
        .scope
        .map(|scope| {
            scope
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| {
            CLAUDE_OAUTH_SCOPES
                .iter()
                .map(|item| item.to_string())
                .collect()
        });

    let account = StoredClaudeAccount {
        id,
        email,
        auth_mode: "oauth".to_string(),
        access_token,
        refresh_token,
        token_type: token_response
            .token_type
            .and_then(|value| normalize_optional(Some(value))),
        scopes,
        expires_at: token_response
            .expires_in
            .map(|seconds| now_timestamp_ms() + seconds.saturating_mul(1000)),
        account_uuid,
        organization_uuid,
        organization_name,
        display_name,
        avatar_url,
        plan_type,
        quota: ClaudeQuotaSummary::default(),
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        usage_updated_at: None,
        profile_updated_at: profile.map(|_| now_timestamp_ms()),
        created_at: now,
        last_used: now,
    };

    upsert_account_in(storage_dir, account)
}

fn upsert_account_in(
    storage_dir: &Path,
    mut account: StoredClaudeAccount,
) -> Result<ClaudeAccountSummary, String> {
    let mut index = load_index_in(storage_dir)?;
    if let Ok(existing) = load_account_in(storage_dir, &account.id) {
        account.created_at = existing.created_at;
        if account.quota == ClaudeQuotaSummary::default() {
            account.quota = existing.quota;
        }
        account.usage_updated_at = existing.usage_updated_at;
    }
    account.last_used = now_timestamp();

    if !index.account_ids.iter().any(|id| id == &account.id) {
        index.account_ids.insert(0, account.id.clone());
    }

    save_account_in(storage_dir, &account)?;
    save_index_in(storage_dir, &index)?;
    Ok(account.to_summary())
}

async fn refresh_account_in(
    storage_dir: &Path,
    account_id: &str,
) -> Result<ClaudeAccountSummary, String> {
    let mut account = load_account_in(storage_dir, account_id)?;
    if let Err(error) = ensure_access_token_valid(&mut account).await {
        return record_refresh_error_in(storage_dir, account, error);
    }

    match request_usage(&account.access_token).await {
        Ok(usage) => {
            account.quota = parse_quota_from_value(&usage);
            account.quota_query_last_error = None;
            account.quota_query_last_error_at = None;
            account.usage_updated_at = Some(now_timestamp_ms());
        }
        Err(error) => {
            account.quota_query_last_error = Some(error);
            account.quota_query_last_error_at = Some(now_timestamp_ms());
        }
    }
    account.last_used = now_timestamp();
    save_account_in(storage_dir, &account)?;
    Ok(account.to_summary())
}

async fn ensure_access_token_valid(account: &mut StoredClaudeAccount) -> Result<(), String> {
    let should_refresh = account
        .expires_at
        .map(|expires_at| expires_at <= now_timestamp_ms() + 300_000)
        .unwrap_or(false);
    if !should_refresh {
        return Ok(());
    }
    let refresh_token = account
        .refresh_token
        .clone()
        .ok_or_else(|| "Claude refresh token is missing.".to_string())?;
    let response = reqwest::Client::new()
        .post(CLAUDE_OAUTH_TOKEN_URL)
        .header(CONTENT_TYPE, "application/json")
        .header(USER_AGENT, "quota")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": CLAUDE_OAUTH_CLIENT_ID
        }))
        .send()
        .await
        .map_err(|err| format!("Claude token refresh failed: {}", err))?;
    let payload = parse_response_json(response, "Claude token refresh").await?;
    if let Some(access_token) = read_string_path(&payload, &["access_token"]) {
        account.access_token = access_token;
    }
    if let Some(refresh_token) = read_string_path(&payload, &["refresh_token"]) {
        account.refresh_token = Some(refresh_token);
    }
    if let Some(expires_in) = read_i64_value(payload.get("expires_in")) {
        account.expires_at = Some(now_timestamp_ms() + expires_in.saturating_mul(1000));
    }
    Ok(())
}

fn record_refresh_error_in(
    storage_dir: &Path,
    mut account: StoredClaudeAccount,
    error: String,
) -> Result<ClaudeAccountSummary, String> {
    account.last_used = now_timestamp();
    account.quota_query_last_error = Some(error);
    account.quota_query_last_error_at = Some(now_timestamp_ms());
    save_account_in(storage_dir, &account)?;
    Ok(account.to_summary())
}

fn parse_quota_from_value(raw: &Value) -> ClaudeQuotaSummary {
    let five_hour = raw.get("five_hour");
    let weekly = raw.get("seven_day");
    let weekly_sonnet = raw
        .get("seven_day_sonnet")
        .or_else(|| raw.get("seven_day_sonnet_4"))
        .or_else(|| raw.get("seven_day_model"));
    let extra_usage = raw.get("extra_usage");
    let extra_enabled = extra_usage
        .and_then(|item| item.get("is_enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    ClaudeQuotaSummary {
        five_hour_remaining_percent: remaining_percent(
            five_hour.and_then(|item| item.get("utilization")),
        ),
        five_hour_reset_at: parse_reset_seconds(five_hour.and_then(|item| item.get("resets_at"))),
        weekly_remaining_percent: remaining_percent(
            weekly.and_then(|item| item.get("utilization")),
        ),
        weekly_reset_at: parse_reset_seconds(weekly.and_then(|item| item.get("resets_at"))),
        weekly_sonnet_remaining_percent: weekly_sonnet
            .and_then(|item| remaining_percent(item.get("utilization"))),
        weekly_sonnet_reset_at: parse_reset_seconds(
            weekly_sonnet.and_then(|item| item.get("resets_at")),
        ),
        extra_usage_remaining_percent: extra_enabled
            .then(|| extra_usage.and_then(|item| remaining_percent(item.get("utilization"))))
            .flatten(),
        extra_usage_reset_at: parse_reset_seconds(
            extra_usage.and_then(|item| item.get("resets_at")),
        ),
        extra_usage_used_cents: read_i64_value(
            extra_usage.and_then(|item| item.get("used_credits")),
        ),
        extra_usage_limit_cents: read_i64_value(
            extra_usage.and_then(|item| item.get("monthly_limit")),
        ),
    }
}

fn remaining_percent(value: Option<&Value>) -> Option<i32> {
    let used = read_f64_value(value)?;
    if !used.is_finite() {
        return None;
    }
    Some((100.0 - used.round()).clamp(0.0, 100.0) as i32)
}

fn parse_reset_seconds(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => {
            let raw = number
                .as_i64()
                .or_else(|| number.as_f64().map(|item| item as i64))?;
            if raw <= 0 {
                None
            } else if raw > 10_000_000_000 {
                Some(raw / 1000)
            } else {
                Some(raw)
            }
        }
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Ok(raw) = trimmed.parse::<i64>() {
                return if raw > 10_000_000_000 {
                    Some(raw / 1000)
                } else {
                    Some(raw)
                };
            }
            chrono::DateTime::parse_from_rfc3339(trimmed)
                .ok()
                .map(|item| item.timestamp())
        }
        _ => None,
    }
}

fn build_account_id(
    email: &str,
    account_uuid: Option<&str>,
    organization_uuid: Option<&str>,
) -> String {
    let identity = format!(
        "{}:{}:{}",
        email.trim().to_ascii_lowercase(),
        account_uuid.unwrap_or_default().trim(),
        organization_uuid.unwrap_or_default().trim()
    );
    format!("claude_{:x}", md5::compute(identity.as_bytes()))
}

fn subscription_type_from_profile(profile: Option<&Value>) -> Option<String> {
    match read_string_path(profile?, &["organization", "organization_type"])?.as_str() {
        "claude_max" => Some("Max".to_string()),
        "claude_pro" => Some("Pro".to_string()),
        "claude_enterprise" => Some("Enterprise".to_string()),
        "claude_team" => Some("Team".to_string()),
        _ => None,
    }
}

fn first_non_empty<const N: usize>(values: [Option<String>; N]) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .find_map(|value| normalize_optional(Some(value)))
}

fn read_string_path(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_str()
        .and_then(|value| normalize_optional(Some(value.to_string())))
}

fn read_i64_value(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|item| item as i64)),
        Some(Value::String(text)) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn read_f64_value(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

impl StoredClaudeAccount {
    fn to_summary(&self) -> ClaudeAccountSummary {
        ClaudeAccountSummary {
            id: self.id.clone(),
            email: self.email.clone(),
            auth_mode: self.auth_mode.clone(),
            account_uuid: self.account_uuid.clone(),
            organization_uuid: self.organization_uuid.clone(),
            organization_name: self.organization_name.clone(),
            display_name: self.display_name.clone(),
            avatar_url: self.avatar_url.clone(),
            plan_type: self.plan_type.clone(),
            quota: self.quota.clone(),
            quota_query_last_error: self.quota_query_last_error.clone(),
            quota_query_last_error_at: self.quota_query_last_error_at,
            usage_updated_at: self.usage_updated_at,
            profile_updated_at: self.profile_updated_at,
            created_at: self.created_at,
            last_used: self.last_used,
        }
    }
}
