use quota_core::opencode_go::local_usage::normalize_usage_response;
use serde_json::json;

/// Mid-afternoon on a sample day, so the placeholder-reset rule has a fixed
/// clock to reason about.
const NOW: i64 = 1_795_200_000;

#[test]
fn inverts_used_percent_into_remaining_percent() {
    // The endpoint reports how much of each window has been used, the same
    // percentages the OpenCode console shows. Everything downstream reads
    // remaining. If this test fails, the CLI is telling users the opposite of
    // the truth.
    let raw = json!({
        "usage": {
            "rolling": {"status": "ok", "percent": 35.0,
                        "resetsAt": "2026-11-20T19:15:17.000Z"},
            "weekly": {"status": "ok", "percent": 50.0,
                       "resetsAt": "2026-11-23T00:00:00.000Z"},
            "monthly": {"status": "ok", "percent": 25.0,
                        "resetsAt": "2026-12-19T14:10:39.000Z"}
        }
    });

    let usage = normalize_usage_response(&raw, NOW);

    assert_eq!(usage.provider, "opencode-go");
    assert_eq!(usage.windows.len(), 3);
    assert_eq!(usage.windows[0].label, "5h");
    assert_eq!(usage.windows[0].remaining_percent, Some(65));
    assert_eq!(usage.windows[0].reset_at, Some(1_795_202_117));
    assert_eq!(usage.windows[1].label, "Wk");
    assert_eq!(usage.windows[1].remaining_percent, Some(50));
    assert_eq!(usage.windows[2].label, "Mo");
    assert_eq!(usage.windows[2].remaining_percent, Some(75));
}

#[test]
fn an_unused_rolling_window_has_no_reset_time() {
    // An unstarted rolling window comes back as 0% with a placeholder reset a
    // full five hours out. There is nothing to roll over yet, so keeping the
    // placeholder would tell the user a window is closing when it is not.
    let resets_at = chrono::DateTime::from_timestamp(NOW + 5 * 60 * 60, 0)
        .expect("timestamp")
        .to_rfc3339();
    let raw = json!({
        "usage": {
            "rolling": {"status": "ok", "percent": 0, "resetsAt": resets_at},
            "weekly": {"status": "ok", "percent": 0,
                       "resetsAt": "2026-11-23T00:00:00.000Z"}
        }
    });

    let usage = normalize_usage_response(&raw, NOW);

    assert_eq!(usage.windows[0].remaining_percent, Some(100));
    assert_eq!(usage.windows[0].reset_at, None);
}

#[test]
fn a_started_rolling_window_keeps_its_reset_time() {
    // Zero percent used with a reset sooner than five hours out means the
    // window has started and just rolled over. That reset time is real.
    let resets_at = chrono::DateTime::from_timestamp(NOW + 60 * 60, 0)
        .expect("timestamp")
        .to_rfc3339();
    let raw = json!({
        "usage": {"rolling": {"status": "ok", "percent": 0, "resetsAt": resets_at}}
    });

    let usage = normalize_usage_response(&raw, NOW);

    assert_eq!(usage.windows[0].reset_at, Some(NOW + 60 * 60));
}

#[test]
fn a_body_with_no_windows_is_said_out_loud() {
    // The endpoint is undocumented, so a shape change must surface as a note
    // rather than render a blank that looks like a broken provider.
    let raw = json!({"usage": {}});

    let usage = normalize_usage_response(&raw, NOW);

    assert!(usage.windows.is_empty());
    assert_eq!(usage.note.as_deref(), Some("no usage windows reported"));
}

#[test]
fn the_compact_token_reads_5h_then_wk() {
    let raw = json!({
        "usage": {
            "rolling": {"status": "ok", "percent": 35.0,
                        "resetsAt": "2026-11-20T19:15:17.000Z"},
            "weekly": {"status": "ok", "percent": 50.0,
                       "resetsAt": "2026-11-23T00:00:00.000Z"},
            "monthly": {"status": "ok", "percent": 25.0,
                        "resetsAt": "2026-12-19T14:10:39.000Z"}
        }
    });

    let usage = normalize_usage_response(&raw, NOW);

    assert_eq!(usage.compact_token().as_deref(), Some("5h 65% · Wk 50%"));
}
