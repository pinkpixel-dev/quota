#![cfg(unix)]
//! Verifies that `herdr report` exits 1 when every pane/workspace report
//! call fails. Before this fix, the loop printed each failure to stderr and
//! then fell off the end of `run_herdr_report`, leaving the process to exit
//! 0 even though nothing was actually reported to Herdr.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

/// Writes a stub `herdr` binary that answers `agent list` with one Claude
/// pane, and fails every `pane report-metadata` / `workspace
/// report-metadata` call, simulating a Herdr server that rejects all
/// reports.
fn write_fake_herdr(dir: &PathBuf) -> PathBuf {
    let script_path = dir.join("herdr");
    let script = r#"#!/bin/sh
if [ "$1" = "agent" ] && [ "$2" = "list" ]; then
  echo '{"id":"x","result":{"agents":[{"agent":"claude","pane_id":"pane-1","workspace_id":"workspace-1"}],"type":"agent_list"}}'
  exit 0
fi
if [ "$1" = "pane" ] && [ "$2" = "report-metadata" ]; then
  echo "pane report-metadata rejected" >&2
  exit 1
fi
if [ "$1" = "workspace" ] && [ "$2" = "report-metadata" ]; then
  echo "workspace report-metadata rejected" >&2
  exit 1
fi
echo "unexpected herdr invocation: $@" >&2
exit 1
"#;
    std::fs::write(&script_path, script).expect("write fake herdr script");
    let mut perms = std::fs::metadata(&script_path)
        .expect("stat fake herdr script")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script_path, perms).expect("chmod fake herdr script");
    script_path
}

#[test]
fn every_report_failing_exits_nonzero() {
    let work_dir = std::env::temp_dir().join(format!(
        "quota-herdr-all-failures-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&work_dir).expect("create work dir");
    let state_dir = work_dir.join("state");
    let fake_herdr = write_fake_herdr(&work_dir);

    // Pre-seed a fresh cache so `herdr report` skips fetching from the
    // network entirely and goes straight to the reporting loop.
    std::fs::create_dir_all(&state_dir).expect("create state dir");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_millis() as i64;
    let cache_json = format!(
        r#"{{"fetchedAtMs":{},"token":"5h 89% · Wk 57%"}}"#,
        now_ms
    );
    std::fs::write(state_dir.join("claude-token.json"), cache_json).expect("seed cache");

    let exe = env!("CARGO_BIN_EXE_quota-cli");
    let output = Command::new(exe)
        .args(["herdr", "report"])
        .env("HERDR_BIN_PATH", &fake_herdr)
        .env("HERDR_PLUGIN_STATE_DIR", &state_dir)
        .output()
        .expect("run quota-cli herdr report");

    assert!(
        !output.status.success(),
        "expected a nonzero exit when every report call fails, got {:?}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1, got {:?}",
        output.status.code()
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("pane report-metadata failed"),
        "expected stderr to mention the pane failure, got: {}",
        stderr
    );
    assert!(
        stderr.contains("workspace report-metadata failed"),
        "expected stderr to mention the workspace failure, got: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&work_dir);
}
