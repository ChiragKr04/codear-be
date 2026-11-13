//! `edit_file`: replace one exact match in a file.

use std::fs;

use serde::Deserialize;

use super::{Sandbox, ToolError, ToolResult};

/// At most this many match line numbers are listed in the "too many matches" error.
pub const MAX_LISTED_MATCHES: usize = 10;

#[derive(Debug, Clone, Deserialize)]
pub struct EditFileArgs {
    pub path: String,
    pub old_str: String,
    pub new_str: String,
}

/// Replaces the single exact match of `old_str` with `new_str`. Errors if
/// `old_str` matches 0 or 2+ times, naming the count.
///
/// Matching is byte-exact (whitespace and line endings included) and counts
/// overlapping occurrences, so `"aa"` in `"aaa"` is 2 matches, not 1: an edit
/// is only made when its location is unambiguous.
///
/// On success returns `edited <path> at line L: -R +A lines (now N lines)`,
/// where R and A are the line counts of `old_str` and `new_str`.
pub fn edit_file(sb: &Sandbox, args: EditFileArgs) -> ToolResult {
    if args.path.trim().is_empty() {
        return Err(ToolError::InvalidArgs("path is empty".into()));
    }
    if args.old_str.is_empty() {
        return Err(ToolError::InvalidArgs(
            "old_str is empty; use write_file to create or overwrite a file".into(),
        ));
    }
    if args.old_str == args.new_str {
        return Err(ToolError::InvalidArgs("old_str and new_str are identical".into()));
    }

    let path = sb.resolve(&args.path)?;
    if path.is_dir() {
        return Err(ToolError::Failed(format!("is a directory: {}", args.path)));
    }
    // Strict UTF-8: a lossy decode would corrupt the file when written back.
    let content = String::from_utf8(fs::read(&path)?)
        .map_err(|_| ToolError::Failed(format!("not valid UTF-8: {}", args.path)))?;

    let matches = find_all(&content, &args.old_str);
    let at = match matches.as_slice() {
        [at] => *at,
        [] => {
            return Err(ToolError::Failed(format!(
                "old_str not found in {} (0 matches); it must match exactly, \
                 including whitespace and indentation",
                args.path
            )));
        }
        many => {
            return Err(ToolError::Failed(format!(
                "old_str matches {} times in {} ({}); include more surrounding \
                 context so it matches exactly once",
                many.len(),
                args.path,
                describe_lines(&content, many)
            )));
        }
    };

    let mut updated = String::with_capacity(content.len() - args.old_str.len() + args.new_str.len());
    updated.push_str(&content[..at]);
    updated.push_str(&args.new_str);
    updated.push_str(&content[at + args.old_str.len()..]);
    fs::write(&path, &updated)?;

    Ok(format!(
        "edited {} at line {}: -{} +{} lines (now {} lines)",
        args.path,
        line_of(&content, at),
        count_lines(&args.old_str),
        count_lines(&args.new_str),
        count_lines(&updated)
    ))
}

/// Byte offsets of every occurrence of `needle` in `haystack`, overlapping included.
pub fn find_all(haystack: &str, needle: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(i) = haystack[from..].find(needle) {
        let at = from + i;
        found.push(at);
        // Step one char (not one byte) past the match start to stay on a char boundary.
        from = at + haystack[at..].chars().next().map_or(1, char::len_utf8);
    }
    found
}

/// `at lines 3, 9, 12` (or `... and N more` past `MAX_LISTED_MATCHES`).
fn describe_lines(content: &str, offsets: &[usize]) -> String {
    let shown: Vec<String> = offsets
        .iter()
        .take(MAX_LISTED_MATCHES)
        .map(|&at| line_of(content, at).to_string())
        .collect();
    let mut out = format!("at lines {}", shown.join(", "));
    if offsets.len() > MAX_LISTED_MATCHES {
        out.push_str(&format!(" and {} more", offsets.len() - MAX_LISTED_MATCHES));
    }
    out
}

/// 1-based line number containing byte offset `at`.
fn line_of(content: &str, at: usize) -> usize {
    content[..at].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Line count where a final line without `\n` still counts.
fn count_lines(s: &str) -> usize {
    s.split_inclusive('\n').count()
}
