//! `search`: regex grep across files.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use regex::Regex;
use serde::Deserialize;

use super::list_dir::SKIP;
use super::{ToolError, ToolResult};

pub const MAX_MATCHES: usize = 200;
/// Matched lines longer than this (in chars) are cut, so one minified file
/// can't flood the output.
pub const MAX_LINE_CHARS: usize = 300;
/// A file with a NUL byte in its first `BINARY_SNIFF_BYTES` is treated as binary and skipped.
const BINARY_SNIFF_BYTES: usize = 8 * 1024;

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
///
/// Files are visited in name order, so output is stable. `.gitignore`,
/// `.ignore` and global git excludes apply even outside a git repo; hidden
/// files and `SKIP` dirs are skipped, as are binary files. `glob` is
/// gitignore-style and relative to `path`: `*.rs` matches at any depth.
/// Invalid UTF-8 is replaced with `U+FFFD`.
pub fn search(args: SearchArgs) -> ToolResult {
    let re = Regex::new(&args.pattern)
        .map_err(|e| ToolError::InvalidArgs(format!("bad regex: {e}")))?;

    let root_str = args.path.as_deref().unwrap_or(".");
    let root = Path::new(root_str);
    if !root.exists() {
        return Err(ToolError::Failed(format!("no such file or directory: {root_str}")));
    }

    let mut walk = WalkBuilder::new(root);
    walk.require_git(false)
        .sort_by_file_name(|a, b| a.cmp(b))
        .filter_entry(|e| !SKIP.contains(&e.file_name().to_string_lossy().as_ref()));
    if let Some(glob) = &args.glob {
        let overrides = OverrideBuilder::new(root)
            .add(glob)
            .and_then(|b| b.build())
            .map_err(|e| ToolError::InvalidArgs(format!("bad glob: {e}")))?;
        walk.overrides(overrides);
    }

    let mut out = Vec::new();
    for entry in walk.build().filter_map(Result::ok) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let display = display_path(entry.path());
        // Unreadable files are skipped rather than failing the whole search.
        let _ = search_file(entry.path(), &display, &re, &mut out);
        if out.len() > MAX_MATCHES {
            break;
        }
    }

    if out.is_empty() {
        return Ok("[no matches]".into());
    }
    if out.len() > MAX_MATCHES {
        out.truncate(MAX_MATCHES);
        out.push(format!(
            "[cut at {MAX_MATCHES} matches; narrow with `path` or `glob`]"
        ));
    }
    Ok(out.join("\n"))
}

/// Appends `file:line: text` for each line of `path` matching `re`, stopping
/// once `out` holds more than `MAX_MATCHES` (the extra one tells the caller
/// output was cut).
fn search_file(path: &Path, display: &str, re: &Regex, out: &mut Vec<String>) -> std::io::Result<()> {
    let mut reader = BufReader::new(File::open(path)?);

    // Peek at the head without consuming it, to skip binaries.
    let mut head = Vec::with_capacity(BINARY_SNIFF_BYTES);
    (&mut reader).take(BINARY_SNIFF_BYTES as u64).read_to_end(&mut head)?;
    if head.contains(&0) {
        return Ok(());
    }
    let mut reader = head.as_slice().chain(reader);

    let mut buf = Vec::new();
    let mut line_no = 0;
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            return Ok(());
        }
        line_no += 1;
        let text = String::from_utf8_lossy(strip_newline(&buf));
        if re.is_match(&text) {
            out.push(format!("{display}:{line_no}: {}", clip(&text)));
            if out.len() > MAX_MATCHES {
                return Ok(());
            }
        }
    }
}

/// `path` as shown in output: the leading `./` of the default root dropped.
fn display_path(path: &Path) -> String {
    let path = path.strip_prefix(".").unwrap_or(path);
    path.to_string_lossy().into_owned()
}

/// `text` cut to `MAX_LINE_CHARS` chars, with a marker when cut.
fn clip(text: &str) -> String {
    match text.char_indices().nth(MAX_LINE_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}

fn strip_newline(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}
