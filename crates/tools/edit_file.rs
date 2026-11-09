//! `edit_file`: replace one exact match in a file.

use serde::Deserialize;

use super::ToolResult;

#[derive(Debug, Clone, Deserialize)]
pub struct EditFileArgs {
    pub path: String,
    pub old_str: String,
    pub new_str: String,
}

/// Replaces the single exact match of `old_str` with `new_str`. Errors if
/// `old_str` matches 0 or 2+ times, naming the count.
pub fn edit_file(args: EditFileArgs) -> ToolResult {
    // TODO: real implementation. Nothing is edited yet.
    Ok(format!(
        "[stub edit_file] would replace {:?} with {:?} in {}",
        args.old_str, args.new_str, args.path
    ))
}
