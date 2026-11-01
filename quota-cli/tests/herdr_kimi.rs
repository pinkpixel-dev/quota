#![cfg(unix)]
//! Exercise the real CLI against a fake Herdr, never the user's live session.

use quota_cli::herdr::{read_cache, write_cache, TokenCache, DEFAULT_TTL_MS};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Fixture {
    dir: PathBuf,
    now_ms: i64,
}

impl Fixture {
    fn new(name: &str, agents: Value) -> Self {
        let dir =
            std::env::temp_dir().join(format!("quota-herdr-kimi-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("agents.json"),
            json!({"result": {"agents": agents}}).to_string(),
        )
        .unwrap();
        let script = r#"#!/bin/sh
if [ "$1" = "agent" ] && [ "$2" = "list" ]; then
  cat "$QUOTA_TEST_DIR/agents.json"
  exit 0
fi
if [ "$2" = "report-metadata" ]; then
  printf '%s\n' "$*" >> "$QUOTA_TEST_DIR/reports"
  exit 0
fi
exit 1
"#;
        let binary = dir.join("herdr");
        std::fs::write(&binary, script).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            dir,
            now_ms: chrono_ms(),
        }
    }

    fn seed(&self, provider: &str, token: &str, age: i64) {
        write_cache(
            &self.dir.join("state"),
            provider,
            &TokenCache {
                fetched_at_ms: self.now_ms - age,
                token: token.to_string(),
            },
        )
        .unwrap();
    }

    fn command(&self, force: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_quota-cli"));
        command.args(["herdr", "report"]);
        if force {
            command.arg("--force");
        }
        command
            .env("HERDR_BIN_PATH", self.dir.join("herdr"))
            .env("HERDR_PLUGIN_STATE_DIR", self.dir.join("state"))
            .env("HERDR_PLUGIN_ID", "pinkpixel.quota")
            .env("KIMI_CODE_HOME", self.dir.join("kimi-home"))
            .env("QUOTA_TEST_DIR", &self.dir)
            // Isolate the override: a value inherited from the developer's
            // shell must never reach a test process.
            .env_remove("KIMI_CODE_BASE_URL");
        command
    }

    fn run(&self, force: bool) -> Output {
        self.command(force).output().unwrap()
    }

    fn reports(&self) -> String {
        std::fs::read_to_string(self.dir.join("reports")).unwrap_or_default()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn chrono_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[test]
fn kimi_panes_share_a_fresh_cache_and_report_the_pure_workspace() {
    let fixture = Fixture::new(
        "pure",
        json!([
            {"agent": "kimi", "pane_id": "w1:p1", "workspace_id": "w1"},
            {"agent": "KIMI-CODE", "pane_id": "w1:p2", "workspace_id": "w1"}
        ]),
    );
    fixture.seed("kimi", "5h 82% · Wk 64%", 0);
    let output = fixture.run(false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // No credentials exist: an accidental fetch would emit a credential error.
    assert!(output.stderr.is_empty());
    let reports = fixture.reports();
    assert_eq!(reports.lines().count(), 3);
    for target in [
        "pane report-metadata w1:p1",
        "pane report-metadata w1:p2",
        "workspace report-metadata w1",
    ] {
        assert!(reports.contains(&format!(
            "{} --source plugin:pinkpixel.quota --token quota=5h 82% · Wk 64%",
            target
        )));
    }
    assert!(!fixture.dir.join("state/codex-token.json").exists());
}

#[test]
fn mixed_workspaces_report_each_pane_with_its_own_provider_cache() {
    let fixture = Fixture::new(
        "mixed",
        json!([
            {"agent": "kimi", "pane_id": "w1:p1", "workspace_id": "w1"},
            {"agent": "codex", "pane_id": "w1:p2", "workspace_id": "w1"}
        ]),
    );
    fixture.seed("kimi", "5h 82% · Wk 64%", 0);
    fixture.seed("codex", "5h 99% · Wk 90%", 0);
    let output = fixture.run(false);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let reports = fixture.reports();
    assert_eq!(reports.lines().count(), 2);
    assert!(reports.contains(
        "pane report-metadata w1:p1 --source plugin:pinkpixel.quota --token quota=5h 82% · Wk 64%"
    ));
    assert!(reports.contains(
        "pane report-metadata w1:p2 --source plugin:pinkpixel.quota --token quota=5h 99% · Wk 90%"
    ));
    assert!(!reports.contains("workspace report-metadata"));
}

#[test]
fn stale_or_forced_cache_fetches_kimi_once_then_falls_back_on_credential_failure() {
    for (name, age, force) in [("stale", DEFAULT_TTL_MS + 1000, false), ("forced", 0, true)] {
        let fixture = Fixture::new(
            name,
            json!([
                {"agent": "kimi", "pane_id": "w1:p1", "workspace_id": "w1"},
                {"agent": "kimi-code", "pane_id": "w1:p2", "workspace_id": "w1"}
            ]),
        );
        fixture.seed("kimi", "5h 82% · Wk 64%", age);
        let output = fixture.run(force);
        assert!(output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            stderr
                .matches("No Kimi Code OAuth credentials found")
                .count(),
            1,
            "{}",
            stderr
        );
        assert_eq!(
            stderr.lines().count(),
            1,
            "only Kimi should be fetched: {}",
            stderr
        );
        assert_eq!(fixture.reports().lines().count(), 3);
        assert_eq!(
            read_cache(&fixture.dir.join("state"), "kimi")
                .unwrap()
                .fetched_at_ms,
            fixture.now_ms - age
        );
    }
}

#[test]
fn expired_credentials_without_a_cache_report_an_error_without_modification() {
    let fixture = Fixture::new(
        "expired",
        json!([
            {"agent": "kimi", "pane_id": "w1:p1", "workspace_id": "w1"}
        ]),
    );
    let home = fixture.dir.join("kimi-home/credentials");
    std::fs::create_dir_all(&home).unwrap();
    let path = home.join("kimi-code.json");
    let body = r#"{"access_token":"test-secret","refresh_token":"refresh-secret","expires_at":1}"#;
    std::fs::write(&path, body).unwrap();
    let output = fixture.run(false);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("expired"));
    assert!(stderr.contains("/login"));
    assert!(!stderr.contains("secret"));
    assert!(fixture.reports().is_empty());
    assert!(!fixture.dir.join("state").exists());
    assert_eq!(std::fs::read_to_string(path).unwrap(), body);
}

#[test]
fn unsupported_panes_do_not_read_kimi_credentials_or_touch_the_cache() {
    let fixture = Fixture::new(
        "unsupported",
        json!([
            {"agent": "opencode", "pane_id": "w1:p1", "workspace_id": "w1"}
        ]),
    );
    let output = fixture.run(true);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(fixture.reports().is_empty());
    assert!(!fixture.dir.join("state").exists());
}

#[test]
fn invalid_base_url_env_fails_safely_without_sending_a_request() {
    let fixture = Fixture::new(
        "bad-base-url",
        json!([
            {"agent": "kimi", "pane_id": "w1:p1", "workspace_id": "w1"}
        ]),
    );
    let creds = fixture.dir.join("kimi-home/credentials");
    std::fs::create_dir_all(&creds).unwrap();
    std::fs::write(
        creds.join("kimi-code.json"),
        r#"{"access_token":"test-secret","refresh_token":"refresh-secret","expires_at":4102444800}"#,
    )
    .unwrap();
    // Credentials are valid and future-dated, so any failure here comes from
    // URL resolution. Neither value is an official endpoint, so the CLI must
    // stop before opening a connection.
    for (name, value) in [
        ("plain-http", "http://api.kimi.com/coding/v1"),
        ("gateway", "https://token@quota-gateway.example/coding/v1"),
    ] {
        let output = fixture
            .command(true)
            .env("KIMI_CODE_BASE_URL", value)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "case: {name}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("KIMI_CODE_BASE_URL"),
            "case: {name}: {stderr}"
        );
        assert!(
            !stderr.contains(value),
            "case: {name}: stderr echoes the configured value"
        );
        assert!(
            !stderr.contains("secret") && !stderr.contains("token"),
            "case: {name}: {stderr}"
        );
        assert!(fixture.reports().is_empty(), "case: {name}");
        assert!(!fixture.dir.join("state").exists(), "case: {name}");
    }
}
