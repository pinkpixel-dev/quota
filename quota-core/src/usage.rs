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
    /// What to show when the account is readable but has no number to report,
    /// such as a plan with no allocation to measure. Without this, "signed in,
    /// nothing to measure" and "provider is broken" both render as a blank
    /// pane, and the user cannot tell which one they are looking at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl ProviderUsage {
    /// Render the compact sidebar token, falling back to the note, or `None`
    /// when there is nothing at all to say.
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
            self.note.clone()
        } else {
            Some(parts.join(" · "))
        }
    }
}
