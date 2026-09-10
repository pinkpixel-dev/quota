//! Herdr integration. Reads the agent list, works out which panes host a
//! provider we can report on, and pushes a display token onto each one.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

/// How long a fetched token stays usable before the network is consulted again.
/// Matches the VS Code extension default in quota-vscode/src/configuration.ts.
pub const DEFAULT_TTL_MS: i64 = 120_000;

#[derive(Debug, Clone)]
pub struct AgentPane {
    pub agent: String,
    pub pane_id: String,
    pub workspace_id: String,
}

/// Parse the JSON envelope `herdr agent list` writes to stdout.
///
/// Read field by field rather than through a struct with required fields. A
/// pane Herdr describes in a shape we do not expect should cost that one pane
/// its number, not blank every pane in the sidebar.
pub fn parse_agent_list(raw: &str) -> Result<Vec<AgentPane>, String> {
    let envelope: Value = serde_json::from_str(raw)
        .map_err(|err| format!("could not parse herdr agent list: {}", err))?;

    let agents = envelope
        .get("result")
        .and_then(|result| result.get("agents"))
        .and_then(Value::as_array)
        .ok_or_else(|| "herdr agent list had no result.agents array".to_string())?;

    Ok(agents.iter().filter_map(agent_pane_from).collect())
}

fn agent_pane_from(entry: &Value) -> Option<AgentPane> {
    // Herdr reports the kind on the pane itself, and on the agent session for
    // panes it adopted rather than started.
    let agent = entry
        .get("agent")
        .and_then(Value::as_str)
        .or_else(|| entry.get("kind").and_then(Value::as_str))
        .or_else(|| {
            entry
                .get("agent_session")
                .and_then(|session| session.get("agent"))
                .and_then(Value::as_str)
        })?;

    Some(AgentPane {
        agent: agent.to_string(),
        pane_id: entry.get("pane_id").and_then(Value::as_str)?.to_string(),
        workspace_id: entry
            .get("workspace_id")
            .and_then(Value::as_str)?
            .to_string(),
    })
}

/// Map a Herdr agent kind onto a Quota provider.
///
/// Herdr names some agents more than one way, so the aliases matter: an
/// unmatched kind is silently skipped, which looks identical to a broken
/// provider. `gemini` is deliberately absent. It is the Gemini Code Assist CLI,
/// a different OAuth client from Antigravity's, and the Antigravity provider
/// only handles its own.
pub fn provider_for_agent_kind(kind: &str) -> Option<&'static str> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "claude" | "claude-code" | "anthropic" => Some("claude"),
        "codex" => Some("codex"),
        "cursor" => Some("cursor"),
        "agy" | "antigravity" | "antigravity-cli" => Some("antigravity"),
        "grok" => Some("grok"),
        "kiro" => Some("kiro"),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCache {
    pub fetched_at_ms: i64,
    pub token: String,
}

impl TokenCache {
    /// Stale when older than the TTL, and also when the stamp is in the future,
    /// which happens after a clock change and should force a refetch.
    pub fn is_stale_at_ms(&self, now_ms: i64, ttl_ms: i64) -> bool {
        let age = now_ms - self.fetched_at_ms;
        age < 0 || age > ttl_ms
    }
}

/// One cache file per provider. A shared file would let panes running
/// different agents overwrite each other's numbers on every status change.
pub fn cache_path(state_dir: &Path, provider: &str) -> PathBuf {
    state_dir.join(format!("{}-token.json", provider))
}

pub fn read_cache(state_dir: &Path, provider: &str) -> Option<TokenCache> {
    let raw = std::fs::read_to_string(cache_path(state_dir, provider)).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn write_cache(state_dir: &Path, provider: &str, cache: &TokenCache) -> Result<(), String> {
    std::fs::create_dir_all(state_dir)
        .map_err(|err| format!("could not create state directory: {}", err))?;
    let rendered = serde_json::to_string(cache)
        .map_err(|err| format!("could not render cache: {}", err))?;
    std::fs::write(cache_path(state_dir, provider), rendered)
        .map_err(|err| format!("could not write cache: {}", err))
}

/// The Herdr binary to invoke. Herdr injects HERDR_BIN_PATH into plugin
/// commands; the fallback only matters when running the CLI by hand.
pub fn herdr_binary() -> String {
    std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_string())
}

pub fn list_agent_panes() -> Result<Vec<AgentPane>, String> {
    let output = ProcessCommand::new(herdr_binary())
        .args(["agent", "list"])
        .output()
        .map_err(|err| format!("could not run herdr agent list: {}", err))?;

    if !output.status.success() {
        return Err(format!(
            "herdr agent list failed with status {}",
            output.status
        ));
    }

    parse_agent_list(&String::from_utf8_lossy(&output.stdout))
}

/// Push one display token onto a pane. The TTL is set slightly above the
/// refresh interval so a value never blinks out between reports.
pub fn report_pane_token(pane_id: &str, source: &str, token: &str) -> Result<(), String> {
    let status = ProcessCommand::new(herdr_binary())
        .args([
            "pane",
            "report-metadata",
            pane_id,
            "--source",
            source,
            "--token",
            &format!("quota={}", token),
            "--ttl-ms",
            "600000",
        ])
        .status()
        .map_err(|err| format!("could not run herdr pane report-metadata: {}", err))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("herdr pane report-metadata failed: {}", status))
    }
}

pub fn report_workspace_token(
    workspace_id: &str,
    source: &str,
    token: &str,
) -> Result<(), String> {
    let status = ProcessCommand::new(herdr_binary())
        .args([
            "workspace",
            "report-metadata",
            workspace_id,
            "--source",
            source,
            "--token",
            &format!("quota={}", token),
            "--ttl-ms",
            "600000",
        ])
        .status()
        .map_err(|err| format!("could not run herdr workspace report-metadata: {}", err))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("herdr workspace report-metadata failed: {}", status))
    }
}
