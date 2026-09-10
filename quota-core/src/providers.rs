//! The provider registry: which providers have a local-credential reader, and
//! how to fetch usage for one by name.
//!
//! Callers that report on several providers at once need a single shape, so the
//! per-provider error types are flattened to their rendered message here. Every
//! one of those messages is already written to be read by a user.

use crate::usage::ProviderUsage;

/// Every provider the CLI can read, in the order it lists them. Shortest
/// windows first is a per-provider concern; this order is just the reading
/// order of the report.
pub const PROVIDERS: &[&str] = &["claude", "codex", "cursor", "antigravity", "grok"];

/// Fetch usage for one provider. `None` when the name is not a known provider,
/// which lets a caller tell "unknown provider" apart from "provider failed".
pub async fn fetch_provider(name: &str) -> Option<Result<ProviderUsage, String>> {
    let result = match name {
        "claude" => crate::claude::local_usage::fetch_local_usage()
            .await
            .map_err(|err| err.to_string()),
        "codex" => crate::codex::local_usage::fetch_local_usage()
            .await
            .map_err(|err| err.to_string()),
        "cursor" => crate::cursor::local_usage::fetch_local_usage()
            .await
            .map_err(|err| err.to_string()),
        "antigravity" => crate::antigravity::local_usage::fetch_local_usage()
            .await
            .map_err(|err| err.to_string()),
        "grok" => crate::grok::local_usage::fetch_local_usage()
            .await
            .map_err(|err| err.to_string()),
        _ => return None,
    };
    Some(result)
}

/// Fetch every provider in registry order.
///
/// The calls run one after another rather than together. A provider the user is
/// not signed into fails on the local file read without touching the network,
/// so the usual cost is one or two real requests.
pub async fn fetch_all() -> Vec<(&'static str, Result<ProviderUsage, String>)> {
    let mut reports = Vec::with_capacity(PROVIDERS.len());
    for name in PROVIDERS {
        if let Some(result) = fetch_provider(name).await {
            reports.push((*name, result));
        }
    }
    reports
}
