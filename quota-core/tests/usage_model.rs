use quota_core::usage::{ProviderUsage, UsageWindow};

fn window(label: &str, remaining: Option<i32>) -> UsageWindow {
    UsageWindow {
        label: label.to_string(),
        short_label: None,
        remaining_percent: remaining,
        reset_at: None,
    }
}

fn window_with_short(label: &str, short_label: &str, remaining: Option<i32>) -> UsageWindow {
    UsageWindow {
        label: label.to_string(),
        short_label: Some(short_label.to_string()),
        remaining_percent: remaining,
        reset_at: None,
    }
}

#[test]
fn compact_token_joins_the_first_two_windows() {
    let usage = ProviderUsage {
        provider: "claude".to_string(),
        account_label: None,
        note: None,
        windows: vec![window("5h", Some(89)), window("Wk", Some(57))],
    };
    assert_eq!(usage.compact_token().as_deref(), Some("5h 89% · Wk 57%"));
}

#[test]
fn compact_token_skips_windows_with_no_number() {
    let usage = ProviderUsage {
        provider: "claude".to_string(),
        account_label: None,
        note: None,
        windows: vec![window("5h", Some(89)), window("Wk", None)],
    };
    assert_eq!(usage.compact_token().as_deref(), Some("5h 89%"));
}

#[test]
fn compact_token_is_none_when_nothing_is_known() {
    let usage = ProviderUsage {
        provider: "claude".to_string(),
        account_label: None,
        note: None,
        windows: vec![window("5h", None)],
    };
    assert_eq!(usage.compact_token(), None);
}

#[test]
fn compact_token_uses_at_most_two_windows() {
    let usage = ProviderUsage {
        provider: "claude".to_string(),
        account_label: None,
        note: None,
        windows: vec![
            window("5h", Some(89)),
            window("Wk", Some(57)),
            window("Opus", Some(12)),
        ],
    };
    assert_eq!(usage.compact_token().as_deref(), Some("5h 89% · Wk 57%"));
}

#[test]
fn compact_token_falls_back_to_the_note_when_no_window_has_a_number() {
    // A readable account with nothing to measure must not render the same as a
    // provider that failed. Both used to produce a blank pane.
    let usage = ProviderUsage {
        provider: "grok".to_string(),
        account_label: None,
        windows: vec![window("Credit", None)],
        note: Some("no credit allocation".to_string()),
    };
    assert_eq!(
        usage.compact_token().as_deref(),
        Some("no credit allocation")
    );
}

#[test]
fn a_real_number_always_wins_over_the_note() {
    let usage = ProviderUsage {
        provider: "grok".to_string(),
        account_label: None,
        windows: vec![window("Credit", Some(64))],
        note: Some("no credit allocation".to_string()),
    };
    assert_eq!(usage.compact_token().as_deref(), Some("Credit 64%"));
}

#[test]
fn compact_token_uses_the_short_label_when_one_is_set() {
    // Antigravity's shared pools carry full labels for the pane but must stay
    // short in the Herdr token when the pair falls back to Claude/GPT.
    let usage = ProviderUsage {
        provider: "antigravity".to_string(),
        account_label: None,
        note: None,
        windows: vec![
            window_with_short("Claude/GPT 5h", "Claude 5h", Some(100)),
            window_with_short("Claude/GPT Wk", "Claude Wk", Some(18)),
        ],
    };
    assert_eq!(
        usage.compact_token().as_deref(),
        Some("Claude 5h 100% · Claude Wk 18%")
    );
}

#[test]
fn a_window_without_a_short_label_keeps_its_full_label_in_the_token() {
    let usage = ProviderUsage {
        provider: "claude".to_string(),
        account_label: None,
        note: None,
        windows: vec![
            window("5h", Some(89)),
            window_with_short("Claude/GPT Wk", "Claude Wk", Some(57)),
        ],
    };
    assert_eq!(
        usage.compact_token().as_deref(),
        Some("5h 89% · Claude Wk 57%")
    );
}

#[test]
fn the_short_label_is_omitted_from_json_when_unset() {
    // Other providers keep the exact JSON they had before the field existed.
    let usage = ProviderUsage {
        provider: "claude".to_string(),
        account_label: None,
        note: None,
        windows: vec![window("5h", Some(89))],
    };
    let json = serde_json::to_value(&usage).expect("serialize usage");
    assert!(json["windows"][0].get("shortLabel").is_none());
}

#[test]
fn the_short_label_is_serialized_when_set() {
    let usage = ProviderUsage {
        provider: "antigravity".to_string(),
        account_label: None,
        note: None,
        windows: vec![window_with_short("Claude/GPT 5h", "Claude 5h", Some(100))],
    };
    let json = serde_json::to_value(&usage).expect("serialize usage");
    assert_eq!(json["windows"][0]["shortLabel"], "Claude 5h");
}
