use serde::Deserialize;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Emitter, Manager, State, Wry,
};

const REFRESH_MENU_ID: &str = "refresh-usage";
const SHOW_MENU_ID: &str = "show";
const QUIT_MENU_ID: &str = "quit";
const USAGE_MENU_ID_PREFIX: &str = "usage-";
const MAX_USAGE_ROWS: usize = 30;
const MAX_ROW_CHARACTERS: usize = 140;

pub struct TrayMenuState {
    menu: Menu<Wry>,
    update_lock: Mutex<()>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuSnapshot {
    rows: Vec<String>,
    refreshing: bool,
}

struct NormalizedRows {
    rows: Vec<String>,
    hidden_count: usize,
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    let menu = Menu::new(app)?;
    rebuild_menu(
        app.handle(),
        &menu,
        &TrayMenuSnapshot {
            rows: Vec::new(),
            refreshing: false,
        },
    )?;

    let mut tray_builder = TrayIconBuilder::with_id("quota-main")
        .menu(&menu)
        .tooltip("Quota")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            REFRESH_MENU_ID => {
                let _ = app.emit("tray-refresh-requested", ());
            }
            SHOW_MENU_ID => show_main_window(app),
            QUIT_MENU_ID => app.exit(0),
            id if id.starts_with(USAGE_MENU_ID_PREFIX) => show_main_window(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray_builder = tray_builder.icon(icon.clone());
    }

    tray_builder.build(app)?;
    app.manage(TrayMenuState {
        menu,
        update_lock: Mutex::new(()),
    });

    Ok(())
}

#[tauri::command]
pub fn update_tray_menu(
    app: AppHandle,
    state: State<'_, TrayMenuState>,
    snapshot: TrayMenuSnapshot,
) -> Result<(), String> {
    let _guard = state
        .update_lock
        .lock()
        .map_err(|_| "Tray menu update lock is unavailable.".to_string())?;

    rebuild_menu(&app, &state.menu, &snapshot).map_err(|error| error.to_string())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn rebuild_menu(
    app: &AppHandle,
    menu: &Menu<Wry>,
    snapshot: &TrayMenuSnapshot,
) -> tauri::Result<()> {
    while !menu.items()?.is_empty() {
        menu.remove_at(0)?;
    }

    let refresh = MenuItem::with_id(
        app,
        REFRESH_MENU_ID,
        if snapshot.refreshing {
            "Refreshing usage..."
        } else {
            "Refresh usage"
        },
        !snapshot.refreshing,
        None::<&str>,
    )?;
    menu.append(&refresh)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let normalized = normalize_rows(&snapshot.rows);
    if normalized.rows.is_empty() {
        let empty = MenuItem::with_id(
            app,
            "usage-empty",
            "No connected account usage yet",
            true,
            None::<&str>,
        )?;
        menu.append(&empty)?;
    } else {
        for (index, text) in normalized.rows.iter().enumerate() {
            let item = MenuItem::with_id(
                app,
                format!("{USAGE_MENU_ID_PREFIX}{index}"),
                text,
                true,
                None::<&str>,
            )?;
            menu.append(&item)?;
        }

        if normalized.hidden_count > 0 {
            let more = MenuItem::with_id(
                app,
                "usage-more",
                format!("{} more accounts · Show Quota", normalized.hidden_count),
                true,
                None::<&str>,
            )?;
            menu.append(&more)?;
        }
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        SHOW_MENU_ID,
        "Show Quota",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        QUIT_MENU_ID,
        "Quit Quota",
        true,
        None::<&str>,
    )?)?;

    Ok(())
}

fn normalize_rows(rows: &[String]) -> NormalizedRows {
    let valid_rows: Vec<String> = rows.iter().filter_map(|row| normalize_row(row)).collect();
    let hidden_count = valid_rows.len().saturating_sub(MAX_USAGE_ROWS);

    NormalizedRows {
        rows: valid_rows.into_iter().take(MAX_USAGE_ROWS).collect(),
        hidden_count,
    }
}

fn normalize_row(row: &str) -> Option<String> {
    let compact = row.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return None;
    }

    let escaped = compact.replace('&', "&&");
    let mut characters = escaped.chars();
    let shortened: String = characters.by_ref().take(MAX_ROW_CHARACTERS).collect();

    if characters.next().is_some() {
        Some(format!("{shortened}..."))
    } else {
        Some(shortened)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_whitespace_and_menu_mnemonics() {
        assert_eq!(
            normalize_row("  Claude & GPT\n5h 80%  "),
            Some("Claude && GPT 5h 80%".to_string())
        );
        assert_eq!(normalize_row(" \n\t "), None);
    }

    #[test]
    fn caps_long_rows_without_breaking_unicode() {
        let row = "é".repeat(MAX_ROW_CHARACTERS + 1);
        let normalized = normalize_row(&row).expect("row should remain visible");

        assert_eq!(normalized.chars().count(), MAX_ROW_CHARACTERS + 3);
        assert!(normalized.ends_with("..."));
    }

    #[test]
    fn limits_large_account_lists() {
        let rows: Vec<String> = (0..35).map(|index| format!("Account {index}")).collect();
        let normalized = normalize_rows(&rows);

        assert_eq!(normalized.rows.len(), MAX_USAGE_ROWS);
        assert_eq!(normalized.hidden_count, 5);
    }
}
