//! `write_file`: create or overwrite a file.

use std::fs;

use serde::Deserialize;

use super::{Sandbox, ToolError, ToolResult};

/// Above this many line comparisons the diff counts are approximated
/// (every differing middle line counted as removed + added) to stay fast.
pub const MAX_DIFF_CELLS: usize = 4_000_000;

#[derive(Debug, Clone, Deserialize)]
pub struct WriteFileArgs {
    pub path: String,
    pub content: String,
}

/// Creates or overwrites `path` (creating parent folders). Returns a diff summary:
///
/// - `created <path> (+N lines, B bytes)`
/// - `updated <path>: +A -R lines (now N lines, B bytes)`
/// - `unchanged <path> (N lines)`, in which case nothing is written.
///
/// Lines keep their `\n`, so adding or dropping a trailing newline counts as a change.
pub fn write_file(sb: &Sandbox, args: WriteFileArgs) -> ToolResult {
    if args.path.trim().is_empty() {
        return Err(ToolError::InvalidArgs("path is empty".into()));
    }
    let path = sb.resolve(&args.path)?;
    if path.is_dir() {
        return Err(ToolError::Failed(format!("is a directory: {}", args.path)));
    }

    let old = match fs::read(&path) {
        Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };

    let new_lines = count_lines(&args.content);
    let bytes = args.content.len();

    if old.as_deref() == Some(args.content.as_str()) {
        return Ok(format!("unchanged {} ({new_lines} lines)", args.path));
    }

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, &args.content)?;

    Ok(match old {
        None => format!("created {} (+{new_lines} lines, {bytes} bytes)", args.path),
        Some(old) => {
            let d = line_diff(&old, &args.content);
            let approx = if d.approximate { " (approx.)" } else { "" };
            format!(
                "updated {}: +{} -{} lines{approx} (now {new_lines} lines, {bytes} bytes)",
                args.path, d.added, d.removed
            )
        }
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct LineDiff {
    pub added: usize,
    pub removed: usize,
    /// True when the input was too large for an exact count.
    pub approximate: bool,
}

/// Counts lines added and removed going from `old` to `new` (minimal edit,
/// via longest common subsequence after trimming the shared prefix and suffix).
pub fn line_diff(old: &str, new: &str) -> LineDiff {
    let a: Vec<&str> = old.split_inclusive('\n').collect();
    let b: Vec<&str> = new.split_inclusive('\n').collect();

    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[prefix..], &b[prefix..]);
    let suffix = a.iter().rev().zip(b.iter().rev()).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[..a.len() - suffix], &b[..b.len() - suffix]);

    if a.is_empty() || b.is_empty() || a.len().saturating_mul(b.len()) > MAX_DIFF_CELLS {
        return LineDiff {
            added: b.len(),
            removed: a.len(),
            approximate: !a.is_empty() && !b.is_empty(),
        };
    }

    let common = lcs_len(a, b);
    LineDiff { added: b.len() - common, removed: a.len() - common, approximate: false }
}

/// Length of the longest common subsequence of `a` and `b`, in O(len(b)) memory.
fn lcs_len(a: &[&str], b: &[&str]) -> usize {
    let mut prev = vec![0usize; b.len() + 1];
    let mut cur = vec![0usize; b.len() + 1];
    for x in a {
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = if x == y { prev[j] + 1 } else { prev[j + 1].max(cur[j]) };
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Line count where a final line without `\n` still counts.
fn count_lines(s: &str) -> usize {
    s.split_inclusive('\n').count()
}
