//! `write_file`: create or overwrite a file.

use serde::Deserialize;

use super::ToolResult;

#[derive(Debug, Clone, Deserialize)]
pub struct WriteFileArgs {
    pub path: String,
    pub content: String,
}

/// Creates or overwrites `path` (creating parent folders). Returns a diff summary.
pub fn write_file(args: WriteFileArgs) -> ToolResult {
    // TODO: real implementation. Nothing is written yet.
    let lines = args.content.lines().count();
    Ok(format!("[stub write_file] would write {}: +{lines} -0 lines", args.path))
}
