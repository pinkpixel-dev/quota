use serde::Serialize;

pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod cursor;
mod github_copilot;
pub mod kiro;

#[derive(Serialize)]
struct AppStatus {
    app_name: &'static str,
    reference_directory: &'static str,
    sidecar_enabled: bool,
    electron_helper_enabled: bool,
}

#[tauri::command]
fn get_app_status() -> AppStatus {
    AppStatus {
        app_name: "Quota",
        reference_directory: "ORIGINAL/",
        sidecar_enabled: false,
        electron_helper_enabled: false,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_app_status,
            github_copilot::github_copilot_oauth_login_start,
            github_copilot::github_copilot_oauth_login_complete,
            github_copilot::github_copilot_oauth_login_cancel,
            github_copilot::list_github_copilot_accounts,
            github_copilot::refresh_github_copilot_account,
            github_copilot::refresh_all_github_copilot_accounts,
            github_copilot::delete_github_copilot_account,
            codex::list_codex_accounts,
            codex::import_codex_from_local,
            codex::codex_oauth_login_start,
            codex::codex_oauth_login_complete,
            codex::codex_oauth_login_cancel,
            codex::refresh_codex_account,
            codex::refresh_all_codex_accounts,
            codex::delete_codex_account,
            antigravity::list_antigravity_accounts,
            antigravity::import_antigravity_from_local,
            antigravity::antigravity_oauth_login_start,
            antigravity::antigravity_oauth_login_complete,
            antigravity::antigravity_oauth_login_cancel,
            antigravity::refresh_antigravity_account,
            antigravity::refresh_all_antigravity_accounts,
            antigravity::delete_antigravity_account,
            claude::list_claude_accounts,
            claude::claude_oauth_login_start,
            claude::claude_oauth_login_complete,
            claude::claude_oauth_login_cancel,
            claude::refresh_claude_account,
            claude::refresh_all_claude_accounts,
            claude::delete_claude_account,
            kiro::list_kiro_accounts,
            kiro::import_kiro_from_local,
            kiro::kiro_oauth_login_start,
            kiro::kiro_oauth_login_complete,
            kiro::kiro_oauth_login_cancel,
            kiro::kiro_oauth_submit_callback_url,
            kiro::refresh_kiro_account,
            kiro::refresh_all_kiro_accounts,
            kiro::delete_kiro_account,
            cursor::list_cursor_accounts,
            cursor::import_cursor_from_local,
            cursor::cursor_oauth_login_start,
            cursor::cursor_oauth_login_complete,
            cursor::cursor_oauth_login_cancel,
            cursor::refresh_cursor_account,
            cursor::refresh_all_cursor_accounts,
            cursor::delete_cursor_account
        ])
        .run(tauri::generate_context!())
        .expect("error while running Quota");
}
