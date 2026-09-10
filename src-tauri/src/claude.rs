//! Tauri command surface for the Claude provider. All logic lives in
//! `quota_core::claude`; this file only re-exports it and attaches the
//! `#[tauri::command]` attribute the Tauri handler generator needs.

pub use quota_core::claude::{
    apply_claude_token_response_for_test, build_claude_oauth_start_for_test,
    classify_claude_refresh_failure_for_test, parse_claude_callback_input_for_test,
    parse_claude_quota_for_test, ClaudeAccountIndex, ClaudeAccountSummary,
    ClaudeOAuthStartResponse,
};

#[tauri::command]
pub fn list_claude_accounts() -> Result<Vec<ClaudeAccountSummary>, String> {
    quota_core::claude::list_claude_accounts()
}

#[tauri::command]
pub fn claude_oauth_login_start() -> Result<ClaudeOAuthStartResponse, String> {
    quota_core::claude::claude_oauth_login_start()
}

#[tauri::command]
pub async fn claude_oauth_login_complete(
    login_id: String,
    callback_or_code: String,
    email_hint: Option<String>,
) -> Result<ClaudeAccountSummary, String> {
    quota_core::claude::claude_oauth_login_complete(login_id, callback_or_code, email_hint).await
}

#[tauri::command]
pub fn claude_oauth_login_cancel(login_id: Option<String>) -> Result<(), String> {
    quota_core::claude::claude_oauth_login_cancel(login_id)
}

#[tauri::command]
pub async fn refresh_claude_account(account_id: String) -> Result<ClaudeAccountSummary, String> {
    quota_core::claude::refresh_claude_account(account_id).await
}

#[tauri::command]
pub async fn refresh_all_claude_accounts() -> Result<Vec<ClaudeAccountSummary>, String> {
    quota_core::claude::refresh_all_claude_accounts().await
}

#[tauri::command]
pub fn delete_claude_account(account_id: String) -> Result<(), String> {
    quota_core::claude::delete_claude_account(account_id)
}
