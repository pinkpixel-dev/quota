//! Opening URLs in the user's real browser.
//!
//! `tauri-plugin-opener` spawns `xdg-open` as a plain child process, which
//! inherits whatever environment the app is running under. Inside an AppImage
//! that environment points `LD_LIBRARY_PATH` (and friends) at the bundled
//! AppDir libraries, so the browser starts against the wrong GLib/GTK/OpenSSL
//! and segfaults. `xdg-open` still exits 0 in that case, so nothing is reported
//! back and the click looks like it did nothing at all.
//!
//! This module opens URLs itself so the launcher is spawned with the AppImage
//! variables stripped from the child environment.

use std::process::{Command, Stdio};

#[cfg(target_os = "linux")]
use std::ffi::OsStr;

/// Variables the AppImage runtime and the linuxdeploy GTK hook inject, which
/// must not reach a browser running against the host system.
#[cfg(target_os = "linux")]
const APPIMAGE_ONLY_VARS: &[&str] = &[
    "LD_LIBRARY_PATH",
    "LD_PRELOAD",
    "GIO_MODULE_DIR",
    "GIO_EXTRA_MODULES",
    "GSETTINGS_SCHEMA_DIR",
    "GTK_DATA_PREFIX",
    "GTK_EXE_PREFIX",
    "GTK_IM_MODULE_FILE",
    "GTK_PATH",
    "GTK_THEME",
    "GDK_BACKEND",
    "GDK_PIXBUF_MODULE_FILE",
    "GST_PLUGIN_SYSTEM_PATH",
    "PYTHONPATH",
    "QT_PLUGIN_PATH",
];

/// Path-list variables where the AppDir entries must be dropped but the host
/// entries have to survive.
#[cfg(target_os = "linux")]
const APPIMAGE_MIXED_PATH_VARS: &[&str] = &["PATH", "XDG_DATA_DIRS"];

/// Sign-in hosts the app is allowed to open. These mirror the allow list that
/// used to live in `capabilities/default.json`, and they matter because some of
/// these URLs arrive in a provider's device-flow response rather than being
/// built locally.
const ALLOWED_HOSTS: &[&str] = &[
    "accounts.google.com",
    "accounts.x.ai",
    "app.kiro.dev",
    "auth.openai.com",
    "auth.x.ai",
    "claude.com",
    "cursor.com",
    "github.com",
];

/// Returns the host of an `https://` URL, without the port or any credentials.
fn https_host(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|value| !value.is_empty())?;

    if authority.contains('@') {
        return None;
    }

    Some(authority.split(':').next().unwrap_or(authority))
}

/// Only allow-listed `https` sign-in URLs are ever handed to a launcher, so a
/// crafted value cannot turn into a local file or a command line option.
fn validate_url(url: &str) -> Result<(), String> {
    if url.contains(['\n', '\r', '\0']) {
        return Err("Refusing to open a URL containing control characters.".to_string());
    }

    let Some(host) = https_host(url) else {
        return Err(format!("Refusing to open a URL that is not https: {url}"));
    };

    if !ALLOWED_HOSTS.contains(&host) {
        return Err(format!("Refusing to open an unexpected sign-in host: {host}"));
    }

    Ok(())
}

/// Removes every AppDir entry from a `:`-separated path list.
#[cfg(target_os = "linux")]
fn strip_appdir_entries(value: &OsStr, appdir: &str) -> Option<String> {
    let value = value.to_string_lossy();
    let kept: Vec<&str> = value
        .split(':')
        .filter(|entry| !entry.is_empty() && !entry.starts_with(appdir))
        .collect();

    if kept.is_empty() {
        None
    } else {
        Some(kept.join(":"))
    }
}

/// Restores a host-like environment for a child process launched from inside an
/// AppImage. Outside an AppImage this is a no-op.
#[cfg(target_os = "linux")]
pub fn sanitize_appimage_env(command: &mut Command) {
    let Some(appdir) = std::env::var_os("APPDIR") else {
        return;
    };
    let appdir = appdir.to_string_lossy().trim_end_matches('/').to_string();
    if appdir.is_empty() {
        return;
    }

    for name in APPIMAGE_ONLY_VARS {
        command.env_remove(name);
    }

    for name in APPIMAGE_MIXED_PATH_VARS {
        let Some(value) = std::env::var_os(name) else {
            continue;
        };
        match strip_appdir_entries(&value, &appdir) {
            Some(cleaned) => command.env(name, cleaned),
            None => command.env_remove(name),
        };
    }
}

#[cfg(not(target_os = "linux"))]
pub fn sanitize_appimage_env(_command: &mut Command) {}

/// Launcher candidates, tried in order until one reports that it handled the URL.
#[cfg(target_os = "linux")]
fn launcher_commands(url: &str) -> Vec<Command> {
    let mut commands = Vec::new();

    let mut xdg = Command::new("xdg-open");
    xdg.arg(url);
    commands.push(xdg);

    let mut gio = Command::new("gio");
    gio.arg("open").arg(url);
    commands.push(gio);

    if let Some(browser) = std::env::var_os("BROWSER") {
        if !browser.is_empty() {
            let mut custom = Command::new(browser);
            custom.arg(url);
            commands.push(custom);
        }
    }

    commands
}

#[cfg(target_os = "macos")]
fn launcher_commands(url: &str) -> Vec<Command> {
    let mut open = Command::new("open");
    open.arg(url);
    vec![open]
}

#[cfg(target_os = "windows")]
fn launcher_commands(url: &str) -> Vec<Command> {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", url]);
    vec![cmd]
}

/// Opens `url` in the user's default browser.
///
/// Returns an error when every launcher failed, so the UI can tell the user to
/// copy the link instead of leaving the click silently unanswered.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    validate_url(&url)?;

    // A cold browser can keep the launcher busy for a few seconds, so this
    // waits off the main thread.
    tauri::async_runtime::spawn_blocking(move || spawn_launchers(&url))
        .await
        .map_err(|error| format!("Browser launch task failed: {error}"))?
}

/// Runs the launcher candidates in order. Public so the integration tests can
/// drive the real launch path against a stub launcher.
pub fn spawn_launchers(url: &str) -> Result<(), String> {
    let mut failures = Vec::new();

    for mut command in launcher_commands(url) {
        let program = command.get_program().to_string_lossy().into_owned();
        sanitize_appimage_env(&mut command);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => failures.push(format!("{program} exited with {status}")),
            Err(error) => failures.push(format!("{program} could not run: {error}")),
        }
    }

    Err(format!(
        "Could not open the link in a browser. {}",
        failures.join("; ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_real_sign_in_urls() {
        for url in [
            "https://github.com/login/device",
            "https://auth.openai.com/oauth/authorize?client_id=abc",
            "https://accounts.google.com/o/oauth2/v2/auth?scope=x",
            "https://claude.com/cai/oauth/authorize?state=1",
            "https://app.kiro.dev/signin?next=%2F",
            "https://cursor.com/loginDeepControl?uuid=1",
            "https://accounts.x.ai/oauth2/device?user_code=ABCD-EFGH",
            "https://auth.x.ai/oauth2/device?user_code=ABCD-EFGH",
        ] {
            assert!(validate_url(url).is_ok(), "should allow {url}");
        }
    }

    #[test]
    fn rejects_non_https_and_unknown_hosts() {
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("/tmp/evil.desktop").is_err());
        assert!(validate_url("--load-modules=/tmp/evil.so").is_err());
        assert!(validate_url("http://github.com/login/device").is_err());
        assert!(validate_url("https://evil.example.com/login").is_err());
    }

    #[test]
    fn rejects_hosts_disguised_with_credentials() {
        assert!(validate_url("https://github.com@evil.example.com/login").is_err());
    }

    #[test]
    fn reads_the_host_without_port_or_path() {
        assert_eq!(https_host("https://claude.com:443/a/b?c=1"), Some("claude.com"));
        assert_eq!(https_host("https://claude.com"), Some("claude.com"));
        assert_eq!(https_host("https://"), None);
    }

    #[test]
    fn rejects_control_characters() {
        assert!(validate_url("https://example.com\nrm -rf /").is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn drops_appdir_entries_but_keeps_host_entries() {
        let value = OsStr::new("/tmp/.mount_quota/usr/bin:/usr/local/bin:/usr/bin");
        assert_eq!(
            strip_appdir_entries(value, "/tmp/.mount_quota"),
            Some("/usr/local/bin:/usr/bin".to_string())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn drops_the_variable_when_only_appdir_entries_remain() {
        let value = OsStr::new("/tmp/.mount_quota/usr/lib:/tmp/.mount_quota/usr/lib64");
        assert_eq!(strip_appdir_entries(value, "/tmp/.mount_quota"), None);
    }
}
