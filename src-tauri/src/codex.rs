use base64::Engine;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const DATA_DIR: &str = ".quota";
const ACCOUNTS_DIR: &str = "codex_accounts";
const ACCOUNTS_INDEX_FILE: &str = "codex_accounts.json";
const CODEX_USAGE_ENDPOINT: &str = "https://chatgpt.com/backend-api/wham/usage";
const CODEX_OAUTH_AUTHORIZE_ENDPOINT: &str = "https://auth.openai.com/oauth/authorize";
const CODEX_OAUTH_TOKEN_ENDPOINT: &str = "https://auth.openai.com/oauth/token";
const CODEX_OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CODEX_OAUTH_SCOPES: &str =
    "openid profile email offline_access api.connectors.read api.connectors.invoke";
const CODEX_OAUTH_CALLBACK_PORT: u16 = 1455;
const CODEX_OAUTH_TIMEOUT_SECONDS: i64 = 300;

static PENDING_CODEX_OAUTH: std::sync::LazyLock<Arc<Mutex<Option<PendingCodexOAuth>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(None)));

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountIndex {
    pub version: String,
    pub account_ids: Vec<String>,
}

impl CodexAccountIndex {
    fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            account_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CodexAuthMode {
    OAuth,
    ApiKey,
}

impl CodexAuthMode {
    fn as_summary_value(&self) -> &'static str {
        match self {
            Self::OAuth => "oauth",
            Self::ApiKey => "apikey",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodexTokens {
    id_token: String,
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCodexAccount {
    id: String,
    email: String,
    auth_mode: CodexAuthMode,
    openai_api_key: Option<String>,
    api_base_url: Option<String>,
    user_id: Option<String>,
    plan: Option<String>,
    account_id: Option<String>,
    organization_id: Option<String>,
    tokens: Option<CodexTokens>,
    quota: CodexQuotaSummary,
    quota_query_last_error: Option<String>,
    quota_query_last_error_at: Option<i64>,
    usage_updated_at: Option<i64>,
    created_at: i64,
    last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAccountSummary {
    pub id: String,
    pub email: String,
    pub auth_mode: String,
    pub api_base_url: Option<String>,
    pub user_id: Option<String>,
    pub plan: Option<String>,
    pub account_id: Option<String>,
    pub organization_id: Option<String>,
    pub quota: CodexQuotaSummary,
    pub quota_query_last_error: Option<String>,
    pub quota_query_last_error_at: Option<i64>,
    pub usage_updated_at: Option<i64>,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexQuotaSummary {
    pub hourly_remaining_percent: Option<i32>,
    pub hourly_reset_at: Option<i64>,
    pub hourly_window_minutes: Option<i64>,
    pub weekly_remaining_percent: Option<i32>,
    pub weekly_reset_at: Option<i64>,
    pub weekly_window_minutes: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ParsedCodexQuota {
    pub plan: Option<String>,
    pub quota: CodexQuotaSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOAuthStartResponse {
    pub login_id: String,
    pub auth_url: String,
    pub callback_url: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
struct PendingCodexOAuth {
    login_id: String,
    redirect_uri: String,
    code_verifier: String,
    state: String,
    expires_at: i64,
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodexAuthFile {
    auth_mode: Option<String>,
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<serde_json::Value>,
    #[serde(default, alias = "api_base_url", alias = "apiBaseUrl")]
    base_url: Option<String>,
    tokens: Option<CodexAuthTokens>,
}

#[derive(Debug, Deserialize)]
struct CodexAuthTokens {
    id_token: String,
    access_token: String,
    refresh_token: Option<String>,
    account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodexTokenResponse {
    id_token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<serde_json::Value>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodexJwtPayload {
    email: Option<String>,
    sub: Option<String>,
    #[serde(rename = "https://api.openai.com/auth")]
    auth_data: Option<CodexJwtAuthData>,
    #[serde(rename = "https://api.openai.com/profile")]
    profile_data: Option<CodexJwtProfileData>,
}

#[derive(Debug, Deserialize)]
struct CodexJwtProfileData {
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodexJwtAuthData {
    chatgpt_user_id: Option<String>,
    user_id: Option<String>,
    chatgpt_plan_type: Option<String>,
    account_id: Option<String>,
    chatgpt_account_id: Option<String>,
    organization_id: Option<String>,
    chatgpt_organization_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageResponse {
    plan_type: Option<String>,
    rate_limit: Option<RateLimitInfo>,
}

#[derive(Debug, Deserialize)]
struct RateLimitInfo {
    primary_window: Option<WindowInfo>,
    secondary_window: Option<WindowInfo>,
}

#[derive(Debug, Deserialize)]
struct WindowInfo {
    used_percent: Option<i32>,
    limit_window_seconds: Option<i64>,
    reset_after_seconds: Option<i64>,
    reset_at: Option<i64>,
}

#[tauri::command]
pub fn list_codex_accounts() -> Result<Vec<CodexAccountSummary>, String> {
    list_accounts_in(&quota_storage_dir()?)
}

#[tauri::command]
pub fn import_codex_from_local() -> Result<CodexAccountSummary, String> {
    import_from_auth_dir(&codex_home(), &quota_storage_dir()?)
}

#[tauri::command]
pub fn codex_oauth_login_start() -> Result<CodexOAuthStartResponse, String> {
    let login_id = generate_base64url_token();
    let state = generate_base64url_token();
    let code_verifier = generate_base64url_token();
    let start = build_oauth_start(&login_id, &state, &code_verifier)?;
    start_callback_listener(CODEX_OAUTH_CALLBACK_PORT)?;
    set_pending_oauth(Some(PendingCodexOAuth {
        login_id: start.login_id.clone(),
        redirect_uri: start.callback_url.clone(),
        code_verifier,
        state,
        expires_at: start.expires_at,
        code: None,
    }));
    Ok(start)
}

#[tauri::command]
pub async fn codex_oauth_login_complete(login_id: String) -> Result<CodexAccountSummary, String> {
    let pending = pending_oauth_for(&login_id)?;
    if pending.expires_at <= now_timestamp() {
        set_pending_oauth(None);
        return Err("Codex OAuth login expired. Start again.".to_string());
    }
    let code = pending
        .code
        .clone()
        .ok_or_else(|| "Codex OAuth callback has not arrived yet.".to_string())?;
    let response = exchange_oauth_code(&pending, &code).await?;
    let summary = upsert_token_response_in(&quota_storage_dir()?, None, &response)?;
    set_pending_oauth(None);
    Ok(summary)
}

#[tauri::command]
pub fn codex_oauth_login_cancel(login_id: Option<String>) -> Result<(), String> {
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
    notify_callback_cancel(CODEX_OAUTH_CALLBACK_PORT);
    Ok(())
}

#[tauri::command]
pub async fn refresh_codex_account(account_id: String) -> Result<CodexAccountSummary, String> {
    refresh_account_in(&quota_storage_dir()?, &account_id).await
}

#[tauri::command]
pub async fn refresh_all_codex_accounts() -> Result<Vec<CodexAccountSummary>, String> {
    let storage_dir = quota_storage_dir()?;
    let account_ids = load_index_in(&storage_dir)?.account_ids;
    for account_id in account_ids {
        let _ = refresh_account_in(&storage_dir, &account_id).await;
    }
    list_accounts_in(&storage_dir)
}

#[tauri::command]
pub fn delete_codex_account(account_id: String) -> Result<(), String> {
    let storage_dir = quota_storage_dir()?;
    let mut index = load_index_in(&storage_dir)?;
    index.account_ids.retain(|id| id != &account_id);
    save_index_in(&storage_dir, &index)?;

    let path = account_path_in(&storage_dir, &account_id);
    if path.exists() {
        fs::remove_file(&path).map_err(|err| format!("Could not delete Codex account: {}", err))?;
    }
    Ok(())
}

pub fn import_codex_from_auth_dir_for_test(
    auth_dir: &Path,
    storage_dir: &Path,
) -> Result<CodexAccountSummary, String> {
    import_from_auth_dir(auth_dir, storage_dir)
}

pub fn parse_codex_quota_for_test(raw: &serde_json::Value) -> Result<ParsedCodexQuota, String> {
    parse_quota_from_value(raw)
}

pub fn apply_codex_token_response_for_test(
    storage_dir: &Path,
    account_id: &str,
    response: &serde_json::Value,
) -> Result<CodexAccountSummary, String> {
    upsert_token_response_in(storage_dir, Some(account_id), response)
}

pub fn build_codex_oauth_start_for_test(
    login_id: &str,
    state: &str,
    code_verifier: &str,
) -> Result<CodexOAuthStartResponse, String> {
    build_oauth_start(login_id, state, code_verifier)
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

fn codex_home() -> PathBuf {
    if let Some(from_env) = std::env::var("CODEX_HOME")
        .ok()
        .map(|raw| raw.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|raw| !raw.is_empty())
    {
        return PathBuf::from(from_env);
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
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

fn load_index_in(storage_dir: &Path) -> Result<CodexAccountIndex, String> {
    let path = index_path_in(storage_dir);
    if !path.exists() {
        return Ok(CodexAccountIndex::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|err| format!("Could not read Codex account index: {}", err))?;
    if content.trim().is_empty() {
        return Ok(CodexAccountIndex::new());
    }
    serde_json::from_str(&content)
        .map_err(|err| format!("Could not parse Codex account index: {}", err))
}

fn save_index_in(storage_dir: &Path, index: &CodexAccountIndex) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index)
        .map_err(|err| format!("Could not encode Codex account index: {}", err))?;
    write_string_atomic(&index_path_in(storage_dir), &content)
}

fn load_account_in(storage_dir: &Path, account_id: &str) -> Result<StoredCodexAccount, String> {
    let content = fs::read_to_string(account_path_in(storage_dir, account_id))
        .map_err(|err| format!("Could not read Codex account: {}", err))?;
    serde_json::from_str(&content).map_err(|err| format!("Could not parse Codex account: {}", err))
}

fn save_account_in(storage_dir: &Path, account: &StoredCodexAccount) -> Result<(), String> {
    let content = serde_json::to_string_pretty(account)
        .map_err(|err| format!("Could not encode Codex account: {}", err))?;
    write_string_atomic(&account_path_in(storage_dir, &account.id), &content)
}

fn list_accounts_in(storage_dir: &Path) -> Result<Vec<CodexAccountSummary>, String> {
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
) -> Result<CodexAccountSummary, String> {
    let auth_path = auth_dir.join("auth.json");
    if !auth_path.exists() {
        return Err(format!(
            "Could not find Codex auth file: {}",
            auth_path.display()
        ));
    }

    let content = fs::read_to_string(&auth_path)
        .map_err(|err| format!("Could not read Codex auth file: {}", err))?;
    let auth_file: CodexAuthFile = serde_json::from_str(&content)
        .map_err(|err| format!("Could not parse Codex auth file: {}", err))?;

    let account =
        if is_auth_mode_apikey(auth_file.auth_mode.as_deref()) || auth_file.tokens.is_none() {
            let api_key = extract_api_key(&auth_file)
                .ok_or_else(|| "Codex auth file has no importable credentials".to_string())?;
            build_api_key_account(&api_key, auth_file.base_url)
        } else {
            let tokens = auth_file
                .tokens
                .ok_or_else(|| "Codex auth file has no tokens".to_string())?;
            build_oauth_account(tokens)?
        };

    upsert_account_in(storage_dir, account)
}

fn is_auth_mode_apikey(value: Option<&str>) -> bool {
    value
        .map(|item| item.trim().eq_ignore_ascii_case("apikey"))
        .unwrap_or(false)
}

fn extract_api_key(auth_file: &CodexAuthFile) -> Option<String> {
    auth_file
        .openai_api_key
        .as_ref()
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn build_api_key_account(api_key: &str, api_base_url: Option<String>) -> StoredCodexAccount {
    let now = now_timestamp();
    let digest = format!("{:x}", md5::compute(api_key.as_bytes()));
    StoredCodexAccount {
        id: format!("codex_apikey_{}", digest),
        email: format!("api-key-{}", &digest[..8]),
        auth_mode: CodexAuthMode::ApiKey,
        openai_api_key: Some(api_key.to_string()),
        api_base_url: normalize_optional(api_base_url),
        user_id: None,
        plan: Some("API Key".to_string()),
        account_id: None,
        organization_id: None,
        tokens: None,
        quota: CodexQuotaSummary::default(),
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        usage_updated_at: None,
        created_at: now,
        last_used: now,
    }
}

fn build_oauth_account(tokens: CodexAuthTokens) -> Result<StoredCodexAccount, String> {
    let payload = decode_jwt_payload(&tokens.id_token)?;
    let auth_data = payload.auth_data;
    let email = normalize_optional(payload.email)
        .or_else(|| {
            payload
                .profile_data
                .and_then(|profile| normalize_optional(profile.email))
        })
        .ok_or_else(|| "Codex id_token does not include an email".to_string())?;
    let user_id = auth_data
        .as_ref()
        .and_then(|data| {
            normalize_optional(data.chatgpt_user_id.clone())
                .or_else(|| normalize_optional(data.user_id.clone()))
        })
        .or_else(|| normalize_optional(payload.sub));
    let plan = auth_data
        .as_ref()
        .and_then(|data| normalize_optional(data.chatgpt_plan_type.clone()));
    let account_id = auth_data
        .as_ref()
        .and_then(|data| {
            normalize_optional(data.account_id.clone())
                .or_else(|| normalize_optional(data.chatgpt_account_id.clone()))
        })
        .or_else(|| normalize_optional(tokens.account_id.clone()));
    let organization_id = auth_data.as_ref().and_then(|data| {
        normalize_optional(data.organization_id.clone())
            .or_else(|| normalize_optional(data.chatgpt_organization_id.clone()))
    });
    let now = now_timestamp();
    let id = build_oauth_account_id(&email, account_id.as_deref(), organization_id.as_deref());

    Ok(StoredCodexAccount {
        id,
        email,
        auth_mode: CodexAuthMode::OAuth,
        openai_api_key: None,
        api_base_url: None,
        user_id,
        plan,
        account_id,
        organization_id,
        tokens: Some(CodexTokens {
            id_token: tokens.id_token,
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
        }),
        quota: CodexQuotaSummary::default(),
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        usage_updated_at: None,
        created_at: now,
        last_used: now,
    })
}

fn build_oauth_start(
    login_id: &str,
    state: &str,
    code_verifier: &str,
) -> Result<CodexOAuthStartResponse, String> {
    let redirect_uri = format!(
        "http://localhost:{}/auth/callback",
        CODEX_OAUTH_CALLBACK_PORT
    );
    let code_challenge = pkce_challenge(code_verifier);
    let auth_url = format!(
        "{}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256&originator=codex_vscode",
        CODEX_OAUTH_AUTHORIZE_ENDPOINT,
        urlencoding::encode(CODEX_OAUTH_CLIENT_ID),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(CODEX_OAUTH_SCOPES),
        urlencoding::encode(state),
        urlencoding::encode(&code_challenge)
    );

    Ok(CodexOAuthStartResponse {
        login_id: login_id.to_string(),
        auth_url,
        callback_url: redirect_uri,
        expires_at: now_timestamp() + CODEX_OAUTH_TIMEOUT_SECONDS,
    })
}

fn pkce_challenge(code_verifier: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn pending_oauth() -> Option<PendingCodexOAuth> {
    PENDING_CODEX_OAUTH
        .lock()
        .ok()
        .and_then(|state| state.clone())
}

fn set_pending_oauth(pending: Option<PendingCodexOAuth>) {
    if let Ok(mut state) = PENDING_CODEX_OAUTH.lock() {
        *state = pending;
    }
}

fn pending_oauth_for(login_id: &str) -> Result<PendingCodexOAuth, String> {
    let pending = pending_oauth()
        .ok_or_else(|| "Codex OAuth login was cancelled. Start again.".to_string())?;
    if pending.login_id != login_id {
        return Err("Codex OAuth login session changed. Start again.".to_string());
    }
    Ok(pending)
}

fn start_callback_listener(port: u16) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|err| {
        format!(
            "Could not start Codex OAuth callback on port {}: {}",
            port, err
        )
    })?;
    std::thread::spawn(move || {
        for stream in listener.incoming().take(1) {
            if let Ok(mut stream) = stream {
                handle_callback_stream(&mut stream);
            }
        }
    });
    Ok(())
}

fn handle_callback_stream(stream: &mut TcpStream) {
    let mut buffer = [0_u8; 4096];
    let read_len = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read_len]);
    let first_line = request.lines().next().unwrap_or_default();
    let path = first_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_string();
    let result = capture_callback_path(&path);
    let body = if result.is_ok() {
        "Codex connected. You can return to Quota."
    } else {
        "Codex connection failed. Return to Quota and try again."
    };
    let _ = stream.write_all(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .as_bytes(),
    );
    let _ = stream.flush();
}

fn capture_callback_path(path: &str) -> Result<(), String> {
    if path.starts_with("/cancel") {
        return Ok(());
    }

    let query = path
        .split_once('?')
        .map(|(_, query)| query)
        .ok_or_else(|| "Codex OAuth callback did not include query parameters.".to_string())?;
    let params = parse_query_params(query);
    let code = params
        .get("code")
        .and_then(|value| normalize_optional(Some(value.clone())))
        .ok_or_else(|| "Codex OAuth callback did not include a code.".to_string())?;
    let state = params
        .get("state")
        .and_then(|value| normalize_optional(Some(value.clone())))
        .ok_or_else(|| "Codex OAuth callback did not include state.".to_string())?;

    let mut pending = PENDING_CODEX_OAUTH
        .lock()
        .map_err(|_| "Codex OAuth state lock failed.".to_string())?;
    let current = pending
        .as_mut()
        .ok_or_else(|| "Codex OAuth login was cancelled. Start again.".to_string())?;
    if current.expires_at <= now_timestamp() {
        *pending = None;
        return Err("Codex OAuth login expired. Start again.".to_string());
    }
    if current.state != state {
        return Err("Codex OAuth state mismatch. Start again.".to_string());
    }
    current.code = Some(code);
    Ok(())
}

fn notify_callback_cancel(port: u16) {
    if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
        let _ = stream
            .write_all(b"GET /cancel HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
        let _ = stream.flush();
    }
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

async fn exchange_oauth_code(
    pending: &PendingCodexOAuth,
    code: &str,
) -> Result<serde_json::Value, String> {
    let response = reqwest::Client::new()
        .post(CODEX_OAUTH_TOKEN_ENDPOINT)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", CODEX_OAUTH_CLIENT_ID),
            ("code", code),
            ("redirect_uri", pending.redirect_uri.as_str()),
            ("code_verifier", pending.code_verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|err| format!("Codex OAuth token request failed: {}", err))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("Could not read Codex OAuth token response: {}", err))?;
    if !status.is_success() {
        return Err(format!(
            "Codex OAuth token exchange returned {} with body length {}",
            status,
            body.len()
        ));
    }
    serde_json::from_str(&body)
        .map_err(|err| format!("Could not parse Codex OAuth token response: {}", err))
}

fn upsert_account_in(
    storage_dir: &Path,
    mut account: StoredCodexAccount,
) -> Result<CodexAccountSummary, String> {
    let mut index = load_index_in(storage_dir)?;
    let existing = load_account_in(storage_dir, &account.id).ok();
    if let Some(existing) = existing {
        account.created_at = existing.created_at;
    }
    account.last_used = now_timestamp();
    save_account_in(storage_dir, &account)?;
    if !index.account_ids.iter().any(|id| id == &account.id) {
        index.account_ids.insert(0, account.id.clone());
    }
    save_index_in(storage_dir, &index)?;
    Ok(account.to_summary())
}

fn upsert_token_response_in(
    storage_dir: &Path,
    existing_account_id: Option<&str>,
    response: &serde_json::Value,
) -> Result<CodexAccountSummary, String> {
    let token_response: CodexTokenResponse = serde_json::from_value(response.clone())
        .map_err(|err| format!("Could not parse Codex token response: {}", err))?;
    if let Some(error) = token_response.error {
        let description = token_response
            .error_description
            .unwrap_or_else(|| error.to_string());
        return Err(format!("Codex token response error: {}", description));
    }

    let id_token = token_response
        .id_token
        .and_then(|value| normalize_optional(Some(value)))
        .ok_or_else(|| "Codex token response did not include an id_token".to_string())?;
    let access_token = token_response
        .access_token
        .and_then(|value| normalize_optional(Some(value)))
        .ok_or_else(|| "Codex token response did not include an access_token".to_string())?;
    let refresh_token = token_response
        .refresh_token
        .and_then(|value| normalize_optional(Some(value)))
        .or_else(|| {
            existing_account_id
                .and_then(|account_id| load_account_in(storage_dir, account_id).ok())
                .and_then(|account| account.tokens.and_then(|tokens| tokens.refresh_token))
        });

    let account = build_oauth_account(CodexAuthTokens {
        id_token,
        access_token,
        refresh_token,
        account_id: None,
    })?;

    let account = if let Some(existing_account_id) = existing_account_id {
        let existing = load_account_in(storage_dir, existing_account_id).ok();
        if existing.as_ref().map(|existing| existing.id.as_str()) == Some(account.id.as_str()) {
            account
        } else {
            account
        }
    } else {
        account
    };

    upsert_account_in(storage_dir, account)
}

async fn refresh_account_in(
    storage_dir: &Path,
    account_id: &str,
) -> Result<CodexAccountSummary, String> {
    let mut account = load_account_in(storage_dir, account_id)?;
    if matches!(account.auth_mode, CodexAuthMode::ApiKey) {
        account.quota_query_last_error =
            Some("API key accounts do not expose Codex web quota in this slice.".to_string());
        account.quota_query_last_error_at = Some(now_timestamp());
        save_account_in(storage_dir, &account)?;
        return Ok(account.to_summary());
    }

    match fetch_quota(&account).await {
        Ok(parsed) => {
            account.plan = parsed.plan.or(account.plan);
            account.quota = parsed.quota;
            account.quota_query_last_error = None;
            account.quota_query_last_error_at = None;
            account.usage_updated_at = Some(now_timestamp());
            account.last_used = now_timestamp();
            save_account_in(storage_dir, &account)?;
            Ok(account.to_summary())
        }
        Err(CodexQuotaFetchError::Unauthorized(_error)) => {
            account = refresh_account_tokens(storage_dir, account).await?;
            match fetch_quota(&account).await {
                Ok(parsed) => {
                    account.plan = parsed.plan.or(account.plan);
                    account.quota = parsed.quota;
                    account.quota_query_last_error = None;
                    account.quota_query_last_error_at = None;
                    account.usage_updated_at = Some(now_timestamp());
                    account.last_used = now_timestamp();
                    save_account_in(storage_dir, &account)?;
                    Ok(account.to_summary())
                }
                Err(error) => {
                    let message = error.to_string();
                    account.quota_query_last_error = Some(message.clone());
                    account.quota_query_last_error_at = Some(now_timestamp());
                    save_account_in(storage_dir, &account)?;
                    Err(message)
                }
            }
        }
        Err(error) => {
            let message = error.to_string();
            account.quota_query_last_error = Some(message.clone());
            account.quota_query_last_error_at = Some(now_timestamp());
            save_account_in(storage_dir, &account)?;
            Err(message)
        }
    }
}

#[derive(Debug)]
enum CodexQuotaFetchError {
    Unauthorized(String),
    Other(String),
}

impl std::fmt::Display for CodexQuotaFetchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unauthorized(message) | Self::Other(message) => formatter.write_str(message),
        }
    }
}

async fn fetch_quota(
    account: &StoredCodexAccount,
) -> Result<ParsedCodexQuota, CodexQuotaFetchError> {
    let token = account
        .tokens
        .as_ref()
        .map(|tokens| tokens.access_token.trim())
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            CodexQuotaFetchError::Other(
                "Codex account does not have an OAuth access token".to_string(),
            )
        })?;

    let client = reqwest::Client::new();
    let mut request = client
        .get(CODEX_USAGE_ENDPOINT)
        .header(ACCEPT, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", token));

    if let Some(account_id) = account
        .account_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request.send().await.map_err(|err| {
        CodexQuotaFetchError::Other(format!("Codex quota request failed: {}", err))
    })?;
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        CodexQuotaFetchError::Other(format!("Could not read Codex quota response: {}", err))
    })?;

    if !status.is_success() {
        let message = format!(
            "Codex quota API returned {} with body length {}",
            status,
            body.len()
        );
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(CodexQuotaFetchError::Unauthorized(message));
        }
        return Err(CodexQuotaFetchError::Other(message));
    }

    let value: serde_json::Value = serde_json::from_str(&body).map_err(|err| {
        CodexQuotaFetchError::Other(format!("Could not parse Codex quota JSON: {}", err))
    })?;
    parse_quota_from_value(&value).map_err(CodexQuotaFetchError::Other)
}

async fn refresh_account_tokens(
    storage_dir: &Path,
    account: StoredCodexAccount,
) -> Result<StoredCodexAccount, String> {
    let refresh_token = account
        .tokens
        .as_ref()
        .and_then(|tokens| normalize_optional(tokens.refresh_token.clone()))
        .ok_or_else(|| "Codex account has no refresh token. Reconnect this account.".to_string())?;
    let response = request_token_refresh(&refresh_token).await?;
    let summary = upsert_token_response_in(storage_dir, Some(&account.id), &response)?;
    load_account_in(storage_dir, &summary.id)
}

async fn request_token_refresh(refresh_token: &str) -> Result<serde_json::Value, String> {
    let response = reqwest::Client::new()
        .post(CODEX_OAUTH_TOKEN_ENDPOINT)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CODEX_OAUTH_CLIENT_ID),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|err| format!("Codex token refresh request failed: {}", err))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("Could not read Codex token refresh response: {}", err))?;
    if !status.is_success() {
        return Err(format!(
            "Codex token refresh returned {} with body length {}",
            status,
            body.len()
        ));
    }
    serde_json::from_str(&body)
        .map_err(|err| format!("Could not parse Codex token response: {}", err))
}

fn parse_quota_from_value(value: &serde_json::Value) -> Result<ParsedCodexQuota, String> {
    let usage: UsageResponse = serde_json::from_value(value.clone())
        .map_err(|err| format!("Could not parse Codex usage payload: {}", err))?;
    let primary = usage
        .rate_limit
        .as_ref()
        .and_then(|limit| limit.primary_window.as_ref());
    let secondary = usage
        .rate_limit
        .as_ref()
        .and_then(|limit| limit.secondary_window.as_ref());
    Ok(ParsedCodexQuota {
        plan: normalize_optional(usage.plan_type),
        quota: CodexQuotaSummary {
            hourly_remaining_percent: primary.map(remaining_percent),
            hourly_reset_at: primary.and_then(reset_at),
            hourly_window_minutes: primary.and_then(window_minutes),
            weekly_remaining_percent: secondary.map(remaining_percent),
            weekly_reset_at: secondary.and_then(reset_at),
            weekly_window_minutes: secondary.and_then(window_minutes),
        },
    })
}

fn remaining_percent(window: &WindowInfo) -> i32 {
    100 - window.used_percent.unwrap_or(0).clamp(0, 100)
}

fn window_minutes(window: &WindowInfo) -> Option<i64> {
    let seconds = window.limit_window_seconds?;
    if seconds <= 0 {
        return None;
    }
    Some((seconds + 59) / 60)
}

fn reset_at(window: &WindowInfo) -> Option<i64> {
    if let Some(value) = window.reset_at {
        return Some(value);
    }
    let seconds = window.reset_after_seconds?;
    if seconds < 0 {
        return None;
    }
    Some(now_timestamp() + seconds)
}

fn decode_jwt_payload(token: &str) -> Result<CodexJwtPayload, String> {
    let payload = decode_jwt_payload_value(token)?;
    serde_json::from_value(payload)
        .map_err(|err| format!("Could not parse Codex JWT payload: {}", err))
}

fn decode_jwt_payload_value(token: &str) -> Result<serde_json::Value, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return Err("Invalid Codex JWT token format".to_string());
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|err| format!("Could not decode Codex JWT payload: {}", err))?;
    serde_json::from_slice(&bytes).map_err(|err| format!("Could not parse Codex JWT JSON: {}", err))
}

fn build_oauth_account_id(
    email: &str,
    account_id: Option<&str>,
    organization_id: Option<&str>,
) -> String {
    let mut seed = email.trim().to_string();
    if let Some(value) = account_id.and_then(|value| normalize_optional(Some(value.to_string()))) {
        seed.push('|');
        seed.push_str(&value);
    }
    if let Some(value) =
        organization_id.and_then(|value| normalize_optional(Some(value.to_string())))
    {
        seed.push('|');
        seed.push_str(&value);
    }
    format!("codex_{:x}", md5::compute(seed.as_bytes()))
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

impl StoredCodexAccount {
    fn to_summary(&self) -> CodexAccountSummary {
        CodexAccountSummary {
            id: self.id.clone(),
            email: self.email.clone(),
            auth_mode: self.auth_mode.as_summary_value().to_string(),
            api_base_url: self.api_base_url.clone(),
            user_id: self.user_id.clone(),
            plan: self.plan.clone(),
            account_id: self.account_id.clone(),
            organization_id: self.organization_id.clone(),
            quota: self.quota.clone(),
            quota_query_last_error: self.quota_query_last_error.clone(),
            quota_query_last_error_at: self.quota_query_last_error_at,
            usage_updated_at: self.usage_updated_at,
            created_at: self.created_at,
            last_used: self.last_used,
        }
    }
}
