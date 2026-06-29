use base64::Engine;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const DATA_DIR: &str = ".quota";
const ACCOUNTS_DIR: &str = "antigravity_accounts";
const ACCOUNTS_INDEX_FILE: &str = "antigravity_accounts.json";
const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_ENDPOINT: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const ANTIGRAVITY_OAUTH_CLIENT_ID: &str =
    "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
const ANTIGRAVITY_OAUTH_CLIENT_SECRET: &str = "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";
const CODE_ASSIST_BASE_ENDPOINT: &str = "https://daily-cloudcode-pa.googleapis.com";
const CODE_ASSIST_LOAD_ENDPOINT: &str = "v1internal:loadCodeAssist";
const CODE_ASSIST_FETCH_MODELS_ENDPOINT: &str = "v1internal:fetchAvailableModels";
const CODE_ASSIST_RETRIEVE_QUOTA_ENDPOINT: &str = "v1internal:retrieveUserQuotaSummary";
const ANTIGRAVITY_OAUTH_AUTHORIZE_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const ANTIGRAVITY_OAUTH_CALLBACK_PATH: &str = "/oauth-callback";
const ANTIGRAVITY_OAUTH_TIMEOUT_SECONDS: i64 = 300;
const ANTIGRAVITY_OAUTH_SCOPES: [&str; 5] = [
    "https://www.googleapis.com/auth/cloud-platform",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/userinfo.profile",
    "https://www.googleapis.com/auth/cclog",
    "https://www.googleapis.com/auth/experimentsandconfigs",
];
const ANTIGRAVITY_IDE_VERSION: &str = "1.20.5";
const ANTIGRAVITY_GOOGLE_API_NODEJS_CLIENT_VERSION: &str = "10.3.0";
const ANTIGRAVITY_X_GOOG_API_CLIENT: &str = "gl-node/22.21.1";

static PENDING_ANTIGRAVITY_OAUTH: std::sync::LazyLock<Arc<Mutex<Option<PendingAntigravityOAuth>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(None)));

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityAccountIndex {
    pub version: String,
    pub account_ids: Vec<String>,
}

impl AntigravityAccountIndex {
    fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            account_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAntigravityAccount {
    id: String,
    email: String,
    #[serde(default = "default_local_source")]
    source: String,
    auth_id: Option<String>,
    name: Option<String>,
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    expiry_date: Option<i64>,
    selected_auth_type: Option<String>,
    project_id: Option<String>,
    tier_id: Option<String>,
    plan_name: Option<String>,
    #[serde(default)]
    credits: Vec<AntigravityCreditInfo>,
    quota: AntigravityQuotaSummary,
    quota_query_last_error: Option<String>,
    quota_query_last_error_at: Option<i64>,
    usage_updated_at: Option<i64>,
    status: Option<String>,
    status_reason: Option<String>,
    created_at: i64,
    last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityAccountSummary {
    pub id: String,
    pub email: String,
    pub auth_id: Option<String>,
    pub name: Option<String>,
    pub source: String,
    pub selected_auth_type: Option<String>,
    pub project_id: Option<String>,
    pub tier_id: Option<String>,
    pub plan_name: Option<String>,
    pub credits: Vec<AntigravityCreditInfo>,
    pub quota: AntigravityQuotaSummary,
    pub quota_query_last_error: Option<String>,
    pub quota_query_last_error_at: Option<i64>,
    pub usage_updated_at: Option<i64>,
    pub status: Option<String>,
    pub status_reason: Option<String>,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityQuotaSummary {
    pub gemini_five_hour: AntigravityQuotaWindow,
    pub gemini_weekly: AntigravityQuotaWindow,
    pub third_party_five_hour: AntigravityQuotaWindow,
    pub third_party_weekly: AntigravityQuotaWindow,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityQuotaWindow {
    pub remaining_percent: Option<i32>,
    pub reset_at: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityCreditInfo {
    pub credit_type: String,
    pub credit_amount: Option<String>,
    pub minimum_credit_amount_for_usage: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityOAuthStartResponse {
    pub login_id: String,
    pub auth_url: String,
    pub callback_url: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
struct PendingAntigravityOAuth {
    login_id: String,
    callback_url: String,
    state: String,
    expires_at: i64,
    callback_port: u16,
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LocalOAuthCreds {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    expiry_date: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct JwtPayload {
    email: Option<String>,
    sub: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    email: Option<String>,
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenRefreshResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    expires_in: Option<i64>,
    error: Option<Value>,
    error_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeAssistStatus {
    pub tier_id: Option<String>,
    pub tier_name: Option<String>,
    pub project_id: Option<String>,
    pub credits: Vec<AntigravityCreditInfo>,
}

struct CallbackListener {
    port: u16,
    listener: TcpListener,
}

#[tauri::command]
pub fn list_antigravity_accounts() -> Result<Vec<AntigravityAccountSummary>, String> {
    list_accounts_in(&quota_storage_dir()?)
}

#[tauri::command]
pub fn import_antigravity_from_local() -> Result<AntigravityAccountSummary, String> {
    import_from_gemini_home(&gemini_home(), &quota_storage_dir()?)
}

#[tauri::command]
pub fn antigravity_oauth_login_start() -> Result<AntigravityOAuthStartResponse, String> {
    let login_id = generate_base64url_token();
    let state = generate_base64url_token();
    let callback = start_callback_listener()?;
    let start = build_oauth_start(&login_id, &state, callback.port)?;
    set_pending_oauth(Some(PendingAntigravityOAuth {
        login_id: start.login_id.clone(),
        callback_url: start.callback_url.clone(),
        state,
        expires_at: start.expires_at,
        callback_port: callback.port,
        code: None,
    }));
    spawn_callback_listener(callback.listener);
    Ok(start)
}

#[tauri::command]
pub async fn antigravity_oauth_login_complete(
    login_id: String,
) -> Result<AntigravityAccountSummary, String> {
    let pending = pending_oauth_for(&login_id)?;
    if pending.expires_at <= now_timestamp() {
        set_pending_oauth(None);
        return Err("Antigravity OAuth login expired. Start again.".to_string());
    }
    let code = pending
        .code
        .clone()
        .ok_or_else(|| "Antigravity OAuth callback has not arrived yet.".to_string())?;
    let response = exchange_oauth_code(&pending, &code).await?;
    let access_token = response
        .get("access_token")
        .and_then(Value::as_str)
        .and_then(|value| normalize_optional(value.to_string()));
    let profile = match access_token {
        Some(token) => fetch_google_userinfo(&token).await,
        None => None,
    };
    let summary = upsert_token_response_in(&quota_storage_dir()?, &response, profile)?;
    set_pending_oauth(None);
    Ok(summary)
}

#[tauri::command]
pub fn antigravity_oauth_login_cancel(login_id: Option<String>) -> Result<(), String> {
    let pending = pending_oauth();
    if let Some(login_id) = login_id {
        if pending.as_ref().map(|pending| pending.login_id.as_str()) == Some(login_id.as_str()) {
            if let Some(pending) = pending {
                notify_callback_cancel(pending.callback_port);
            }
            set_pending_oauth(None);
        }
    } else {
        if let Some(pending) = pending {
            notify_callback_cancel(pending.callback_port);
        }
        set_pending_oauth(None);
    }
    Ok(())
}

#[tauri::command]
pub async fn refresh_antigravity_account(
    account_id: String,
) -> Result<AntigravityAccountSummary, String> {
    refresh_account_in(&quota_storage_dir()?, &account_id).await
}

#[tauri::command]
pub async fn refresh_all_antigravity_accounts() -> Result<Vec<AntigravityAccountSummary>, String> {
    let storage_dir = quota_storage_dir()?;
    let account_ids = load_index_in(&storage_dir)?.account_ids;
    for account_id in account_ids {
        let _ = refresh_account_in(&storage_dir, &account_id).await;
    }
    list_accounts_in(&storage_dir)
}

#[tauri::command]
pub fn delete_antigravity_account(account_id: String) -> Result<(), String> {
    let storage_dir = quota_storage_dir()?;
    let mut index = load_index_in(&storage_dir)?;
    index.account_ids.retain(|id| id != &account_id);
    save_index_in(&storage_dir, &index)?;

    let path = account_path_in(&storage_dir, &account_id);
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("Could not delete Antigravity account: {}", err))?;
    }
    Ok(())
}

pub fn import_antigravity_from_gemini_home_for_test(
    gemini_home: &Path,
    storage_dir: &Path,
) -> Result<AntigravityAccountSummary, String> {
    import_from_gemini_home(gemini_home, storage_dir)
}

pub fn parse_antigravity_quota_for_test(raw: &Value) -> AntigravityQuotaSummary {
    parse_quota_from_value(raw)
}

pub fn parse_antigravity_code_assist_response_for_test(
    endpoint: &str,
    status: u16,
    text: &str,
) -> Result<Value, String> {
    parse_code_assist_response_text(endpoint, status, text)
}

pub fn build_antigravity_code_assist_headers_for_test(endpoint: &str) -> Vec<(String, String)> {
    build_code_assist_headers(endpoint)
}

pub fn build_antigravity_load_code_assist_payload_for_test() -> Value {
    build_load_code_assist_payload()
}

pub fn parse_antigravity_load_status_for_test(raw: &Value) -> CodeAssistStatus {
    parse_load_code_assist_status(raw)
}

pub fn build_antigravity_oauth_start_for_test(
    login_id: &str,
    state: &str,
) -> Result<AntigravityOAuthStartResponse, String> {
    build_oauth_start(login_id, state, 1466)
}

pub fn apply_antigravity_token_response_for_test(
    storage_dir: &Path,
    response: &Value,
) -> Result<AntigravityAccountSummary, String> {
    upsert_token_response_in(storage_dir, response, None)
}

pub fn record_antigravity_refresh_error_for_test(
    storage_dir: &Path,
    account_id: &str,
    error: &str,
) -> Result<AntigravityAccountSummary, String> {
    let account = load_account_in(storage_dir, account_id)?;
    record_refresh_error_in(storage_dir, account, error.to_string())
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

fn gemini_home() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".gemini")
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

fn load_index_in(storage_dir: &Path) -> Result<AntigravityAccountIndex, String> {
    let path = index_path_in(storage_dir);
    if !path.exists() {
        return Ok(AntigravityAccountIndex::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|err| format!("Could not read Antigravity account index: {}", err))?;
    if content.trim().is_empty() {
        return Ok(AntigravityAccountIndex::new());
    }
    serde_json::from_str(&content)
        .map_err(|err| format!("Could not parse Antigravity account index: {}", err))
}

fn save_index_in(storage_dir: &Path, index: &AntigravityAccountIndex) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index)
        .map_err(|err| format!("Could not encode Antigravity account index: {}", err))?;
    write_string_atomic(&index_path_in(storage_dir), &content)
}

fn load_account_in(
    storage_dir: &Path,
    account_id: &str,
) -> Result<StoredAntigravityAccount, String> {
    let content = fs::read_to_string(account_path_in(storage_dir, account_id))
        .map_err(|err| format!("Could not read Antigravity account: {}", err))?;
    serde_json::from_str(&content)
        .map_err(|err| format!("Could not parse Antigravity account: {}", err))
}

fn save_account_in(storage_dir: &Path, account: &StoredAntigravityAccount) -> Result<(), String> {
    let content = serde_json::to_string_pretty(account)
        .map_err(|err| format!("Could not encode Antigravity account: {}", err))?;
    write_string_atomic(&account_path_in(storage_dir, &account.id), &content)
}

fn list_accounts_in(storage_dir: &Path) -> Result<Vec<AntigravityAccountSummary>, String> {
    let index = load_index_in(storage_dir)?;
    Ok(index
        .account_ids
        .iter()
        .filter_map(|account_id| load_account_in(storage_dir, account_id).ok())
        .map(|account| account.to_summary())
        .collect())
}

fn import_from_gemini_home(
    gemini_home: &Path,
    storage_dir: &Path,
) -> Result<AntigravityAccountSummary, String> {
    let creds_path = gemini_home.join("oauth_creds.json");
    if !creds_path.exists() {
        return Err(format!(
            "Could not find Antigravity/Gemini credentials: {}",
            creds_path.display()
        ));
    }

    let content = fs::read_to_string(&creds_path)
        .map_err(|err| format!("Could not read Antigravity credentials: {}", err))?;
    let creds: LocalOAuthCreds = serde_json::from_str(&content)
        .map_err(|err| format!("Could not parse Antigravity credentials: {}", err))?;

    let jwt = creds
        .id_token
        .as_deref()
        .and_then(|token| decode_jwt_payload(token).ok());
    let active_email = read_active_google_email(gemini_home);
    let selected_auth_type = read_selected_auth_type(gemini_home);
    let email = active_email
        .or_else(|| {
            jwt.as_ref()
                .and_then(|payload| payload.email.clone().and_then(normalize_optional))
        })
        .unwrap_or_else(|| "unknown@gmail.com".to_string());
    let auth_id = jwt
        .as_ref()
        .and_then(|payload| payload.sub.clone().and_then(normalize_optional));
    let name = jwt
        .as_ref()
        .and_then(|payload| payload.name.clone().and_then(normalize_optional));
    let now = now_timestamp();
    let id = build_account_id(&email, auth_id.as_deref());

    let account = StoredAntigravityAccount {
        id,
        email,
        source: "local".to_string(),
        auth_id,
        name,
        access_token: creds.access_token,
        refresh_token: creds.refresh_token,
        id_token: creds.id_token,
        token_type: creds.token_type,
        scope: creds.scope,
        expiry_date: creds.expiry_date,
        selected_auth_type: selected_auth_type.or_else(|| Some("oauth-personal".to_string())),
        project_id: None,
        tier_id: None,
        plan_name: None,
        credits: Vec::new(),
        quota: AntigravityQuotaSummary::default(),
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        usage_updated_at: None,
        status: None,
        status_reason: None,
        created_at: now,
        last_used: now,
    };

    upsert_account_in(storage_dir, account)
}

fn upsert_account_in(
    storage_dir: &Path,
    mut account: StoredAntigravityAccount,
) -> Result<AntigravityAccountSummary, String> {
    let mut index = load_index_in(storage_dir)?;
    if let Ok(existing) = load_account_in(storage_dir, &account.id) {
        account.created_at = existing.created_at;
        if account.project_id.is_none() {
            account.project_id = existing.project_id;
        }
        if account.tier_id.is_none() {
            account.tier_id = existing.tier_id;
        }
        if account.plan_name.is_none() {
            account.plan_name = existing.plan_name;
        }
        if account.credits.is_empty() {
            account.credits = existing.credits;
        }
        if account.quota == AntigravityQuotaSummary::default() {
            account.quota = existing.quota;
        }
        account.usage_updated_at = existing.usage_updated_at;
    }

    if !index.account_ids.iter().any(|id| id == &account.id) {
        index.account_ids.insert(0, account.id.clone());
    }

    save_account_in(storage_dir, &account)?;
    save_index_in(storage_dir, &index)?;
    Ok(account.to_summary())
}

fn upsert_token_response_in(
    storage_dir: &Path,
    response: &Value,
    profile: Option<GoogleUserInfo>,
) -> Result<AntigravityAccountSummary, String> {
    let token_response: TokenRefreshResponse = serde_json::from_value(response.clone())
        .map_err(|err| format!("Could not parse Antigravity token response: {}", err))?;
    if token_response.error.is_some() {
        return Err(token_response.error_description.unwrap_or_else(|| {
            format!(
                "Antigravity token response error: {:?}",
                token_response.error
            )
        }));
    }

    let access_token = token_response
        .access_token
        .and_then(normalize_optional)
        .ok_or_else(|| "Antigravity token response did not include an access token.".to_string())?;
    let id_token = token_response.id_token.and_then(normalize_optional);
    let jwt = id_token
        .as_deref()
        .and_then(|token| decode_jwt_payload(token).ok());
    let profile_email = profile
        .as_ref()
        .and_then(|profile| profile.email.clone().and_then(normalize_optional));
    let profile_id = profile
        .as_ref()
        .and_then(|profile| profile.id.clone().and_then(normalize_optional));
    let profile_name = profile
        .as_ref()
        .and_then(|profile| profile.name.clone().and_then(normalize_optional));
    let email = profile_email
        .or_else(|| {
            jwt.as_ref()
                .and_then(|payload| payload.email.clone().and_then(normalize_optional))
        })
        .unwrap_or_else(|| "unknown@gmail.com".to_string());
    let auth_id = profile_id.or_else(|| {
        jwt.as_ref()
            .and_then(|payload| payload.sub.clone().and_then(normalize_optional))
    });
    let name = profile_name.or_else(|| {
        jwt.as_ref()
            .and_then(|payload| payload.name.clone().and_then(normalize_optional))
    });
    let now = now_timestamp();
    let account = StoredAntigravityAccount {
        id: build_account_id(&email, auth_id.as_deref()),
        email,
        source: "oauth".to_string(),
        auth_id,
        name,
        access_token,
        refresh_token: token_response.refresh_token.and_then(normalize_optional),
        id_token,
        token_type: token_response.token_type.and_then(normalize_optional),
        scope: token_response.scope.and_then(normalize_optional),
        expiry_date: token_response
            .expires_in
            .map(|seconds| now_timestamp_ms() + seconds.saturating_mul(1000)),
        selected_auth_type: Some("oauth-personal".to_string()),
        project_id: None,
        tier_id: None,
        plan_name: None,
        credits: Vec::new(),
        quota: AntigravityQuotaSummary::default(),
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        usage_updated_at: None,
        status: None,
        status_reason: None,
        created_at: now,
        last_used: now,
    };

    upsert_account_in(storage_dir, account)
}

fn build_oauth_start(
    login_id: &str,
    state: &str,
    callback_port: u16,
) -> Result<AntigravityOAuthStartResponse, String> {
    let callback_url = format!(
        "http://127.0.0.1:{}{}",
        callback_port, ANTIGRAVITY_OAUTH_CALLBACK_PATH
    );
    let scope = ANTIGRAVITY_OAUTH_SCOPES.join(" ");
    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&access_type=offline&scope={}&state={}&prompt=consent",
        ANTIGRAVITY_OAUTH_AUTHORIZE_ENDPOINT,
        urlencoding::encode(ANTIGRAVITY_OAUTH_CLIENT_ID),
        urlencoding::encode(&callback_url),
        urlencoding::encode(&scope),
        urlencoding::encode(state)
    );

    Ok(AntigravityOAuthStartResponse {
        login_id: login_id.to_string(),
        auth_url,
        callback_url,
        expires_at: now_timestamp() + ANTIGRAVITY_OAUTH_TIMEOUT_SECONDS,
    })
}

fn pending_oauth() -> Option<PendingAntigravityOAuth> {
    PENDING_ANTIGRAVITY_OAUTH
        .lock()
        .ok()
        .and_then(|state| state.clone())
}

fn set_pending_oauth(pending: Option<PendingAntigravityOAuth>) {
    if let Ok(mut state) = PENDING_ANTIGRAVITY_OAUTH.lock() {
        *state = pending;
    }
}

fn pending_oauth_for(login_id: &str) -> Result<PendingAntigravityOAuth, String> {
    let pending = pending_oauth()
        .ok_or_else(|| "Antigravity OAuth login was cancelled. Start again.".to_string())?;
    if pending.login_id != login_id {
        return Err("Antigravity OAuth login session changed. Start again.".to_string());
    }
    Ok(pending)
}

fn start_callback_listener() -> Result<CallbackListener, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|err| format!("Could not start Antigravity OAuth callback: {}", err))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("Could not read Antigravity callback port: {}", err))?
        .port();
    Ok(CallbackListener { port, listener })
}

fn spawn_callback_listener(listener: TcpListener) {
    std::thread::spawn(move || {
        for stream in listener.incoming().take(1) {
            if let Ok(mut stream) = stream {
                handle_callback_stream(&mut stream);
            }
        }
    });
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
        "Antigravity connected. You can return to Quota."
    } else {
        "Antigravity connection failed. Return to Quota and try again."
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
    if !path.starts_with(ANTIGRAVITY_OAUTH_CALLBACK_PATH) {
        return Err("Antigravity OAuth callback path did not match.".to_string());
    }
    let query = path
        .split_once('?')
        .map(|(_, query)| query)
        .ok_or_else(|| {
            "Antigravity OAuth callback did not include query parameters.".to_string()
        })?;
    let params = parse_query_params(query);
    if let Some(error) = params.get("error") {
        return Err(format!(
            "Antigravity OAuth returned error: {}",
            params
                .get("error_description")
                .map(String::as_str)
                .unwrap_or(error)
        ));
    }
    let code = params
        .get("code")
        .and_then(|value| normalize_optional(value.clone()))
        .ok_or_else(|| "Antigravity OAuth callback did not include a code.".to_string())?;
    let state = params
        .get("state")
        .and_then(|value| normalize_optional(value.clone()))
        .ok_or_else(|| "Antigravity OAuth callback did not include state.".to_string())?;

    let mut pending = PENDING_ANTIGRAVITY_OAUTH
        .lock()
        .map_err(|_| "Antigravity OAuth state lock failed.".to_string())?;
    let current = pending
        .as_mut()
        .ok_or_else(|| "Antigravity OAuth login was cancelled. Start again.".to_string())?;
    if current.expires_at <= now_timestamp() {
        *pending = None;
        return Err("Antigravity OAuth login expired. Start again.".to_string());
    }
    if current.state != state {
        return Err("Antigravity OAuth state mismatch. Start again.".to_string());
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
    pending: &PendingAntigravityOAuth,
    code: &str,
) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .post(GOOGLE_TOKEN_ENDPOINT)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&[
            ("code", code),
            ("client_id", ANTIGRAVITY_OAUTH_CLIENT_ID),
            ("client_secret", ANTIGRAVITY_OAUTH_CLIENT_SECRET),
            ("redirect_uri", pending.callback_url.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|err| format!("Antigravity OAuth token request failed: {}", err))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("Could not read Antigravity OAuth token response: {}", err))?;
    if !status.is_success() {
        return Err(format!(
            "Antigravity OAuth token exchange returned {} with body length {}",
            status,
            body.len()
        ));
    }
    serde_json::from_str(&body)
        .map_err(|err| format!("Could not parse Antigravity OAuth token response: {}", err))
}

async fn refresh_account_in(
    storage_dir: &Path,
    account_id: &str,
) -> Result<AntigravityAccountSummary, String> {
    let mut account = load_account_in(storage_dir, account_id)?;

    if let Err(error) = ensure_access_token_valid(&mut account).await {
        return record_refresh_error_in(storage_dir, account, error);
    }
    let load_status = match load_code_assist_status(&account.access_token).await {
        Ok(load_status) => load_status,
        Err(error) => return record_refresh_error_in(storage_dir, account, error),
    };

    if let Some(userinfo) = fetch_google_userinfo(&account.access_token).await {
        if let Some(email) = userinfo.email.and_then(normalize_optional) {
            account.email = email;
        }
        if account.auth_id.is_none() {
            account.auth_id = userinfo.id.and_then(normalize_optional);
        }
        if account.name.is_none() {
            account.name = userinfo.name.and_then(normalize_optional);
        }
    }

    account.project_id = load_status.project_id;
    account.tier_id = load_status.tier_id;
    account.plan_name = load_status
        .tier_name
        .or_else(|| account.tier_id.as_deref().and_then(parse_tier_plan_name));
    account.credits = load_status.credits;
    account.last_used = now_timestamp();

    if let Some(project_id) = account.project_id.as_deref() {
        match retrieve_user_quota(&account.access_token, project_id).await {
            Ok(raw_quota) => {
                account.quota = parse_quota_from_value(&raw_quota);
                account.quota_query_last_error = None;
                account.quota_query_last_error_at = None;
                account.usage_updated_at = Some(account.last_used);
                account.status = None;
                account.status_reason = None;
            }
            Err(error) => {
                account.quota_query_last_error = Some(error.clone());
                account.quota_query_last_error_at = Some(now_timestamp_ms());
                if is_forbidden_error(&error) {
                    account.status = Some("forbidden".to_string());
                    account.status_reason = Some(error);
                }
            }
        }
    }

    save_account_in(storage_dir, &account)?;
    Ok(account.to_summary())
}

fn record_refresh_error_in(
    storage_dir: &Path,
    mut account: StoredAntigravityAccount,
    error: String,
) -> Result<AntigravityAccountSummary, String> {
    account.last_used = now_timestamp();
    account.quota_query_last_error = Some(error.clone());
    account.quota_query_last_error_at = Some(now_timestamp_ms());
    if is_forbidden_error(&error) {
        account.status = Some("forbidden".to_string());
        account.status_reason = Some(error);
    }
    save_account_in(storage_dir, &account)?;
    Ok(account.to_summary())
}

async fn ensure_access_token_valid(account: &mut StoredAntigravityAccount) -> Result<(), String> {
    let should_refresh = account
        .expiry_date
        .map(|expiry| expiry <= now_timestamp_ms() + 60_000)
        .unwrap_or(false);

    if !should_refresh {
        return Ok(());
    }

    let refresh_token = account
        .refresh_token
        .clone()
        .ok_or_else(|| "Antigravity refresh token is missing.".to_string())?;
    let refreshed = refresh_access_token(&refresh_token).await?;
    account.access_token = refreshed
        .access_token
        .ok_or_else(|| "Antigravity token refresh returned no access token.".to_string())?;
    if let Some(id_token) = refreshed.id_token {
        account.id_token = Some(id_token);
    }
    if let Some(token_type) = refreshed.token_type {
        account.token_type = Some(token_type);
    }
    if let Some(scope) = refreshed.scope {
        account.scope = Some(scope);
    }
    if let Some(expires_in) = refreshed.expires_in {
        account.expiry_date = Some(now_timestamp_ms() + expires_in.saturating_mul(1000));
    }
    Ok(())
}

async fn refresh_access_token(refresh_token: &str) -> Result<TokenRefreshResponse, String> {
    let params = [
        ("client_id", ANTIGRAVITY_OAUTH_CLIENT_ID),
        ("client_secret", ANTIGRAVITY_OAUTH_CLIENT_SECRET),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    let response = reqwest::Client::new()
        .post(GOOGLE_TOKEN_ENDPOINT)
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .map_err(|err| format!("Could not refresh Antigravity token: {}", err))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| format!("Could not read Antigravity token response: {}", err))?;
    if !status.is_success() {
        return Err(format!(
            "Antigravity token refresh failed: status={} {}",
            status, text
        ));
    }
    let parsed: TokenRefreshResponse = serde_json::from_str(&text)
        .map_err(|err| format!("Could not parse Antigravity token response: {}", err))?;
    if parsed.error.is_some() {
        return Err(parsed
            .error_description
            .clone()
            .unwrap_or_else(|| format!("Antigravity token refresh error: {:?}", parsed.error)));
    }
    Ok(parsed)
}

async fn fetch_google_userinfo(access_token: &str) -> Option<GoogleUserInfo> {
    let response = reqwest::Client::new()
        .get(GOOGLE_USERINFO_ENDPOINT)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .send()
        .await
        .ok()?;
    response.json::<GoogleUserInfo>().await.ok()
}

async fn load_code_assist_status(access_token: &str) -> Result<CodeAssistStatus, String> {
    let raw = post_code_assist_json(
        access_token,
        &code_assist_url(CODE_ASSIST_LOAD_ENDPOINT),
        &build_load_code_assist_payload(),
    )
    .await?;
    Ok(parse_load_code_assist_status(&raw))
}

fn parse_load_code_assist_status(raw: &Value) -> CodeAssistStatus {
    let current_tier = raw.get("currentTier");
    let paid_tier = raw.get("paidTier");
    let tier_id = first_non_empty([
        paid_tier
            .and_then(|item| item.get("id"))
            .and_then(Value::as_str),
        current_tier
            .and_then(|item| item.get("id"))
            .and_then(Value::as_str),
        raw.get("allowedTiers")
            .and_then(Value::as_array)
            .and_then(|tiers| tiers.first())
            .and_then(|tier| tier.get("id"))
            .and_then(Value::as_str),
    ]);
    let tier_name = first_non_empty([
        paid_tier
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str),
        current_tier
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str),
    ]);
    let project_id = first_non_empty([
        raw.get("cloudaicompanionProject").and_then(Value::as_str),
        raw.get("cloudaicompanionProject")
            .and_then(|item| item.get("id"))
            .and_then(Value::as_str),
        raw.get("cloudaicompanionProject")
            .and_then(|item| item.get("projectId"))
            .and_then(Value::as_str),
    ]);
    let credits = paid_tier
        .and_then(|tier| tier.get("availableCredits"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let credit_type = item
                        .get("creditType")
                        .and_then(Value::as_str)
                        .and_then(|value| normalize_optional(value.to_string()))?;
                    let credit_amount = item
                        .get("creditAmount")
                        .and_then(Value::as_str)
                        .and_then(|value| normalize_optional(value.to_string()));
                    if credit_amount.is_none() {
                        return None;
                    }
                    Some(AntigravityCreditInfo {
                        credit_type,
                        credit_amount,
                        minimum_credit_amount_for_usage: item
                            .get("minimumCreditAmountForUsage")
                            .and_then(Value::as_str)
                            .and_then(|value| normalize_optional(value.to_string())),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    CodeAssistStatus {
        tier_id,
        tier_name,
        project_id,
        credits,
    }
}

async fn retrieve_user_quota(access_token: &str, project_id: &str) -> Result<Value, String> {
    let project_payload = serde_json::json!({ "project": project_id });
    post_code_assist_json(
        access_token,
        &code_assist_url(CODE_ASSIST_FETCH_MODELS_ENDPOINT),
        &project_payload,
    )
    .await?;

    post_code_assist_json(
        access_token,
        &code_assist_url(CODE_ASSIST_RETRIEVE_QUOTA_ENDPOINT),
        &project_payload,
    )
    .await
}

async fn post_code_assist_json(
    access_token: &str,
    endpoint: &str,
    payload: &Value,
) -> Result<Value, String> {
    let mut request = reqwest::Client::new()
        .post(endpoint)
        .header(AUTHORIZATION, format!("Bearer {}", access_token));
    for (name, value) in build_code_assist_headers(endpoint) {
        request = request.header(name, value);
    }
    let response = request
        .json(payload)
        .send()
        .await
        .map_err(|err| format!("Antigravity quota request failed: {}", err))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| format!("Could not read Antigravity quota response: {}", err))?;
    if !status.is_success() {
        return Err(format!(
            "Antigravity quota request failed: status={} body_length={} preview={}",
            status,
            text.len(),
            response_preview(&text)
        ));
    }
    parse_code_assist_response_text(endpoint, status.as_u16(), &text)
}

fn parse_code_assist_response_text(
    endpoint: &str,
    status: u16,
    text: &str,
) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }

    serde_json::from_str(text).map_err(|err| {
        format!(
            "Could not parse Antigravity quota response: {} endpoint={} status={} body_length={} preview={}",
            err,
            endpoint,
            status,
            text.len(),
            response_preview(text)
        )
    })
}

fn response_preview(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(300)
        .collect()
}

fn build_load_code_assist_payload() -> Value {
    serde_json::json!({
        "mode": "FULL_ELIGIBILITY_CHECK",
        "metadata": {
            "ideName": "antigravity",
            "ideType": "ANTIGRAVITY",
            "ideVersion": ANTIGRAVITY_IDE_VERSION,
            "pluginVersion": env!("CARGO_PKG_NAME"),
            "platform": antigravity_platform_name(),
            "updateChannel": "stable",
            "pluginType": "GEMINI"
        }
    })
}

fn code_assist_url(path: &str) -> String {
    format!("{}/{}", CODE_ASSIST_BASE_ENDPOINT, path)
}

fn code_assist_user_agent(endpoint: &str) -> String {
    let base = format!(
        "antigravity/{} {}/{}",
        ANTIGRAVITY_IDE_VERSION,
        antigravity_user_agent_os(),
        antigravity_user_agent_arch()
    );
    if endpoint.contains(CODE_ASSIST_LOAD_ENDPOINT) {
        format!(
            "{} google-api-nodejs-client/{}",
            base, ANTIGRAVITY_GOOGLE_API_NODEJS_CLIENT_VERSION
        )
    } else {
        base
    }
}

fn build_code_assist_headers(endpoint: &str) -> Vec<(String, String)> {
    vec![
        (
            CONTENT_TYPE.as_str().to_string(),
            "application/json".to_string(),
        ),
        (
            USER_AGENT.as_str().to_string(),
            code_assist_user_agent(endpoint),
        ),
        (
            "x-goog-api-client".to_string(),
            ANTIGRAVITY_X_GOOG_API_CLIENT.to_string(),
        ),
        (ACCEPT.as_str().to_string(), "*/*".to_string()),
    ]
}

fn antigravity_user_agent_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "windows",
        "linux" => "linux",
        _ => "windows",
    }
}

fn antigravity_user_agent_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => "amd64",
    }
}

fn antigravity_platform_name() -> &'static str {
    match (antigravity_user_agent_os(), antigravity_user_agent_arch()) {
        ("darwin", "amd64") => "DARWIN_AMD64",
        ("darwin", "arm64") => "DARWIN_ARM64",
        ("linux", "amd64") => "LINUX_AMD64",
        ("linux", "arm64") => "LINUX_ARM64",
        ("windows", "amd64") => "WINDOWS_AMD64",
        _ => "PLATFORM_UNSPECIFIED",
    }
}

fn read_active_google_email(gemini_home: &Path) -> Option<String> {
    let raw = fs::read_to_string(gemini_home.join("google_accounts.json")).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    first_non_empty([
        value.get("active").and_then(Value::as_str),
        value.get("activeEmail").and_then(Value::as_str),
        value.get("current").and_then(Value::as_str),
    ])
}

fn read_selected_auth_type(gemini_home: &Path) -> Option<String> {
    let raw = fs::read_to_string(gemini_home.join("settings.json")).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    value
        .get("security")
        .and_then(|item| item.get("auth"))
        .and_then(|item| item.get("selectedType"))
        .and_then(Value::as_str)
        .and_then(|item| normalize_optional(item.to_string()))
}

fn parse_quota_from_value(raw: &Value) -> AntigravityQuotaSummary {
    let mut quota = AntigravityQuotaSummary::default();
    let Some(groups) = raw.get("groups").and_then(Value::as_array) else {
        return quota;
    };

    for group in groups {
        let Some(buckets) = group.get("buckets").and_then(Value::as_array) else {
            continue;
        };
        for bucket in buckets {
            let bucket_id = bucket
                .get("bucketId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let window = AntigravityQuotaWindow {
                remaining_percent: bucket
                    .get("remainingFraction")
                    .and_then(value_to_number)
                    .map(|fraction| clamp_percent(fraction * 100.0)),
                reset_at: bucket.get("resetTime").and_then(parse_reset_at),
            };

            match bucket_id {
                "gemini-5h" => quota.gemini_five_hour = window,
                "gemini-weekly" => quota.gemini_weekly = window,
                "3p-5h" => quota.third_party_five_hour = window,
                "3p-weekly" => quota.third_party_weekly = window,
                _ => {}
            }
        }
    }

    quota
}

fn value_to_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| {
            value
                .as_str()
                .and_then(|item| item.trim().parse::<f64>().ok())
        })
        .filter(|item| item.is_finite())
}

fn parse_reset_at(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        if number <= 0 {
            return None;
        }
        return Some(if number > 10_000_000_000 {
            number / 1000
        } else {
            number
        });
    }

    let raw = value.as_str()?.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(number) = raw.parse::<i64>() {
        return Some(if number > 10_000_000_000 {
            number / 1000
        } else {
            number
        });
    }
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|date| date.timestamp())
}

fn clamp_percent(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    value.round().clamp(0.0, 100.0) as i32
}

fn decode_jwt_payload(token: &str) -> Result<JwtPayload, String> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| "Antigravity id token is not a JWT".to_string())?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|err| format!("Could not decode Antigravity id token: {}", err))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| format!("Could not parse Antigravity id token: {}", err))
}

fn build_account_id(email: &str, auth_id: Option<&str>) -> String {
    let key = format!(
        "{}:{}",
        email.trim().to_lowercase(),
        auth_id.unwrap_or_default()
    );
    format!(
        "antigravity_{}",
        format!("{:x}", md5::compute(key.as_bytes()))
    )
}

fn normalize_optional(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn first_non_empty<const N: usize>(items: [Option<&str>; N]) -> Option<String> {
    items
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|item| !item.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_tier_plan_name(tier_id: &str) -> Option<String> {
    let lower = tier_id.trim().to_lowercase();
    if lower.contains("ultra") {
        Some("Ultra".to_string())
    } else if lower.contains("pro") || lower.contains("premium") {
        Some("Pro".to_string())
    } else if lower.contains("free") || lower == "standard-tier" {
        Some("Free".to_string())
    } else {
        normalize_optional(tier_id.to_string())
    }
}

fn is_forbidden_error(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("status=403")
        || lower.contains("403 forbidden")
        || lower.contains("permission_denied")
        || lower.contains("caller does not have permission")
}

impl StoredAntigravityAccount {
    fn to_summary(&self) -> AntigravityAccountSummary {
        AntigravityAccountSummary {
            id: self.id.clone(),
            email: self.email.clone(),
            auth_id: self.auth_id.clone(),
            name: self.name.clone(),
            source: self.source.clone(),
            selected_auth_type: self.selected_auth_type.clone(),
            project_id: self.project_id.clone(),
            tier_id: self.tier_id.clone(),
            plan_name: self.plan_name.clone(),
            credits: self.credits.clone(),
            quota: self.quota.clone(),
            quota_query_last_error: self.quota_query_last_error.clone(),
            quota_query_last_error_at: self.quota_query_last_error_at,
            usage_updated_at: self.usage_updated_at,
            status: self.status.clone(),
            status_reason: self.status_reason.clone(),
            created_at: self.created_at,
            last_used: self.last_used,
        }
    }
}

fn default_local_source() -> String {
    "local".to_string()
}
