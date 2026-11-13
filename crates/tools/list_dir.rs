//! `list_dir`: tree listing of a directory.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::{Sandbox, ToolError, ToolResult};

pub const DEFAULT_DEPTH: usize = 2;
pub const MAX_ENTRIES: usize = usize::MAX;
/// Always skipped, whatever `.gitignore` says.
pub const SKIP: &[&str] = &[".git", "target", "node_modules"];

#[derive(Debug, Clone, Deserialize)]
pub struct ListDirArgs {
    pub path: String,
    /// Defaults to `DEFAULT_DEPTH`.
    pub depth: Option<usize>,
}

/// Tree listing of `path` (relative to the sandbox root) down to
/// `depth` levels. Directories end in `/`; entries are sorted by name and
/// indented two spaces per level. Skips `SKIP`; at most `MAX_ENTRIES` entries,
/// with a note when cut.
// TODO: also skip `.gitignore` matches.
pub fn list_dir(sb: &Sandbox, args: ListDirArgs) -> ToolResult {
    let root = sb.resolve(&args.path)?;
    if !root.is_dir() {
        return Err(ToolError::Failed(format!("not a directory: {}", args.path)));
    }

    let mut tree = Tree {
        lines: vec![format!("{}/", args.path.trim_end_matches('/'))],
        entries: 0,
        max_depth: args.depth.unwrap_or(DEFAULT_DEPTH),
    };
    fs::read_dir(&root)?; // surface an unreadable root as an error, not an empty tree
    if tree.walk(&root, 1) == Walk::Full {
        tree.lines.push(format!("[cut at {MAX_ENTRIES} entries]"));
    }
    Ok(tree.lines.join("\n"))
}

#[derive(PartialEq)]
enum Walk {
    Continue,
    /// `MAX_ENTRIES` reached; stop everywhere.
    Full,
}

/// Output being built, plus the limits that apply across the whole walk.
struct Tree {
    lines: Vec<String>,
    entries: usize,
    max_depth: usize,
}

impl Tree {
    /// Appends the children of `dir`, which sit at `level` (1-based).
    fn walk(&mut self, dir: &Path, level: usize) -> Walk {
        if level > self.max_depth {
            return Walk::Continue;
        }
        let indent = "  ".repeat(level);

        for child in sorted_children(dir) {
            if self.entries >= MAX_ENTRIES {
                return Walk::Full;
            }
            self.entries += 1;

            let name = child.file_name().to_string_lossy().into_owned();
            // `file_type` does not follow symlinks, so a symlinked dir is
            // listed but not descended into (avoids cycles).
            if child.file_type().is_ok_and(|t| t.is_dir()) {
                self.lines.push(format!("{indent}{name}/"));
                if self.walk(&child.path(), level + 1) == Walk::Full {
                    return Walk::Full;
                }
            } else {
                self.lines.push(format!("{indent}{name}"));
            }
        }
        Walk::Continue
    }
}

/// Entries of `dir` sorted by name, minus `SKIP`. An unreadable directory
/// yields nothing.
fn sorted_children(dir: &Path) -> Vec<fs::DirEntry> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut children: Vec<_> = read
        .filter_map(Result::ok)
        .filter(|e| !SKIP.contains(&e.file_name().to_string_lossy().as_ref()))
        .collect();
    children.sort_by_key(|e| e.file_name());
    children
}
