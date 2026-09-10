//! Grok provider support for the standalone CLI.
//!
//! Only the local-credential path lives here. The desktop app keeps its own
//! account store, OAuth device flow, and token refresh in
//! `src-tauri/src/grok.rs`, none of which the CLI needs or is allowed to touch.

pub mod local;
pub mod local_usage;
