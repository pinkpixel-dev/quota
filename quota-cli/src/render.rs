//! Human-readable rendering for `quota-cli usage`.
//!
//! The Herdr sidebar has one narrow line per pane, so `compact_token` stays
//! percent-only. A terminal has room, and the reset time is the other half of
//! what the desktop app shows, so the text report spells it out.

use quota_core::usage::ProviderUsage;

/// Render one provider's detail line: every window that reports a percent,
/// plus how long until it rolls over when the provider tells us.
///
/// `None` when there is nothing at all to say, matching `compact_token` so the
/// caller keeps one fallback.
pub fn usage_detail(usage: &ProviderUsage, now: i64) -> Option<String> {
    let parts: Vec<String> = usage
        .windows
        .iter()
        .filter_map(|window| {
            let percent = window.remaining_percent?;
            Some(match window.reset_at {
                Some(reset_at) => format!(
                    "{} {}% ({})",
                    window.label,
                    percent,
                    relative_reset(reset_at, now)
                ),
                None => format!("{} {}%", window.label, percent),
            })
        })
        .collect();

    if parts.is_empty() {
        usage.note.clone()
    } else {
        Some(parts.join(" · "))
    }
}

/// How long until a window resets, in the coarsest two units that still say
/// something useful. A stamp already in the past reads as due rather than as a
/// negative duration, because a provider can report a boundary we have crossed
/// before its own numbers catch up.
fn relative_reset(reset_at: i64, now: i64) -> String {
    let seconds = reset_at - now;
    if seconds <= 0 {
        return "resets now".to_string();
    }

    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days > 0 {
        let hours = hours % 24;
        if hours > 0 {
            return format!("resets in {}d {}h", days, hours);
        }
        return format!("resets in {}d", days);
    }

    if hours > 0 {
        let minutes = minutes % 60;
        if minutes > 0 {
            return format!("resets in {}h {}m", hours, minutes);
        }
        return format!("resets in {}h", hours);
    }

    format!("resets in {}m", minutes.max(1))
}
