//! `search`: regex grep across files.

use serde::Deserialize;

use super::ToolResult;

pub const MAX_MATCHES: usize = 200;

#[derive(Debug, Clone, Deserialize)]
pub struct SearchArgs {
    /// Regex pattern.
    pub pattern: String,
    /// Directory or file to search; defaults to the repo root.
    pub path: Option<String>,
    /// Restrict to files matching this glob, e.g. `*.rs`.
    pub glob: Option<String>,
}

/// Regex search across files, respecting `.gitignore`. Returns `file:line: text`,
/// at most `MAX_MATCHES` matches.
pub fn search(args: SearchArgs) -> ToolResult {
    // TODO: real implementation. Placeholder output for now.
    let path = args.path.as_deref().unwrap_or(".");
    let glob = args.glob.as_deref().unwrap_or("*");
    Ok(format!(
        "[stub search pattern={:?} path={path} glob={glob}]\nsrc/main.rs:1: sample match for {}",
        args.pattern, args.pattern
    ))
}
