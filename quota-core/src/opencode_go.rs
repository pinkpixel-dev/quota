//! OpenCode Go provider support for the standalone CLI.
//!
//! Only the local-credential path lives here. The desktop app keeps its own
//! account store for Go API keys in `src-tauri/src/opencode_go.rs`, none of
//! which the CLI needs or is allowed to touch.

pub mod local;
pub mod local_usage;
