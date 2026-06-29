use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const DATA_DIR: &str = ".quota";
const ACCOUNTS_DIR: &str = "kiro_accounts";
const ACCOUNTS_INDEX_FILE: &str = "kiro_accounts.json";
const KIRO_AUTH_PORTAL_URL: &str = "https://app.kiro.dev/signin";
const KIRO_TOKEN_ENDPOINT: &str = "https://prod.us-east-1.auth.desktop.kiro.dev/oauth/token";
const KIRO_REFRESH_ENDPOINT: &str = "https://prod.us-east-1.auth.desktop.kiro.dev/refreshToken";
const KIRO_RUNTIME_DEFAULT_ENDPOINT: &str = "https://q.us-east-1.amazonaws.com";
const OAUTH_TIMEOUT_SECONDS: i64 = 600;
const CALLBACK_PORT_CANDIDATES: [u16; 10] = [
    3128, 4649, 6588, 8008, 9091, 49153, 50153, 51153, 52153, 53153,
];

static PENDING_KIRO_OAUTH: std::sync::LazyLock<Arc<Mutex<Option<PendingKiroOAuth>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(None)));

#[derive(Debug, Clone)]
struct PendingKiroOAuth {
    login_id: String,
    state: String,
    code_verifier: String,
    callback_port: u16,
    callback_url: String,
    expires_at: i64,
    // Set by the callback server when the browser redirects back
    callback_result: Option<Result<CallbackData, String>>,
}

#[derive(Debug, Clone)]
struct CallbackData {
    code: String,
    path: String,
    login_option: String,
    issuer_url: Option<String>,
    idc_region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredKiroAccount {
    id: String,
    email: String,
    login_provider: Option<String>,
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
    idc_region: Option<String>,
    client_id: Option<String>,
    plan_name: Option<String>,
    plan_tier: Option<String>,
    credits_total: Option<f64>,
    credits_used: Option<f64>,
    bonus_total: Option<f64>,
    bonus_used: Option<f64>,
    usage_reset_at: Option<i64>,
    bonus_expire_days: Option<i64>,
    kiro_auth_token_raw: Option<Value>,
    kiro_profile_raw: Option<Value>,
    status: Option<String>,
    status_reason: Option<String>,
    quota_query_last_error: Option<String>,
    quota_query_last_error_at: Option<i64>,
    usage_updated_at: Option<i64>,
    created_at: i64,
    last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroAccountSummary {
    pub id: String,
    pub email: String,
    pub login_provider: Option<String>,
    pub plan_name: Option<String>,
    pub credits_total: Option<f64>,
    pub credits_used: Option<f64>,
    pub bonus_total: Option<f64>,
    pub bonus_used: Option<f64>,
    pub usage_reset_at: Option<i64>,
    pub bonus_expire_days: Option<i64>,
    pub status: Option<String>,
    pub status_reason: Option<String>,
    pub quota_query_last_error: Option<String>,
    pub quota_query_last_error_at: Option<i64>,
    pub usage_updated_at: Option<i64>,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KiroAccountIndex {
    version: String,
    account_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroOAuthStartResponse {
    pub login_id: String,
    pub auth_url: String,
    pub callback_url: String,
    pub expires_at: i64,
}

impl KiroAccountIndex {
    fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            account_ids: Vec::new(),
        }
    }
}

// ── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_kiro_accounts() -> Result<Vec<KiroAccountSummary>, String> {
    list_accounts_in(&quota_storage_dir()?)
}

#[tauri::command]
pub async fn import_kiro_from_local() -> Result<Vec<KiroAccountSummary>, String> {
    let auth_token = read_local_auth_token()?.ok_or_else(|| {
        "No Kiro auth token found at ~/.aws/sso/cache/kiro-auth-token.json".to_string()
    })?;
    let profile = read_local_profile_json()?;
    let account = build_and_save_account(auth_token, profile, None).await?;
    Ok(vec![account])
}

#[tauri::command]
pub fn kiro_oauth_login_start() -> Result<KiroOAuthStartResponse, String> {
    let login_id = generate_token();
    let state = generate_token();
    let code_verifier = generate_token();
    let code_challenge = pkce_challenge(&code_verifier);

    let callback_port = find_free_port()?;
    // Base URL only — no path. Cognito has http://localhost (no port, no path) registered.
    // AWS Cognito's localhost special-casing matches any port, but breaks if a path is present.
    let callback_url = format!("http://localhost:{}", callback_port);

    let auth_url = format!(
        "{}?state={}&code_challenge={}&code_challenge_method=S256&redirect_uri={}&redirect_from=KiroIDE",
        KIRO_AUTH_PORTAL_URL,
        urlencoding::encode(&state),
        urlencoding::encode(&code_challenge),
        urlencoding::encode(&callback_url),
    );

    let expires_at = now_timestamp() + OAUTH_TIMEOUT_SECONDS;

    let pending = PendingKiroOAuth {
        login_id: login_id.clone(),
        state: state.clone(),
        code_verifier,
        callback_port,
        callback_url: callback_url.clone(),
        expires_at,
        callback_result: None,
    };

    set_pending_oauth(Some(pending));
    spawn_callback_server(callback_port, login_id.clone(), state);

    Ok(KiroOAuthStartResponse {
        login_id,
        auth_url,
        callback_url,
        expires_at,
    })
}

#[tauri::command]
pub async fn kiro_oauth_login_complete(login_id: String) -> Result<KiroAccountSummary, String> {
    let deadline = now_timestamp() + OAUTH_TIMEOUT_SECONDS;
    loop {
        let result = {
            let guard = PENDING_KIRO_OAUTH
                .lock()
                .map_err(|_| "Kiro OAuth state lock unavailable".to_string())?;
            match guard.as_ref() {
                None => return Err("Kiro OAuth login was cancelled.".to_string()),
                Some(pending) => {
                    if pending.login_id != login_id {
                        return Err("Kiro OAuth login session changed.".to_string());
                    }
                    if now_timestamp() > pending.expires_at {
                        return Err("Kiro OAuth login expired. Start again.".to_string());
                    }
                    pending.callback_result.clone()
                }
            }
        };

        if let Some(callback_result) = result {
            return match callback_result {
                Err(msg) => {
                    set_pending_oauth(None);
                    Err(msg)
                }
                Ok(callback) => {
                    let pending = {
                        let guard = PENDING_KIRO_OAUTH
                            .lock()
                            .map_err(|_| "Kiro OAuth state lock unavailable".to_string())?;
                        guard
                            .clone()
                            .ok_or_else(|| "Kiro OAuth login was cancelled.".to_string())?
                    };
                    // Use the actual path Kiro redirected to (varies by login provider).
                    // Strip any path from callback_url to get just the base URL.
                    let base_url = {
                        let raw = &pending.callback_url;
                        if let Some(pos) = raw
                            .find("://")
                            .and_then(|p| raw[p + 3..].find('/').map(|q| p + 3 + q))
                        {
                            &raw[..pos]
                        } else {
                            raw.trim_end_matches('/')
                        }
                    };
                    let callback_path = if callback.path.is_empty() {
                        "/oauth/callback".to_string()
                    } else if callback.path.starts_with('/') {
                        callback.path.clone()
                    } else {
                        format!("/{}", callback.path)
                    };
                    let redirect_uri = if callback.login_option.is_empty() {
                        format!("{}{}", base_url, callback_path)
                    } else {
                        format!(
                            "{}{}?login_option={}",
                            base_url,
                            callback_path,
                            urlencoding::encode(&callback.login_option),
                        )
                    };
                    let auth_token = exchange_code_for_token(
                        &callback.code,
                        &pending.code_verifier,
                        &redirect_uri,
                        &callback.login_option,
                        callback.issuer_url.as_deref(),
                        callback.idc_region.as_deref(),
                    )
                    .await?;
                    set_pending_oauth(None);
                    let account = build_and_save_account(auth_token, None, None).await?;
                    Ok(account)
                }
            };
        }

        if now_timestamp() > deadline {
            set_pending_oauth(None);
            return Err("Kiro OAuth login timed out. Start again.".to_string());
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    }
}

#[tauri::command]
pub fn kiro_oauth_login_cancel(login_id: Option<String>) -> Result<(), String> {
    if let Some(id) = login_id {
        let guard = PENDING_KIRO_OAUTH
            .lock()
            .map_err(|_| "Kiro OAuth state lock unavailable".to_string())?;
        if guard.as_ref().map(|p| p.login_id.as_str()) == Some(id.as_str()) {
            drop(guard);
            set_pending_oauth(None);
        }
    } else {
        set_pending_oauth(None);
    }
    Ok(())
}

#[tauri::command]
pub fn kiro_oauth_submit_callback_url(
    login_id: String,
    callback_url: String,
) -> Result<(), String> {
    let parsed = parse_callback_url(&callback_url);
    match parsed {
        Err(msg) => Err(msg),
        Ok((code, login_option, state)) => {
            let mut guard = PENDING_KIRO_OAUTH
                .lock()
                .map_err(|_| "Kiro OAuth state lock unavailable".to_string())?;
            if let Some(pending) = guard.as_mut() {
                if pending.login_id != login_id {
                    return Err("Login session changed.".to_string());
                }
                if let Some(expected_state) = &state {
                    if expected_state != &pending.state {
                        return Err("State mismatch in callback URL.".to_string());
                    }
                }
                pending.callback_result = Some(Ok(CallbackData {
                    code,
                    path: String::new(),
                    login_option: login_option.unwrap_or_default(),
                    issuer_url: None,
                    idc_region: None,
                }));
            } else {
                return Err("No active Kiro login session.".to_string());
            }
            Ok(())
        }
    }
}

#[tauri::command]
pub async fn refresh_kiro_account(account_id: String) -> Result<KiroAccountSummary, String> {
    let storage_dir = quota_storage_dir()?;
    refresh_account_in(&storage_dir, &account_id).await
}

#[tauri::command]
pub async fn refresh_all_kiro_accounts() -> Result<Vec<KiroAccountSummary>, String> {
    let storage_dir = quota_storage_dir()?;
    let account_ids = load_index_in(&storage_dir)?.account_ids;
    for account_id in &account_ids {
        let _ = refresh_account_in(&storage_dir, account_id).await;
    }
    list_accounts_in(&storage_dir)
}

#[tauri::command]
pub fn delete_kiro_account(account_id: String) -> Result<(), String> {
    let storage_dir = quota_storage_dir()?;
    let mut index = load_index_in(&storage_dir)?;
    index.account_ids.retain(|id| id != &account_id);
    save_index_in(&storage_dir, &index)?;
    let path = account_path_in(&storage_dir, &account_id);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Could not delete Kiro account: {}", e))?;
    }
    Ok(())
}

// ── OAuth helpers ───────────────────────────────────────────────────────────

fn generate_token() -> String {
    let bytes: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn pkce_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn find_free_port() -> Result<u16, String> {
    for port in CALLBACK_PORT_CANDIDATES {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    Err("No available callback port found. Close other applications and try again.".to_string())
}

fn set_pending_oauth(pending: Option<PendingKiroOAuth>) {
    if let Ok(mut guard) = PENDING_KIRO_OAUTH.lock() {
        *guard = pending;
    }
}

fn spawn_callback_server(port: u16, expected_login_id: String, expected_state: String) {
    std::thread::spawn(move || {
        let server = match tiny_http::Server::http(format!("127.0.0.1:{}", port)) {
            Ok(s) => s,
            Err(e) => {
                set_callback_error(
                    &expected_login_id,
                    &expected_state,
                    format!("Callback server failed to start: {}", e),
                );
                return;
            }
        };

        let deadline = std::time::Instant::now()
            + std::time::Duration::from_secs(OAUTH_TIMEOUT_SECONDS as u64);

        loop {
            if std::time::Instant::now() > deadline {
                set_callback_error(
                    &expected_login_id,
                    &expected_state,
                    "OAuth login timed out.".to_string(),
                );
                break;
            }

            let is_done = {
                let guard = PENDING_KIRO_OAUTH.lock().ok();
                guard
                    .as_ref()
                    .and_then(|g| g.as_ref())
                    .map(|p| p.login_id != expected_login_id || p.callback_result.is_some())
                    .unwrap_or(true)
            };
            if is_done {
                break;
            }

            if let Ok(Some(request)) = server.try_recv() {
                let raw_url = request.url().to_string();
                let (path, query) = raw_url.split_once('?').unwrap_or((raw_url.as_str(), ""));
                let path = path.to_string();

                if path == "/cancel" {
                    set_callback_error(
                        &expected_login_id,
                        &expected_state,
                        "Login cancelled.".to_string(),
                    );
                    let _ = request.respond(html_response(
                        200,
                        "<h2>Cancelled</h2><p>Return to Quota.</p>",
                    ));
                    break;
                }

                let params = parse_query_params(query);

                // Accept any path that carries code/error — Kiro uses different paths
                // per login provider (Google, GitHub, AWS Builder ID, etc.).
                let has_code = params.get("code").map(|c| !c.is_empty()).unwrap_or(false);
                let has_error = params.contains_key("error");

                if !has_code && !has_error {
                    // Might be a browser preflight or favicon — wait for the real callback.
                    let _ = request.respond(html_response(200, "<p>Waiting for Kiro login…</p>"));
                    continue;
                }

                if has_error {
                    let error = params.get("error").cloned().unwrap_or_default();
                    let desc = params.get("error_description").cloned().unwrap_or_default();
                    let msg = if desc.is_empty() {
                        format!("Authorization failed: {}", error)
                    } else {
                        format!("Authorization failed: {} ({})", error, desc)
                    };
                    set_callback_error(&expected_login_id, &expected_state, msg.clone());
                    let _ = request.respond(html_response(400,
                        &format!("<h2>Authorization failed</h2><p>{}</p><p>Close this tab and return to Quota to try again.</p>", error)
                    ));
                    break;
                }

                let callback_state = params.get("state").cloned().unwrap_or_default();
                if callback_state.is_empty() || callback_state != expected_state {
                    set_callback_error(
                        &expected_login_id,
                        &expected_state,
                        "State mismatch in OAuth callback.".to_string(),
                    );
                    let _ = request.respond(html_response(400,
                        "<h2>State mismatch</h2><p>Close this tab and try connecting again from Quota.</p>"
                    ));
                    break;
                }

                let code = params
                    .get("code")
                    .filter(|c| !c.is_empty())
                    .cloned()
                    .unwrap();

                let login_option = params
                    .get("login_option")
                    .or_else(|| params.get("loginOption"))
                    .cloned()
                    .unwrap_or_default()
                    .to_ascii_lowercase();

                let issuer_url = params
                    .get("issuer_url")
                    .or_else(|| params.get("issuerUrl"))
                    .filter(|v| !v.is_empty())
                    .cloned();

                let idc_region = params
                    .get("idc_region")
                    .or_else(|| params.get("idcRegion"))
                    .filter(|v| !v.is_empty())
                    .cloned();

                {
                    let mut guard = PENDING_KIRO_OAUTH.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(pending) = guard.as_mut() {
                        if pending.login_id == expected_login_id {
                            pending.callback_result = Some(Ok(CallbackData {
                                code,
                                path: path.clone(),
                                login_option,
                                issuer_url,
                                idc_region,
                            }));
                        }
                    }
                }

                let _ = request.respond(html_response(200,
                    "<h2>&#10003; Connected</h2><p>You can close this tab and return to Quota.</p><script>window.close();</script>"
                ));
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(120));
        }
    });
}

fn set_callback_error(expected_login_id: &str, expected_state: &str, message: String) {
    let mut guard = PENDING_KIRO_OAUTH.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(pending) = guard.as_mut() {
        if pending.login_id == expected_login_id && pending.state == expected_state {
            pending.callback_result = Some(Err(message));
        }
    }
}

fn html_response(
    status: u16,
    body_fragment: &str,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Quota – Kiro</title>\
        <style>body{{font-family:sans-serif;background:#111;color:#eee;text-align:center;padding:60px}}\
        h2{{margin-bottom:8px}}p{{color:#aaa}}</style></head>\
        <body>{}</body></html>",
        body_fragment
    );
    let bytes = html.into_bytes();
    let content_type =
        tiny_http::Header::from_bytes(b"Content-Type", b"text/html; charset=utf-8").unwrap();
    tiny_http::Response::new(
        tiny_http::StatusCode(status),
        vec![content_type],
        std::io::Cursor::new(bytes.clone()),
        Some(bytes.len()),
        None,
    )
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

fn parse_callback_url(raw: &str) -> Result<(String, Option<String>, Option<String>), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Callback URL is empty.".to_string());
    }

    let query = if let Some((_, q)) = trimmed.split_once('?') {
        q
    } else if trimmed.starts_with('/') || trimmed.starts_with("http") {
        return Err("Callback URL has no query parameters.".to_string());
    } else {
        trimmed.trim_start_matches('?')
    };

    let params = parse_query_params(query);
    let code = params
        .get("code")
        .filter(|c| !c.is_empty())
        .cloned()
        .ok_or_else(|| "No code parameter in callback URL.".to_string())?;
    let login_option = params
        .get("login_option")
        .or_else(|| params.get("loginOption"))
        .cloned();
    let state = params.get("state").cloned();
    Ok((code, login_option, state))
}

async fn exchange_code_for_token(
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
    login_option: &str,
    issuer_url: Option<&str>,
    idc_region: Option<&str>,
) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .post(KIRO_TOKEN_ENDPOINT)
        .header("Content-Type", "application/json")
        .json(&json!({
            "code": code,
            "code_verifier": code_verifier,
            "redirect_uri": redirect_uri,
        }))
        .send()
        .await
        .map_err(|e| format!("Kiro token exchange failed: {}", e))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Kiro token exchange error: status={} body_len={}",
            status,
            body.len()
        ));
    }

    let mut token: Value = serde_json::from_str(&body)
        .map_err(|e| format!("Could not parse Kiro token response: {}", e))?;

    // Unwrap { "data": { ... } } if present
    if let Some(data) = token
        .as_object_mut()
        .and_then(|obj| obj.remove("data"))
        .filter(|v| v.is_object())
    {
        token = data;
    }

    if let Some(obj) = token.as_object_mut() {
        // Inject login_option and provider so we can recover them later
        if !login_option.is_empty() {
            obj.entry("login_option")
                .or_insert_with(|| Value::String(login_option.to_string()));
            let provider = provider_from_login_option(login_option);
            obj.entry("provider")
                .or_insert_with(|| Value::String(provider.clone()));
            obj.entry("loginProvider")
                .or_insert_with(|| Value::String(provider));
        }
        // Inject IDC/issuer fields for AWS Builder ID accounts
        if let Some(iu) = issuer_url {
            obj.entry("issuer_url")
                .or_insert_with(|| Value::String(iu.to_string()));
        }
        if let Some(ir) = idc_region {
            obj.entry("idc_region")
                .or_insert_with(|| Value::String(ir.to_string()));
            obj.entry("idcRegion")
                .or_insert_with(|| Value::String(ir.to_string()));
        }
    }

    // Convert expiresIn → expiresAt if needed
    ensure_expires_at(&mut token);
    Ok(token)
}

fn ensure_expires_at(token: &mut Value) {
    let obj = match token.as_object_mut() {
        Some(o) => o,
        None => return,
    };
    if obj.contains_key("expiresAt") || obj.contains_key("expires_at") {
        return;
    }
    let expires_in = obj
        .get("expiresIn")
        .or_else(|| obj.get("expires_in"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if expires_in > 0 {
        let expires_at = now_timestamp() + expires_in;
        obj.insert("expiresAt".to_string(), Value::Number(expires_at.into()));
    }
}

fn provider_from_login_option(login_option: &str) -> String {
    match login_option.trim().to_ascii_lowercase().as_str() {
        "google" => "Google".to_string(),
        "github" => "GitHub".to_string(),
        _ => login_option.to_string(),
    }
}

// ── Usage fetch and parsing ─────────────────────────────────────────────────

fn runtime_endpoint_for_region(region: Option<&str>) -> String {
    let r = region.unwrap_or("us-east-1").trim().to_ascii_lowercase();
    match r.as_str() {
        "us-east-1" => "https://q.us-east-1.amazonaws.com".to_string(),
        "eu-central-1" => "https://q.eu-central-1.amazonaws.com".to_string(),
        _ => KIRO_RUNTIME_DEFAULT_ENDPOINT.to_string(),
    }
}

fn parse_profile_arn_region(arn: &str) -> Option<String> {
    let mut parts = arn.split(':');
    let prefix = parts.next()?.trim();
    if !prefix.eq_ignore_ascii_case("arn") {
        return None;
    }
    parts.next()?; // partition
    parts.next()?; // service
    let region = parts.next()?.trim();
    if region.is_empty() {
        None
    } else {
        Some(region.to_string())
    }
}

fn extract_profile_arn(auth_token: Option<&Value>, profile: Option<&Value>) -> Option<String> {
    pick_string(profile, &[&["arn"], &["profileArn"]])
        .or_else(|| pick_string(auth_token, &[&["profileArn"], &["profile_arn"], &["arn"]]))
}

async fn fetch_runtime_usage(access_token: &str, profile_arn: &str) -> Result<Value, String> {
    let region = parse_profile_arn_region(profile_arn);
    let endpoint = runtime_endpoint_for_region(region.as_deref());
    let url = format!(
        "{}/getUsageLimits?origin=AI_EDITOR&profileArn={}&resourceType=AGENTIC_REQUEST&isEmailRequired=true",
        endpoint.trim_end_matches('/'),
        urlencoding::encode(profile_arn),
    );

    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token.trim()))
        .send()
        .await
        .map_err(|e| format!("Kiro runtime usage request failed: {}", e))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        // 403 typically means the account is banned/disabled
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(format!(
                "BANNED:{}",
                parse_runtime_error(&body).unwrap_or(body)
            ));
        }
        return Err(format!("Kiro runtime usage error: status={}", status));
    }

    serde_json::from_str(&body).map_err(|e| format!("Could not parse Kiro usage response: {}", e))
}

fn parse_runtime_error(body: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(body).ok()?;
    pick_string(
        Some(&parsed),
        &[
            &["reason"],
            &["message"],
            &["errorMessage"],
            &["error", "message"],
            &["detail"],
        ],
    )
}

async fn try_refresh_token(refresh_token: &str) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .post(KIRO_REFRESH_ENDPOINT)
        .header("Content-Type", "application/json")
        .json(&json!({ "refreshToken": refresh_token }))
        .send()
        .await
        .map_err(|e| format!("Kiro token refresh failed: {}", e))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Kiro refresh token error: status={}", status));
    }

    let mut token: Value = serde_json::from_str(&body)
        .map_err(|e| format!("Could not parse Kiro refresh response: {}", e))?;
    if let Some(data) = token
        .as_object_mut()
        .and_then(|o| o.remove("data"))
        .filter(|v| v.is_object())
    {
        token = data;
    }
    ensure_expires_at(&mut token);
    Ok(token)
}

struct ParsedUsage {
    plan_name: Option<String>,
    credits_total: Option<f64>,
    credits_used: Option<f64>,
    bonus_total: Option<f64>,
    bonus_used: Option<f64>,
    usage_reset_at: Option<i64>,
    bonus_expire_days: Option<i64>,
    email: Option<String>,
}

fn resolve_usage_root(usage: &Value) -> &Value {
    if let Some(state) = usage
        .as_object()
        .and_then(|o| o.get("kiro.resourceNotifications.usageState"))
    {
        return state;
    }
    if let Some(state) = usage.get("usageState") {
        return state;
    }
    usage
}

fn parse_usage(raw: &Value) -> ParsedUsage {
    let root = resolve_usage_root(raw);

    // Email from userInfo
    let email = pick_string(Some(raw), &[&["userInfo", "email"], &["email"]]);

    // Plan name
    let plan_name = pick_string(
        Some(root),
        &[
            &["planName"],
            &["currentPlanName"],
            &["subscriptionInfo", "subscriptionName"],
            &["subscriptionInfo", "subscriptionTitle"],
            &["subscriptionInfo", "type"],
            &["usageBreakdowns", "planName"],
            &["plan", "name"],
        ],
    );

    // Find the primary breakdown (prefer type == "credit")
    let breakdown = find_primary_breakdown(root);

    let plan_name = plan_name.or_else(|| {
        pick_string(
            breakdown,
            &[
                &["displayName"],
                &["displayNamePlural"],
                &["type"],
                &["unit"],
            ],
        )
    });

    // Credits total/used
    let credits_total = pick_number(
        Some(root),
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
        Some(root),
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

    // Bonus / free trial credits
    let free_trial =
        breakdown.and_then(|b| b.get("freeTrialUsage").or_else(|| b.get("freeTrialInfo")));

    let bonus_total = pick_number(
        free_trial,
        &[
            &["usageLimitWithPrecision"],
            &["usageLimit"],
            &["limit"],
            &["total"],
        ],
    )
    .or_else(|| {
        pick_number(
            Some(root),
            &[&["bonusCredits", "total"], &["bonus", "total"]],
        )
    });

    let bonus_used = pick_number(
        free_trial,
        &[&["currentUsageWithPrecision"], &["currentUsage"], &["used"]],
    )
    .or_else(|| pick_number(Some(root), &[&["bonusCredits", "used"], &["bonus", "used"]]));

    // Bonus expiry days
    let bonus_expire_days = pick_number(
        free_trial,
        &[&["daysRemaining"], &["expiryDays"], &["expireDays"]],
    )
    .map(|v| v.round() as i64)
    .or_else(|| {
        let expiry_ts = pick_timestamp(free_trial, &[&["expiryDate"], &["freeTrialExpiry"]])?;
        let now = now_timestamp();
        if expiry_ts <= now {
            Some(0)
        } else {
            Some(((expiry_ts - now) as f64 / 86400.0).ceil() as i64)
        }
    });

    // Usage reset time
    let usage_reset_at = pick_timestamp(
        Some(root),
        &[
            &["resetAt"],
            &["resetTime"],
            &["resetOn"],
            &["nextDateReset"],
            &["usageBreakdowns", "resetAt"],
        ],
    )
    .or_else(|| pick_timestamp(breakdown, &[&["resetDate"], &["resetAt"]]));

    ParsedUsage {
        plan_name,
        credits_total,
        credits_used,
        bonus_total,
        bonus_used,
        usage_reset_at,
        bonus_expire_days,
        email,
    }
}

fn find_primary_breakdown(root: &Value) -> Option<&Value> {
    let list = root
        .get("usageBreakdownList")
        .and_then(|v| v.as_array())
        .or_else(|| root.get("usageBreakdowns").and_then(|v| v.as_array()))?;

    if list.is_empty() {
        return None;
    }

    list.iter()
        .find(|item| {
            item.get("type")
                .and_then(|t| t.as_str())
                .map(|t| t.eq_ignore_ascii_case("credit"))
                .unwrap_or(false)
        })
        .or_else(|| list.first())
}

// ── Account build/save ──────────────────────────────────────────────────────

async fn build_and_save_account(
    auth_token: Value,
    profile: Option<Value>,
    usage: Option<Value>,
) -> Result<KiroAccountSummary, String> {
    let storage_dir = quota_storage_dir()?;

    let access_token = pick_string(
        Some(&auth_token),
        &[
            &["accessToken"],
            &["access_token"],
            &["token"],
            &["idToken"],
            &["id_token"],
        ],
    )
    .ok_or_else(|| "Kiro auth token missing access token field.".to_string())?;

    let refresh_token = pick_string(Some(&auth_token), &[&["refreshToken"], &["refresh_token"]]);

    let expires_at = pick_timestamp(
        Some(&auth_token),
        &[
            &["expiresAt"],
            &["expires_at"],
            &["expiry"],
            &["expiration"],
        ],
    );

    let idc_region = pick_string(
        Some(&auth_token),
        &[&["idc_region"], &["idcRegion"], &["region"]],
    );

    let client_id = pick_string(Some(&auth_token), &[&["client_id"], &["clientId"]]);

    let login_provider = pick_string(
        Some(&auth_token),
        &[&["login_option"], &["provider"], &["loginProvider"]],
    )
    .map(|v| provider_from_login_option(&v));

    // Email from profile → auth token → decode JWT claims
    let email = pick_string(
        profile.as_ref(),
        &[&["email"], &["account", "email"], &["primaryEmail"]],
    )
    .or_else(|| {
        pick_string(
            Some(&auth_token),
            &[&["email"], &["userEmail"], &["login_hint"], &["loginHint"]],
        )
    })
    .or_else(|| decode_jwt_email(&access_token))
    .unwrap_or_default();

    let profile_arn = extract_profile_arn(Some(&auth_token), profile.as_ref());

    let mut account = StoredKiroAccount {
        // ID is computed after usage fetch, which may provide the email.
        // Kiro's token exchange often returns no email; usage API reliably returns userInfo.email.
        id: String::new(),
        email,
        login_provider,
        access_token: access_token.clone(),
        refresh_token,
        expires_at,
        idc_region,
        client_id,
        plan_name: None,
        plan_tier: None,
        credits_total: None,
        credits_used: None,
        bonus_total: None,
        bonus_used: None,
        usage_reset_at: None,
        bonus_expire_days: None,
        kiro_auth_token_raw: Some(auth_token.clone()),
        kiro_profile_raw: profile.clone(),
        status: None,
        status_reason: None,
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        usage_updated_at: None,
        created_at: now_timestamp(),
        last_used: now_timestamp(),
    };

    // Apply pre-fetched usage if provided
    if let Some(u) = usage {
        apply_usage(&mut account, &u);
    }

    // Try to fetch live usage from runtime endpoint
    if let Some(arn) = profile_arn.as_deref() {
        match fetch_and_apply_usage(&mut account, arn).await {
            Ok(()) => {}
            Err(e) => {
                account.quota_query_last_error = Some(e);
                account.quota_query_last_error_at = Some(now_timestamp_ms());
            }
        }
    }

    // Compute the stable ID now that usage may have populated account.email.
    // If email is still unknown (usage unavailable), fingerprint the access token so
    // two distinct OAuth sessions never collide on the same empty-email bucket.
    let id_email = if account.email.contains('@') {
        account.email.clone()
    } else {
        format!("__tok__{:x}", md5::compute(access_token.as_bytes()))
    };
    let final_arn = profile_arn.or_else(|| {
        extract_profile_arn(
            account.kiro_auth_token_raw.as_ref(),
            account.kiro_profile_raw.as_ref(),
        )
    });
    account.id = build_account_id(&id_email, final_arn.as_deref());

    upsert_account_in(&storage_dir, account)
}

async fn fetch_and_apply_usage(
    account: &mut StoredKiroAccount,
    profile_arn: &str,
) -> Result<(), String> {
    let usage = match fetch_runtime_usage(&account.access_token, profile_arn).await {
        Ok(u) => u,
        Err(e) => {
            // Try refresh token if first attempt fails
            let refresh_token = account
                .refresh_token
                .as_deref()
                .filter(|t| !t.is_empty())
                .ok_or_else(|| e.clone())?;
            let new_token = try_refresh_token(refresh_token).await?;
            if let Some(at) = pick_string(Some(&new_token), &[&["accessToken"], &["access_token"]])
            {
                account.access_token = at;
            }
            if let Some(rt) =
                pick_string(Some(&new_token), &[&["refreshToken"], &["refresh_token"]])
            {
                account.refresh_token = Some(rt);
            }
            if let Some(ea) = pick_timestamp(Some(&new_token), &[&["expiresAt"], &["expires_at"]]) {
                account.expires_at = Some(ea);
            }
            fetch_runtime_usage(&account.access_token, profile_arn).await?
        }
    };

    apply_usage(account, &usage);
    Ok(())
}

fn apply_usage(account: &mut StoredKiroAccount, usage: &Value) {
    let parsed = parse_usage(usage);

    if let Some(email) = parsed.email {
        if !email.is_empty() && email.contains('@') {
            account.email = email;
        }
    }
    if let Some(v) = parsed.plan_name {
        account.plan_name = Some(v);
    }
    if let Some(v) = parsed.credits_total {
        account.credits_total = Some(v);
    }
    if let Some(v) = parsed.credits_used {
        account.credits_used = Some(v);
    }
    if let Some(v) = parsed.bonus_total {
        account.bonus_total = Some(v);
    }
    if let Some(v) = parsed.bonus_used {
        account.bonus_used = Some(v);
    }
    if let Some(v) = parsed.usage_reset_at {
        account.usage_reset_at = Some(v);
    }
    if let Some(v) = parsed.bonus_expire_days {
        account.bonus_expire_days = Some(v);
    }
    account.quota_query_last_error = None;
    account.quota_query_last_error_at = None;
    account.usage_updated_at = Some(now_timestamp_ms());
}

fn resolve_plan_display(account: &StoredKiroAccount) -> Option<String> {
    let raw = account
        .plan_name
        .as_deref()
        .or(account.plan_tier.as_deref())?;
    let upper = raw.trim().to_uppercase();
    if upper.contains("FREE") || upper.contains("STANDALONE") {
        Some("FREE".to_string())
    } else if upper.contains("PRO") {
        Some("PRO".to_string())
    } else if upper.contains("INDIVIDUAL") {
        Some("INDIVIDUAL".to_string())
    } else if upper.contains("BUSINESS") || upper.contains("TEAM") {
        Some("BUSINESS".to_string())
    } else if upper.contains("ENTERPRISE") {
        Some("ENTERPRISE".to_string())
    } else {
        Some(upper)
    }
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
        .ok_or_else(|| "No parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("Could not create directory: {}", e))?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("kiro"),
        std::process::id()
    ));
    fs::write(&tmp, content).map_err(|e| format!("Could not write temp file: {}", e))?;
    fs::rename(&tmp, path).map_err(|e| format!("Could not replace file: {}", e))?;
    Ok(())
}

fn load_index_in(storage_dir: &Path) -> Result<KiroAccountIndex, String> {
    let path = index_path_in(storage_dir);
    if !path.exists() {
        return Ok(KiroAccountIndex::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read Kiro account index: {}", e))?;
    if content.trim().is_empty() {
        return Ok(KiroAccountIndex::new());
    }
    serde_json::from_str(&content).map_err(|e| format!("Could not parse Kiro account index: {}", e))
}

fn save_index_in(storage_dir: &Path, index: &KiroAccountIndex) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index)
        .map_err(|e| format!("Could not encode Kiro account index: {}", e))?;
    write_string_atomic(&index_path_in(storage_dir), &content)
}

fn load_account_in(storage_dir: &Path, account_id: &str) -> Result<StoredKiroAccount, String> {
    let content = fs::read_to_string(account_path_in(storage_dir, account_id))
        .map_err(|e| format!("Could not read Kiro account: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Could not parse Kiro account: {}", e))
}

fn save_account_in(storage_dir: &Path, account: &StoredKiroAccount) -> Result<(), String> {
    let content = serde_json::to_string_pretty(account)
        .map_err(|e| format!("Could not encode Kiro account: {}", e))?;
    write_string_atomic(&account_path_in(storage_dir, &account.id), &content)
}

fn list_accounts_in(storage_dir: &Path) -> Result<Vec<KiroAccountSummary>, String> {
    let index = load_index_in(storage_dir)?;
    Ok(index
        .account_ids
        .iter()
        .filter_map(|id| load_account_in(storage_dir, id).ok())
        .map(|a| a.to_summary())
        .collect())
}

fn upsert_account_in(
    storage_dir: &Path,
    mut account: StoredKiroAccount,
) -> Result<KiroAccountSummary, String> {
    let mut index = load_index_in(storage_dir)?;
    if let Ok(existing) = load_account_in(storage_dir, &account.id) {
        account.created_at = existing.created_at;
        if account.quota_query_last_error.is_none() {
            // preserve error info if refresh didn't update it
        }
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
) -> Result<KiroAccountSummary, String> {
    let mut account = load_account_in(storage_dir, account_id)?;

    let profile_arn = extract_profile_arn(
        account.kiro_auth_token_raw.as_ref(),
        account.kiro_profile_raw.as_ref(),
    );

    if let Some(arn) = profile_arn.as_deref() {
        match fetch_and_apply_usage(&mut account, arn).await {
            Ok(()) => {}
            Err(e) => {
                let is_banned = e.starts_with("BANNED:");
                let msg = if is_banned {
                    e.trim_start_matches("BANNED:").to_string()
                } else {
                    e
                };
                account.quota_query_last_error = Some(msg.clone());
                account.quota_query_last_error_at = Some(now_timestamp_ms());
                if is_banned {
                    account.status = Some("banned".to_string());
                    account.status_reason = Some(msg);
                }
            }
        }
    } else {
        account.quota_query_last_error =
            Some("Cannot refresh: no profile ARN in stored credentials.".to_string());
        account.quota_query_last_error_at = Some(now_timestamp_ms());
    }

    account.last_used = now_timestamp();
    save_account_in(storage_dir, &account)?;
    Ok(account.to_summary())
}

fn build_account_id(email: &str, profile_arn: Option<&str>) -> String {
    let identity = format!(
        "{}:{}",
        email.trim().to_ascii_lowercase(),
        profile_arn.unwrap_or_default().trim(),
    );
    format!("kiro_{:x}", md5::compute(identity.as_bytes()))
}

impl StoredKiroAccount {
    fn to_summary(&self) -> KiroAccountSummary {
        KiroAccountSummary {
            id: self.id.clone(),
            email: self.email.clone(),
            login_provider: self.login_provider.clone(),
            plan_name: resolve_plan_display(self),
            credits_total: self.credits_total,
            credits_used: self.credits_used,
            bonus_total: self.bonus_total,
            bonus_used: self.bonus_used,
            usage_reset_at: self.usage_reset_at,
            bonus_expire_days: self.bonus_expire_days,
            status: self.status.clone(),
            status_reason: self.status_reason.clone(),
            quota_query_last_error: self.quota_query_last_error.clone(),
            quota_query_last_error_at: self.quota_query_last_error_at,
            usage_updated_at: self.usage_updated_at,
            created_at: self.created_at,
            last_used: self.last_used,
        }
    }
}

// ── Local file import ───────────────────────────────────────────────────────

fn local_auth_token_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not locate home directory".to_string())?;
    Ok(home
        .join(".aws")
        .join("sso")
        .join("cache")
        .join("kiro-auth-token.json"))
}

fn local_profile_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not locate home directory".to_string())?;

    #[cfg(target_os = "linux")]
    return Ok(home.join(".config/Kiro/User/globalStorage/kiro.kiroagent/profile.json"));

    #[cfg(target_os = "macos")]
    return Ok(home
        .join("Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/profile.json"));

    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA")
            .map_err(|_| "Could not read APPDATA environment variable".to_string())?;
        return Ok(
            PathBuf::from(appdata).join("Kiro/User/globalStorage/kiro.kiroagent/profile.json")
        );
    }

    #[allow(unreachable_code)]
    Err("Unsupported platform for Kiro local profile".to_string())
}

fn read_local_auth_token() -> Result<Option<Value>, String> {
    let path = local_auth_token_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read {}: {}", path.display(), e))?;
    let parsed = serde_json::from_str(&raw)
        .map_err(|e| format!("Could not parse {}: {}", path.display(), e))?;
    Ok(Some(parsed))
}

fn read_local_profile_json() -> Result<Option<Value>, String> {
    let path = match local_profile_path() {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read {}: {}", path.display(), e))?;
    let parsed = serde_json::from_str(&raw)
        .map_err(|e| format!("Could not parse {}: {}", path.display(), e))?;
    Ok(Some(parsed))
}

// ── Value extraction helpers ────────────────────────────────────────────────

fn get_path<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for key in path {
        current = current.as_object()?.get(*key)?;
    }
    Some(current)
}

fn pick_string(root: Option<&Value>, paths: &[&[&str]]) -> Option<String> {
    let root = root?;
    for path in paths {
        if let Some(val) = get_path(root, path) {
            if let Some(text) = val.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn pick_number(root: Option<&Value>, paths: &[&[&str]]) -> Option<f64> {
    let root = root?;
    for path in paths {
        if let Some(val) = get_path(root, path) {
            if let Some(n) = val.as_f64().filter(|n| n.is_finite()) {
                return Some(n);
            }
            if let Some(text) = val.as_str() {
                if let Ok(n) = text.trim().parse::<f64>() {
                    if n.is_finite() {
                        return Some(n);
                    }
                }
            }
        }
    }
    None
}

fn pick_timestamp(root: Option<&Value>, paths: &[&[&str]]) -> Option<i64> {
    let root = root?;
    for path in paths {
        if let Some(val) = get_path(root, path) {
            if let Some(ts) = parse_ts_value(val) {
                return Some(ts);
            }
        }
    }
    None
}

fn parse_ts_value(val: &Value) -> Option<i64> {
    match val {
        Value::Number(n) => {
            let raw = n.as_i64().or_else(|| n.as_f64().map(|f| f as i64))?;
            normalize_ts(raw)
        }
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Ok(n) = trimmed.parse::<i64>() {
                return normalize_ts(n);
            }
            chrono::DateTime::parse_from_rfc3339(trimmed)
                .ok()
                .map(|dt| dt.timestamp())
        }
        _ => None,
    }
}

fn normalize_ts(raw: i64) -> Option<i64> {
    if raw <= 0 {
        return None;
    }
    // milliseconds → seconds
    if raw > 10_000_000_000 {
        Some(raw / 1000)
    } else {
        Some(raw)
    }
}

fn decode_jwt_email(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .ok()?;
    let claims: Value = serde_json::from_slice(&decoded).ok()?;
    pick_string(
        Some(&claims),
        &[&["email"], &["upn"], &["preferred_username"]],
    )
    .filter(|e| e.contains('@'))
}
