//! `read_file`: numbered lines of a file.

use serde::Deserialize;

use super::ToolResult;

pub const MAX_LINES: usize = 2_000;
pub const MAX_BYTES: usize = 100 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct ReadFileArgs {
    pub path: String,
    /// 1-based, inclusive.
    pub start_line: Option<usize>,
    /// 1-based, inclusive.
    pub end_line: Option<usize>,
}

/// Returns numbered lines of `path`, optionally limited to a line range.
/// At most `MAX_LINES` lines or `MAX_BYTES`, with a note when cut.
pub fn read_file(args: ReadFileArgs) -> ToolResult {
    // TODO: real implementation. Placeholder output for now.
    let start = args.start_line.unwrap_or(1);
    let end = args.end_line.unwrap_or(start + 2);
    let lines: Vec<String> = (start..=end).map(|n| format!("{n:>6}\tsample line {n}")).collect();
    Ok(format!("[stub read_file path={}]\n{}", args.path, lines.join("\n")))
}
