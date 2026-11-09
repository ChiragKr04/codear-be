//! `run_command`: run a shell command in the repo root.

use serde::Deserialize;

use super::ToolResult;

pub const DEFAULT_TIMEOUT_SECS: u64 = 60;
pub const MAX_TIMEOUT_SECS: u64 = 300;
/// Only the last this-many bytes of stdout/stderr are kept.
pub const MAX_OUTPUT_BYTES: usize = 30 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct RunCommandArgs {
    pub command: String,
    /// Defaults to `DEFAULT_TIMEOUT_SECS`, capped at `MAX_TIMEOUT_SECS`.
    pub timeout_secs: Option<u64>,
}

/// Runs `sh -c <command>` in the repo root with no stdin. Returns the exit code,
/// stdout and stderr, keeping the last `MAX_OUTPUT_BYTES` of each.
pub fn run_command(args: RunCommandArgs) -> ToolResult {
    // TODO: real implementation. Nothing is run yet.
    let timeout = args.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS).min(MAX_TIMEOUT_SECS);
    Ok(format!(
        "[stub run_command timeout={timeout}s] would run: {}\nexit code: 0\nstdout:\nsample output\nstderr:\n",
        args.command
    ))
}
