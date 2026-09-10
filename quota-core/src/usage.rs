use serde::{Deserialize, Serialize};

/// One usage window as the sidebar thinks about it: a short label, how much is
/// left, and when it rolls over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub label: String,
    pub remaining_percent: Option<i32>,
    pub reset_at: Option<i64>,
}

/// Normalized usage for one provider account. Windows are ordered shortest
/// first, so the compact token reads "5h ... then Wk ...".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsage {
    pub provider: String,
    pub account_label: Option<String>,
    pub windows: Vec<UsageWindow>,
}

impl ProviderUsage {
    /// Render the compact sidebar token, or `None` when no window has a number.
    pub fn compact_token(&self) -> Option<String> {
        let parts: Vec<String> = self
            .windows
            .iter()
            .filter_map(|window| {
                window
                    .remaining_percent
                    .map(|percent| format!("{} {}%", window.label, percent))
            })
            .take(2)
            .collect();

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" · "))
        }
    }
}
