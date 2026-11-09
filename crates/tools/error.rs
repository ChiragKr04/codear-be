//! Error and result types shared by every tool.

use std::fmt;

pub type ToolResult = Result<String, ToolError>;

#[derive(Debug)]
pub enum ToolError {
    /// Tool name the model asked for does not exist.
    UnknownTool(String),
    /// Arguments JSON did not match the tool's args struct.
    InvalidArgs(String),
    /// I/O failure (missing file, permission denied, ...).
    Io(std::io::Error),
    /// Any other tool-specific failure (e.g. `edit_file` match count).
    Failed(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::UnknownTool(name) => write!(f, "unknown tool: {name}"),
            ToolError::InvalidArgs(msg) => write!(f, "invalid arguments: {msg}"),
            ToolError::Io(err) => write!(f, "io error: {err}"),
            ToolError::Failed(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ToolError {}

impl From<std::io::Error> for ToolError {
    fn from(err: std::io::Error) -> Self {
        ToolError::Io(err)
    }
}
