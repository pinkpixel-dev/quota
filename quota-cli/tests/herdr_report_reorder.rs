#![cfg(unix)]
//! Verifies the fix that made `herdr report` list panes before touching the
//! cache or the network: when no pane maps to a known provider, the command
//! must exit 0 quietly, without fetching and without writing a cache file.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

/// Writes a stub `herdr` binary that answers `agent list` with an empty
/// list, and fails loudly for any other invocation (such as
/// `pane report-metadata`), which would mean the reorder regressed and the
/// reporting loop ran anyway.
fn write_fake_herdr(dir: &PathBuf) -> PathBuf {
    let script_path = dir.join("herdr");
    let script = r#"#!/bin/sh
if [ "$1" = "agent" ] && [ "$2" = "list" ]; then
  echo '{"id":"x","result":{"agents":[],"type":"agent_list"}}'
  exit 0
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
fn no_reportable_panes_skips_network_and_cache_and_exits_zero() {
    let work_dir =
        std::env::temp_dir().join(format!("quota-herdr-reorder-test-{}", std::process::id()));
    std::fs::create_dir_all(&work_dir).expect("create work dir");
    let state_dir = work_dir.join("state");
    let fake_herdr = write_fake_herdr(&work_dir);

    let exe = env!("CARGO_BIN_EXE_quota-cli");
    let output = Command::new(exe)
        .args(["herdr", "report"])
        .env("HERDR_BIN_PATH", &fake_herdr)
        .env("HERDR_PLUGIN_STATE_DIR", &state_dir)
        .output()
        .expect("run quota-cli herdr report");

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "expected no stdout, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "expected no stderr, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !state_dir.exists(),
        "cache directory should never be created when no pane needs a number"
    );

    let _ = std::fs::remove_dir_all(&work_dir);
}
