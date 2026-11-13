#![allow(dead_code)] // not every tool is wired into the agent loop yet.

//! Tools the model can call. Each tool lives in its own file with its args
//! struct, limits and logic; this file only registers them and dispatches.
//!
//! Adding a tool: create `<name>.rs`, add `pub mod <name>;` + `pub use <name>::<name>;` below,
//! then add it to `TOOL_NAMES`, `sample_args` and `dispatch`.

mod error;

pub mod edit_file;
pub mod list_dir;
pub mod read_file;
pub mod run_command;
pub mod sandbox;
pub mod search;
pub mod write_file;

pub use edit_file::edit_file;
pub use error::{ToolError, ToolResult};
pub use list_dir::list_dir;
pub use read_file::read_file;
pub use run_command::run_command;
pub use sandbox::Sandbox;
pub use search::search;
pub use write_file::write_file;

use serde::Deserialize;

/// Names of every tool `dispatch` accepts.
pub const TOOL_NAMES: &[&str] =
    &["list_dir", "read_file", "search", "write_file", "edit_file", "run_command"];

/// Example JSON arguments for each tool, used when none are given (e.g. from the CLI).
pub fn sample_args(name: &str) -> Option<&'static str> {
    Some(match name {
        "list_dir" => r#"{"path": "."}"#,
        "read_file" => r#"{"path": "src/main.rs", "start_line": 1, "end_line": 5}"#,
        "search" => r#"{"pattern": "fn main", "glob": "*.rs"}"#,
        "write_file" => r#"{"path": "tmp/hello.txt", "content": "hello\nworld\n"}"#,
        "edit_file" => r#"{"path": "tmp/hello.txt", "old_str": "world", "new_str": "there"}"#,
        "run_command" => r#"{"command": "echo hi"}"#,
        _ => return None,
    })
}

/// Runs the tool called `name` with its JSON-encoded `args` (as sent by the model).
/// Every path the model gives is resolved inside `sb`.
pub fn dispatch(sb: &Sandbox, name: &str, args: &str) -> ToolResult {
    match name {
        "list_dir" => list_dir(sb, parse(args)?),
        "read_file" => read_file(sb, parse(args)?),
        "search" => search(sb, parse(args)?),
        "write_file" => write_file(sb, parse(args)?),
        "edit_file" => edit_file(sb, parse(args)?),
        "run_command" => run_command(sb, parse(args)?),
        other => Err(ToolError::UnknownTool(other.to_string())),
    }
}

fn parse<'a, T: Deserialize<'a>>(args: &'a str) -> Result<T, ToolError> {
    serde_json::from_str(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))
}

#[cfg(test)]
#[path = "tests/sandbox_tests.rs"]
mod sandbox_tests;

#[cfg(test)]
#[path = "tests/list_dir_tests.rs"]
mod list_dir_tests;

#[cfg(test)]
#[path = "tests/read_file_tests.rs"]
mod read_file_tests;

#[cfg(test)]
#[path = "tests/search_tests.rs"]
mod search_tests;

#[cfg(test)]
#[path = "tests/write_file_tests.rs"]
mod write_file_tests;

#[cfg(test)]
#[path = "tests/edit_file_tests.rs"]
mod edit_file_tests;

#[cfg(test)]
#[path = "tests/run_command_tests.rs"]
mod run_command_tests;
