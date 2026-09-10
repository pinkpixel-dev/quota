//! Locating the directories provider CLIs keep their credentials in.

use std::path::PathBuf;

/// Resolve a provider's home directory the way its own CLI does: an explicit
/// environment override wins, otherwise a dot-directory under the user's home.
///
/// The override is trimmed of whitespace and of the quotes a shell profile
/// often leaves behind, matching `codex_home` and `grok_home` in the desktop
/// app. A user who relocates their CLI home and then sees no usage would have
/// no way to tell why, so this is worth honoring rather than assuming.
pub fn provider_home(env_var: &str, default_dir_name: &str) -> Option<PathBuf> {
    if let Some(from_env) = std::env::var(env_var)
        .ok()
        .map(|raw| {
            raw.trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string()
        })
        .filter(|raw| !raw.is_empty())
    {
        return Some(PathBuf::from(from_env));
    }

    dirs::home_dir().map(|home| home.join(default_dir_name))
}

/// Resolve the XDG config directory, for CLIs that store credentials under
/// `~/.config/<name>` instead of a dot-directory of their own.
pub fn config_home() -> Option<PathBuf> {
    provider_home("XDG_CONFIG_HOME", ".config")
}

/// Resolve the XDG data directory, for CLIs that keep durable state under
/// `~/.local/share/<name>`.
pub fn data_home() -> Option<PathBuf> {
    provider_home("XDG_DATA_HOME", ".local/share")
}
