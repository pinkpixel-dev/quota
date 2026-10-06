use super::*;
use serde_json::json;

// 2026-10-02T12:00:00Z, earlier than every reset in the fixtures below.
const NOW: i64 = 1_790_942_400;

fn temp_storage_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("quota-opencode-go-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn stored(id: &str) -> StoredOpenCodeGoAccount {
    StoredOpenCodeGoAccount {
        id: id.to_string(),
        label: "Work".to_string(),
        api_key: "sk-secret-abcd".to_string(),
        usage: OpenCodeGoUsage::default(),
        quota_query_last_error: None,
        quota_query_last_error_at: None,
        usage_updated_at: None,
        created_at: 1,
        last_used: 1,
    }
}

#[test]
fn parses_all_three_windows() {
    let usage = parse_usage(&json!({
        "usage": {
            "rolling": { "status": "ok", "percent": 12, "resetsAt": "2026-10-02T16:18:57.800Z" },
            "weekly": { "status": "ok", "percent": 78, "resetsAt": "2026-10-05T00:00:00.000Z" },
            "monthly": { "status": "ok", "percent": 94, "resetsAt": "2026-10-11T12:22:02.000Z" }
        }
    }), NOW)
    .unwrap();

    assert_eq!(usage.five_hour.used_percent, Some(12.0));
    assert_eq!(usage.five_hour.remaining_percent, Some(88.0));
    assert_eq!(usage.five_hour.reset_at, Some(1_790_957_937));
    assert_eq!(usage.weekly.remaining_percent, Some(22.0));
    assert_eq!(usage.monthly.used_percent, Some(94.0));
}

#[test]
fn clamps_percents_and_tolerates_missing_reset() {
    let usage = parse_usage(
        &json!({
            "usage": {
                "rolling": { "percent": 0, "resetsAt": null },
                "weekly": { "percent": 104 },
                "monthly": { "percent": -3, "resetsAt": "not a date" }
            }
        }),
        NOW,
    )
    .unwrap();

    assert_eq!(usage.five_hour.used_percent, Some(0.0));
    assert_eq!(usage.five_hour.reset_at, None);
    assert_eq!(usage.weekly.used_percent, Some(100.0));
    assert_eq!(usage.monthly.used_percent, Some(0.0));
    assert_eq!(usage.monthly.reset_at, None);
}

#[test]
fn rejects_bodies_without_windows() {
    assert!(parse_usage(&json!({}), NOW).is_err());
    assert!(parse_usage(&json!({ "usage": {} }), NOW).is_err());
    assert!(parse_usage(&json!({ "usage": "nope" }), NOW).is_err());
}

#[test]
fn explains_rejected_keys_and_missing_subscriptions() {
    assert!(usage_error_message(401).contains("rejected this API key"));
    assert!(usage_error_message(403).contains("no OpenCode Go subscription"));
    assert_eq!(usage_error_message(500), "OpenCode Go usage returned 500.");
}

#[test]
fn masks_keys_and_derives_stable_ids() {
    assert_eq!(mask_api_key("  sk-test-abcd1234 "), "Go key ••••1234");
    assert_eq!(mask_api_key("ab"), "Go key ••••ab");
    assert_eq!(account_id_for_key("one"), account_id_for_key("one"));
    assert_ne!(account_id_for_key("one"), account_id_for_key("two"));
    assert_eq!(account_id_for_key("one").len(), 16);
}

#[test]
fn debug_output_redacts_the_key() {
    assert!(!format!("{:?}", stored("a")).contains("sk-secret"));
}

#[test]
fn stores_lists_and_deletes_accounts_privately() {
    let dir = temp_storage_dir("roundtrip");
    upsert_account_in(&dir, stored("first")).unwrap();
    upsert_account_in(&dir, stored("second")).unwrap();

    let ids: Vec<String> = list_accounts_in(&dir)
        .unwrap()
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(ids, vec!["second", "first"]);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(account_path_in(&dir, "first"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    delete_account_in(&dir, "first").unwrap();
    let ids: Vec<String> = list_accounts_in(&dir)
        .unwrap()
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(ids, vec!["second"]);
    assert!(!account_path_in(&dir, "first").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn treats_a_full_unused_rolling_window_as_not_started() {
    // Mirrors a real response: 0% used and a reset exactly five hours out.
    let usage = parse_usage(
        &json!({ "usage": { "rolling": { "status": "ok", "percent": 0, "resetsAt": "2026-10-02T17:00:00.000Z" } } }),
        NOW,
    )
    .unwrap();

    assert!(usage.five_hour.starts_on_first_use);
    assert_eq!(usage.five_hour.reset_at, None);
    assert_eq!(usage.five_hour.used_percent, Some(0.0));
}

#[test]
fn keeps_the_reset_for_a_started_rolling_window() {
    // Used, so the window is open even though the reset is far out.
    let used = parse_usage(
        &json!({ "usage": { "rolling": { "percent": 3, "resetsAt": "2026-10-02T17:00:00.000Z" } } }),
        NOW,
    )
    .unwrap();
    assert!(!used.five_hour.starts_on_first_use);
    assert!(used.five_hour.reset_at.is_some());

    // 0% but resetting in two hours, so it started three hours ago.
    let partial = parse_usage(
        &json!({ "usage": { "rolling": { "percent": 0, "resetsAt": "2026-10-02T14:00:00.000Z" } } }),
        NOW,
    )
    .unwrap();
    assert!(!partial.five_hour.starts_on_first_use);
    assert_eq!(partial.five_hour.reset_at, Some(NOW + 2 * 60 * 60));
}
