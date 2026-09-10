//! Shared provider logic for Quota, free of any Tauri dependency so both the
//! desktop app and the standalone CLI can build from it.

pub mod usage;
pub mod claude;
