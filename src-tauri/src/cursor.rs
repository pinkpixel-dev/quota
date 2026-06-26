use base64::Engine;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const DATA_DIR: &str = ".quota";
const ACCOUNTS_DIR: &str = "cursor_accounts";
const ACCOUNTS_INDEX_FILE: &str = "cursor_accounts.json";

const CURSOR_LOGIN_URL: &str = "https://cursor.com/loginDeepControl";
const CURSOR_POLL_URL: &str = "https://api2.cursor.sh/auth/poll";
const CURSOR_USAGE_URL: &str = "https://cursor.com/api/usage-summary";
const CURSOR_USER_META_URL: &str = "https://api2.cursor.sh/aiserver.v1.AuthService/GetUserMeta";
const CURSOR_FULL_STRIPE_URL: &str = "https://api2.cursor.sh/auth/full_stripe_profile";
const CURSOR_STRIPE_URL: &str = "https://api2.cursor.sh/auth/stripe_profile";
const CURSOR_OAUTH_TOKEN_URL: &str = "https://api2.cursor.sh/oauth/token";
const CURSOR_AUTH_CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";
const CURSOR_LOCAL_DB_PATH: &str = ".config/Cursor/User/globalStorage/state.vscdb";

const POLL_INTERVAL_MS: u64 = 2000;
const POLL_MAX_ATTEMPTS: u32 = 150;
const OAUTH_TIMEOUT_SECONDS: i64 = 300;

// ---------------------------------------------------------------------------
// Pending OAuth state
// ---------------------------------------------------------------------------

static PENDING_CURSOR_OAUTH: std::sync::LazyLock<Arc<Mutex<Option<PendingCursorOAuth>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(None)));

#[derive(Debug, Clone)]
struct PendingCursorOAuth {
    login_id: String,
    uuid: String,
    code_verifier: String,
    expires_at: i64,
    cancelled: bool,
}

// ---------------------------------------------------------------------------
// Disk storage types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCursorAccount {
    id: String,
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sign_up_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    membership_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subscription_status: Option<String>,
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_percent: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_percent: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_percent: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    billing_cycle_end: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan_used: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan_limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on_demand_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on_demand_used: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on_demand_limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quota_query_last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage_updated_at: Option<i64>,
    created_at: i64,
    last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CursorAccountIndex {
    version: String,
    account_ids: Vec<String>,
}

impl CursorAccountIndex {
    fn new() -> Self {
        Self { version: "1.0".to_string(), account_ids: Vec::new() }
    }
}

// ---------------------------------------------------------------------------
// Public summary sent to React (no raw tokens)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorAccountSummary {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign_up_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub membership_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_status: Option<String>,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_percent: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_percent: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_percent: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_cycle_end: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_used: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_used: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_demand_limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota_query_last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_updated_at: Option<i64>,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorOAuthStartResponse {
    pub login_id: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval_seconds: u64,
}

// ---------------------------------------------------------------------------
// HTTP response types
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PollResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    auth_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserMetaResponse {
    email: Option<String>,
    sign_up_type: Option<String>,
    workos_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StripeProfileResponse {
    membership_type: Option<String>,
    individual_membership_type: Option<String>,
    subscription_status: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefreshTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    #[serde(default, alias = "shouldLogout")]
    should_logout: bool,
}

// ---------------------------------------------------------------------------
// Utility: timestamps and identity
// ---------------------------------------------------------------------------

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn now_ts_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn build_account_id(email: &str, access_token: &str) -> String {
    let key = if email.contains('@') {
        email.to_string()
    } else {
        format!("__tok__{:x}", md5::compute(access_token.as_bytes()))
    };
    format!("cursor_{:x}", md5::compute(key.as_bytes()))
}

fn normalize_email(email: &str) -> String {
    let trimmed = email.trim().to_lowercase();
    if trimmed.contains('@') { trimmed } else { String::new() }
}

fn normalize_str(s: Option<&str>) -> Option<String> {
    s.and_then(|v| {
        let t = v.trim();
        if t.is_empty() { None } else { Some(t.to_string()) }
    })
}

// ---------------------------------------------------------------------------
// Utility: PKCE + UUID
// ---------------------------------------------------------------------------

fn generate_code_verifier() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen::<u8>()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

fn generate_uuid() -> String {
    let mut rng = rand::thread_rng();
    let b: [u8; 16] = rng.gen();
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3],
        b[4], b[5],
        (b[6] & 0x0f) | 0x40, b[7],
        (b[8] & 0x3f) | 0x80, b[9],
        b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

// ---------------------------------------------------------------------------
// Utility: JWT decode for session cookie
// ---------------------------------------------------------------------------

fn decode_jwt_payload(jwt: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = jwt.splitn(3, '.').collect();
    if parts.len() < 2 {
        return None;
    }
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn extract_workos_user_id(access_token: &str) -> Option<String> {
    let payload = decode_jwt_payload(access_token)?;
    let sub = payload.get("sub")?.as_str()?;
    let user_id = sub.rsplit('|').next().unwrap_or(sub);
    if user_id.starts_with("user_") {
        Some(user_id.to_string())
    } else {
        None
    }
}

fn build_session_cookie(access_token: &str) -> Option<String> {
    let user_id = extract_workos_user_id(access_token)?;
    Some(format!("WorkosCursorSessionToken={}%3A%3A{}", user_id, access_token))
}

// ---------------------------------------------------------------------------
// Utility: usage JSON parsing
// ---------------------------------------------------------------------------

fn pick_f64(obj: Option<&serde_json::Value>, keys: &[&str]) -> Option<f64> {
    let obj = obj?.as_object()?;
    for key in keys {
        if let Some(val) = obj.get(*key) {
            if let Some(n) = val.as_f64() {
                if n.is_finite() {
                    return Some(n);
                }
            }
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.trim().parse::<f64>() {
                    if n.is_finite() {
                        return Some(n);
                    }
                }
            }
        }
    }
    None
}

fn pick_pct(obj: Option<&serde_json::Value>, keys: &[&str]) -> Option<i32> {
    let n = pick_f64(obj, keys)?;
    Some(n.round().clamp(0.0, 100.0) as i32)
}

fn pick_bool_field(obj: Option<&serde_json::Value>, keys: &[&str]) -> Option<bool> {
    let obj = obj?.as_object()?;
    for key in keys {
        if let Some(val) = obj.get(*key) {
            if let Some(b) = val.as_bool() {
                return Some(b);
            }
        }
    }
    None
}

fn get_path<'a>(root: &'a serde_json::Value, parts: &[&str]) -> Option<&'a serde_json::Value> {
    let mut cur = root;
    for part in parts {
        cur = cur.as_object()?.get(*part)?;
    }
    Some(cur)
}

fn apply_usage(account: &mut StoredCursorAccount, raw: &serde_json::Value) {
    let plan = get_path(raw, &["individualUsage", "plan"])
        .or_else(|| get_path(raw, &["individual_usage", "plan"]))
        .or_else(|| raw.get("planUsage"))
        .or_else(|| raw.get("plan_usage"));

    account.total_percent = pick_pct(plan, &["totalPercentUsed", "total_percent_used"]);
    account.auto_percent = pick_pct(plan, &["autoPercentUsed", "auto_percent_used"]);
    account.api_percent = pick_pct(plan, &["apiPercentUsed", "api_percent_used"]);
    account.plan_used = pick_f64(plan, &["used", "totalSpend", "total_spend"]);
    account.plan_limit = pick_f64(plan, &["limit"]);

    let on_demand = get_path(raw, &["individualUsage", "onDemand"])
        .or_else(|| get_path(raw, &["individual_usage", "onDemand"]))
        .or_else(|| raw.get("spendLimitUsage"))
        .or_else(|| raw.get("spend_limit_usage"));

    account.on_demand_used = pick_f64(
        on_demand,
        &["used", "totalSpend", "total_spend", "individualUsed", "individual_used"],
    );
    account.on_demand_limit = pick_f64(
        on_demand,
        &["limit", "individualLimit", "individual_limit", "pooledLimit", "pooled_limit"],
    );
    account.on_demand_enabled = pick_bool_field(on_demand, &["enabled"]);

    if let Some(end_raw) = raw.get("billingCycleEnd").or_else(|| raw.get("billing_cycle_end")) {
        if let Some(s) = end_raw.as_str() {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                account.billing_cycle_end = Some(dt.timestamp());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Utility: membership resolution
// ---------------------------------------------------------------------------

fn resolve_membership(profile: &StripeProfileResponse) -> Option<String> {
    let membership = normalize_str(profile.membership_type.as_deref());
    let individual = normalize_str(profile.individual_membership_type.as_deref());

    if let Some(ref ind) = individual {
        if !ind.eq_ignore_ascii_case("free")
            && !matches!(membership.as_deref(), Some(m) if m.eq_ignore_ascii_case("enterprise"))
        {
            return Some(ind.clone());
        }
    }

    membership.or(individual)
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

fn quota_storage_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not locate home directory".to_string())?;
    let dir = home.join(DATA_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create Quota data directory: {}", e))?;
    Ok(dir)
}

fn accounts_dir_in(storage_dir: &Path) -> PathBuf {
    storage_dir.join(ACCOUNTS_DIR)
}

fn index_path_in(storage_dir: &Path) -> PathBuf {
    storage_dir.join(ACCOUNTS_INDEX_FILE)
}

fn account_path_in(storage_dir: &Path, id: &str) -> PathBuf {
    accounts_dir_in(storage_dir).join(format!("{}.json", id))
}

fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "No parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("Could not create directory: {}", e))?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("cursor"),
        std::process::id()
    ));
    fs::write(&tmp, content).map_err(|e| format!("Could not write temp file: {}", e))?;
    fs::rename(&tmp, path).map_err(|e| format!("Could not rename temp file: {}", e))?;
    Ok(())
}

fn load_index_in(storage_dir: &Path) -> Result<CursorAccountIndex, String> {
    let path = index_path_in(storage_dir);
    if !path.exists() {
        return Ok(CursorAccountIndex::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read Cursor account index: {}", e))?;
    if content.trim().is_empty() {
        return Ok(CursorAccountIndex::new());
    }
    serde_json::from_str(&content).map_err(|e| format!("Could not parse Cursor account index: {}", e))
}

fn save_index_in(storage_dir: &Path, index: &CursorAccountIndex) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index)
        .map_err(|e| format!("Could not encode Cursor account index: {}", e))?;
    write_atomic(&index_path_in(storage_dir), &content)
}

fn load_account_in(storage_dir: &Path, id: &str) -> Result<StoredCursorAccount, String> {
    let content = fs::read_to_string(account_path_in(storage_dir, id))
        .map_err(|e| format!("Could not read Cursor account: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Could not parse Cursor account: {}", e))
}

fn save_account_in(storage_dir: &Path, account: &StoredCursorAccount) -> Result<(), String> {
    let content = serde_json::to_string_pretty(account)
        .map_err(|e| format!("Could not encode Cursor account: {}", e))?;
    write_atomic(&account_path_in(storage_dir, &account.id), &content)
}

fn upsert_account_in(storage_dir: &Path, mut account: StoredCursorAccount) -> Result<CursorAccountSummary, String> {
    let mut index = load_index_in(storage_dir)?;
    if let Ok(existing) = load_account_in(storage_dir, &account.id) {
        account.created_at = existing.created_at;
    }
    account.last_used = now_ts();
    if !index.account_ids.iter().any(|id| id == &account.id) {
        index.account_ids.insert(0, account.id.clone());
    }
    save_account_in(storage_dir, &account)?;
    save_index_in(storage_dir, &index)?;
    Ok(account.to_summary())
}

fn list_accounts_in(storage_dir: &Path) -> Result<Vec<CursorAccountSummary>, String> {
    let index = load_index_in(storage_dir)?;
    Ok(index
        .account_ids
        .iter()
        .filter_map(|id| load_account_in(storage_dir, id).ok())
        .map(|a| a.to_summary())
        .collect())
}

impl StoredCursorAccount {
    fn to_summary(&self) -> CursorAccountSummary {
        CursorAccountSummary {
            id: self.id.clone(),
            email: self.email.clone(),
            auth_id: self.auth_id.clone(),
            sign_up_type: self.sign_up_type.clone(),
            membership_type: self.membership_type.clone(),
            subscription_status: self.subscription_status.clone(),
            source: self.source.clone(),
            total_percent: self.total_percent,
            auto_percent: self.auto_percent,
            api_percent: self.api_percent,
            billing_cycle_end: self.billing_cycle_end,
            plan_used: self.plan_used,
            plan_limit: self.plan_limit,
            on_demand_enabled: self.on_demand_enabled,
            on_demand_used: self.on_demand_used,
            on_demand_limit: self.on_demand_limit,
            quota_query_last_error: self.quota_query_last_error.clone(),
            usage_updated_at: self.usage_updated_at,
            created_at: self.created_at,
            last_used: self.last_used,
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP client
// ---------------------------------------------------------------------------

fn build_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

// ---------------------------------------------------------------------------
// API calls
// ---------------------------------------------------------------------------

async fn fetch_user_meta(client: &reqwest::Client, access_token: &str) -> Result<UserMetaResponse, String> {
    let resp = client
        .post(CURSOR_USER_META_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| format!("Cursor user meta request failed: {}", e))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err("cursor_auth_expired".to_string());
    }
    if status != 200 {
        return Err(format!("Cursor user meta returned status {}", status));
    }

    let body = resp.text().await.map_err(|e| format!("Failed to read user meta response: {}", e))?;
    serde_json::from_str::<UserMetaResponse>(&body)
        .map_err(|e| format!("Failed to parse user meta JSON: {}", e))
}

async fn fetch_stripe_profile(client: &reqwest::Client, access_token: &str) -> Option<StripeProfileResponse> {
    // Try full profile first, fall back to basic profile
    for url in [CURSOR_FULL_STRIPE_URL, CURSOR_STRIPE_URL] {
        let resp = client
            .get(url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Accept", "application/json")
            .send()
            .await
            .ok()?;

        if resp.status().as_u16() == 200 {
            let body = resp.text().await.ok()?;
            // The basic profile may return a plain string for pro subscribers
            if let Ok(profile) = serde_json::from_str::<StripeProfileResponse>(&body) {
                return Some(profile);
            }
            // If it's a non-empty string, treat as pro
            if let Ok(serde_json::Value::String(s)) = serde_json::from_str::<serde_json::Value>(&body) {
                if !s.trim().is_empty() {
                    return Some(StripeProfileResponse {
                        membership_type: Some("pro".to_string()),
                        individual_membership_type: None,
                        subscription_status: None,
                    });
                }
            }
        }
    }
    None
}

async fn exchange_refresh_token(client: &reqwest::Client, refresh_token: &str) -> Result<RefreshTokenResponse, String> {
    let resp = client
        .post(CURSOR_OAUTH_TOKEN_URL)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": CURSOR_AUTH_CLIENT_ID,
            "refresh_token": refresh_token,
        }))
        .send()
        .await
        .map_err(|e| format!("Cursor token refresh request failed: {}", e))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err("cursor_refresh_expired".to_string());
    }
    if status != 200 {
        return Err(format!("Cursor token refresh returned status {}", status));
    }

    let body = resp.text().await.map_err(|e| format!("Failed to read token refresh response: {}", e))?;
    serde_json::from_str::<RefreshTokenResponse>(&body)
        .map_err(|e| format!("Failed to parse token refresh JSON: {}", e))
}

async fn fetch_usage_raw(client: &reqwest::Client, access_token: &str) -> Result<serde_json::Value, String> {
    let cookie = build_session_cookie(access_token)
        .ok_or_else(|| "cursor_auth_expired".to_string())?;

    let resp = client
        .get(CURSOR_USAGE_URL)
        .header("Accept", "application/json")
        .header("Cookie", &cookie)
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
        .send()
        .await
        .map_err(|e| format!("Cursor usage request failed: {}", e))?;

    let status = resp.status().as_u16();
    if status == 401 || status == 403 {
        return Err("cursor_auth_expired".to_string());
    }
    if status != 200 {
        return Err(format!("Cursor usage API returned status {}", status));
    }

    let body = resp.text().await.map_err(|e| format!("Failed to read usage response: {}", e))?;
    serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|e| format!("Failed to parse usage JSON: {}", e))
}

// ---------------------------------------------------------------------------
// Core refresh logic (shared by import/oauth/refresh commands)
// ---------------------------------------------------------------------------

async fn do_refresh_account(mut account: StoredCursorAccount) -> StoredCursorAccount {
    let client = match build_http_client() {
        Ok(c) => c,
        Err(e) => {
            account.quota_query_last_error = Some(e);
            account.last_used = now_ts();
            return account;
        }
    };

    // Fetch user meta (best-effort)
    match fetch_user_meta(&client, &account.access_token).await {
        Ok(meta) => {
            if let Some(email) = normalize_str(meta.email.as_deref()).filter(|e| e.contains('@')) {
                account.email = email;
            }
            if let Some(workos_id) = normalize_str(meta.workos_id.as_deref()) {
                account.auth_id = Some(workos_id);
            }
            if let Some(sign_up_type) = normalize_str(meta.sign_up_type.as_deref()) {
                account.sign_up_type = Some(sign_up_type);
            }
        }
        Err(e) if e == "cursor_auth_expired" => {
            // Will be handled below during usage fetch
        }
        Err(_) => {}
    }

    // Fetch stripe profile (best-effort)
    if let Some(profile) = fetch_stripe_profile(&client, &account.access_token).await {
        if let Some(membership) = resolve_membership(&profile) {
            account.membership_type = Some(membership);
        }
        if let Some(status) = normalize_str(profile.subscription_status.as_deref()) {
            account.subscription_status = Some(status);
        }
    }

    // Fetch usage — retry once after token refresh if auth expired
    let usage_result = fetch_usage_raw(&client, &account.access_token).await;
    match usage_result {
        Ok(raw) => {
            apply_usage(&mut account, &raw);
            account.quota_query_last_error = None;
            account.usage_updated_at = Some(now_ts_millis());
        }
        Err(ref e) if e == "cursor_auth_expired" => {
            let refreshed = if let Some(ref rt) = account.refresh_token.clone() {
                match exchange_refresh_token(&client, rt).await {
                    Ok(tokens) if !tokens.should_logout => {
                        if let (Some(new_at), Some(new_rt)) = (tokens.access_token, tokens.refresh_token) {
                            account.access_token = new_at.clone();
                            account.refresh_token = Some(new_rt);
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            } else {
                false
            };

            if refreshed {
                match fetch_usage_raw(&client, &account.access_token).await {
                    Ok(raw) => {
                        apply_usage(&mut account, &raw);
                        account.quota_query_last_error = None;
                        account.usage_updated_at = Some(now_ts_millis());
                    }
                    Err(e) => {
                        account.quota_query_last_error = Some(e);
                    }
                }
            } else {
                account.quota_query_last_error =
                    Some("Cursor session expired. Re-import or reconnect your account.".to_string());
            }
        }
        Err(e) => {
            account.quota_query_last_error = Some(e);
        }
    }

    account.last_used = now_ts();
    account
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_cursor_accounts() -> Result<Vec<CursorAccountSummary>, String> {
    let dir = quota_storage_dir()?;
    list_accounts_in(&dir)
}

#[tauri::command]
pub async fn import_cursor_from_local() -> Result<CursorAccountSummary, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not locate home directory".to_string())?;
    let db_path = home.join(CURSOR_LOCAL_DB_PATH);

    if !db_path.exists() {
        return Err(format!(
            "Cursor local database not found at ~/{CURSOR_LOCAL_DB_PATH}. Sign in to Cursor first."
        ));
    }

    let (access_token, refresh_token, email, auth_id, membership_type) = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| format!("Failed to open Cursor database: {}", e))?;

        let mut stmt = conn
            .prepare("SELECT key, value FROM ItemTable WHERE key LIKE 'cursorAuth/%'")
            .map_err(|e| format!("Failed to prepare SQLite query: {}", e))?;

        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| format!("Failed to query Cursor database: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        let mut access_token = String::new();
        let mut refresh_token: Option<String> = None;
        let mut email = String::new();
        let mut auth_id: Option<String> = None;
        let mut membership_type: Option<String> = None;

        for (key, value) in &rows {
            let v = value.trim().to_string();
            match key.as_str() {
                "cursorAuth/accessToken" => access_token = v,
                "cursorAuth/refreshToken" => refresh_token = Some(v).filter(|s| !s.is_empty()),
                "cursorAuth/cachedEmail" => email = normalize_email(&v),
                "cursorAuth/authId" => auth_id = Some(v).filter(|s| !s.is_empty()),
                "cursorAuth/stripeMembershipType" => membership_type = Some(v).filter(|s| !s.is_empty()),
                _ => {}
            }
        }

        Ok::<_, String>((access_token, refresh_token, email, auth_id, membership_type))
    })
    .await
    .map_err(|e| format!("Database task panicked: {}", e))??;

    if access_token.is_empty() {
        return Err(
            "No Cursor access token found in local database. Sign in to Cursor first.".to_string()
        );
    }

    let now = now_ts();
    let mut account = StoredCursorAccount {
        id: String::new(), // set after meta fetch
        email,
        auth_id,
        sign_up_type: None,
        membership_type,
        subscription_status: None,
        access_token,
        refresh_token,
        source: "local".to_string(),
        total_percent: None,
        auto_percent: None,
        api_percent: None,
        billing_cycle_end: None,
        plan_used: None,
        plan_limit: None,
        on_demand_enabled: None,
        on_demand_used: None,
        on_demand_limit: None,
        quota_query_last_error: None,
        usage_updated_at: None,
        created_at: now,
        last_used: now,
    };

    account = do_refresh_account(account).await;

    let id_email = if account.email.contains('@') {
        account.email.clone()
    } else {
        format!("__tok__{:x}", md5::compute(account.access_token.as_bytes()))
    };
    account.id = build_account_id(&id_email, &account.access_token);

    let dir = quota_storage_dir()?;
    upsert_account_in(&dir, account)
}

#[tauri::command]
pub fn cursor_oauth_login_start() -> Result<CursorOAuthStartResponse, String> {
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    let uuid = generate_uuid();
    let login_id = uuid.clone();

    let verification_uri = format!(
        "{}?challenge={}&uuid={}&mode=login",
        CURSOR_LOGIN_URL, code_challenge, uuid
    );

    let now = now_ts();
    let state = PendingCursorOAuth {
        login_id: login_id.clone(),
        uuid,
        code_verifier,
        expires_at: now + OAUTH_TIMEOUT_SECONDS,
        cancelled: false,
    };

    if let Ok(mut guard) = PENDING_CURSOR_OAUTH.lock() {
        *guard = Some(state);
    }

    Ok(CursorOAuthStartResponse {
        login_id,
        verification_uri,
        expires_in: OAUTH_TIMEOUT_SECONDS as u64,
        interval_seconds: POLL_INTERVAL_MS / 1000,
    })
}

#[tauri::command]
pub async fn cursor_oauth_login_complete(login_id: String) -> Result<CursorAccountSummary, String> {
    let (uuid, code_verifier) = {
        let guard = PENDING_CURSOR_OAUTH
            .lock()
            .map_err(|_| "Could not lock OAuth state".to_string())?;

        let state = guard
            .as_ref()
            .ok_or_else(|| "No Cursor login session in progress".to_string())?;

        if state.login_id != login_id {
            return Err(format!(
                "login_id mismatch: expected {}, got {}",
                state.login_id, login_id
            ));
        }
        if state.cancelled {
            return Err("Login was cancelled".to_string());
        }
        if now_ts() > state.expires_at {
            return Err("Login session expired".to_string());
        }

        (state.uuid.clone(), state.code_verifier.clone())
    };

    let client = build_http_client()?;
    let poll_url = format!("{}?uuid={}&verifier={}", CURSOR_POLL_URL, uuid, code_verifier);

    for attempt in 0..POLL_MAX_ATTEMPTS {
        {
            if let Ok(guard) = PENDING_CURSOR_OAUTH.lock() {
                if let Some(ref s) = *guard {
                    if s.cancelled {
                        return Err("Login was cancelled".to_string());
                    }
                    if now_ts() > s.expires_at {
                        return Err("Login session expired".to_string());
                    }
                }
            }
        }

        let resp = client
            .get(&poll_url)
            .header("Accept", "application/json")
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status().as_u16();

                if status == 404 {
                    if attempt % 15 == 0 {
                        eprintln!("[Cursor OAuth] Waiting for login... attempt={}", attempt);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
                    continue;
                }

                if status != 200 {
                    tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
                    continue;
                }

                let body = r.text().await.map_err(|e| format!("Failed to read poll response: {}", e))?;
                let poll: PollResponse =
                    serde_json::from_str(&body).map_err(|e| format!("Failed to parse poll response: {}", e))?;

                if let (Some(access_token), Some(refresh_token)) = (poll.access_token, poll.refresh_token) {
                    if let Ok(mut guard) = PENDING_CURSOR_OAUTH.lock() {
                        *guard = None;
                    }

                    // auth_id from poll may be an email for some sign-in types
                    let email_from_poll = poll
                        .auth_id
                        .as_deref()
                        .filter(|v| v.contains('@'))
                        .map(normalize_email)
                        .unwrap_or_default();

                    let now = now_ts();
                    let mut account = StoredCursorAccount {
                        id: String::new(),
                        email: email_from_poll,
                        auth_id: poll.auth_id,
                        sign_up_type: None,
                        membership_type: None,
                        subscription_status: None,
                        access_token,
                        refresh_token: Some(refresh_token),
                        source: "oauth".to_string(),
                        total_percent: None,
                        auto_percent: None,
                        api_percent: None,
                        billing_cycle_end: None,
                        plan_used: None,
                        plan_limit: None,
                        on_demand_enabled: None,
                        on_demand_used: None,
                        on_demand_limit: None,
                        quota_query_last_error: None,
                        usage_updated_at: None,
                        created_at: now,
                        last_used: now,
                    };

                    account = do_refresh_account(account).await;

                    let id_email = if account.email.contains('@') {
                        account.email.clone()
                    } else {
                        format!("__tok__{:x}", md5::compute(account.access_token.as_bytes()))
                    };
                    account.id = build_account_id(&id_email, &account.access_token);

                    let dir = quota_storage_dir()?;
                    return upsert_account_in(&dir, account);
                }

                tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
            }
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS * 2)).await;
            }
        }
    }

    if let Ok(mut guard) = PENDING_CURSOR_OAUTH.lock() {
        *guard = None;
    }

    Err("Cursor login timed out. Please try again.".to_string())
}

#[tauri::command]
pub fn cursor_oauth_login_cancel(login_id: Option<String>) -> Result<(), String> {
    if let Ok(mut guard) = PENDING_CURSOR_OAUTH.lock() {
        if let Some(ref mut state) = *guard {
            if login_id.is_none() || login_id.as_deref() == Some(state.login_id.as_str()) {
                state.cancelled = true;
            }
        }
        *guard = None;
    }
    Ok(())
}

#[tauri::command]
pub async fn refresh_cursor_account(account_id: String) -> Result<CursorAccountSummary, String> {
    let dir = quota_storage_dir()?;
    let account = load_account_in(&dir, &account_id)?;
    let refreshed = do_refresh_account(account).await;
    upsert_account_in(&dir, refreshed)
}

#[tauri::command]
pub async fn refresh_all_cursor_accounts() -> Result<Vec<CursorAccountSummary>, String> {
    let dir = quota_storage_dir()?;
    let index = load_index_in(&dir)?;
    for id in &index.account_ids {
        if let Ok(account) = load_account_in(&dir, id) {
            let refreshed = do_refresh_account(account).await;
            let _ = upsert_account_in(&dir, refreshed);
        }
    }
    list_accounts_in(&dir)
}

#[tauri::command]
pub fn delete_cursor_account(account_id: String) -> Result<(), String> {
    let dir = quota_storage_dir()?;
    let mut index = load_index_in(&dir)?;
    index.account_ids.retain(|id| id != &account_id);
    save_index_in(&dir, &index)?;
    let path = account_path_in(&dir, &account_id);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Could not delete Cursor account: {}", e))?;
    }
    Ok(())
}
