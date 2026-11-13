//! `read_file`: numbered lines of a file.

use std::fs::File;
use std::io::{BufRead, BufReader};

use serde::Deserialize;

use super::{Sandbox, ToolError, ToolResult};

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

/// Returns numbered lines of `path` (inside the sandbox), optionally limited to a line range.
/// At most `MAX_LINES` lines or `MAX_BYTES`, with a note when cut.
///
/// Each line is `{number:>6}\t{text}`. The file is streamed: lines before
/// `start_line` are skipped without being kept, and reading stops at
/// `end_line` or a limit. Invalid UTF-8 is replaced with `U+FFFD`.
pub fn read_file(sb: &Sandbox, args: ReadFileArgs) -> ToolResult {
    let start = args.start_line.unwrap_or(1);
    if start == 0 {
        return Err(ToolError::InvalidArgs("start_line is 1-based; got 0".into()));
    }
    if let Some(end) = args.end_line {
        if end < start {
            return Err(ToolError::InvalidArgs(format!(
                "end_line ({end}) is before start_line ({start})"
            )));
        }
    }

    let path = sb.resolve(&args.path)?;
    if path.is_dir() {
        return Err(ToolError::Failed(format!("is a directory: {}", args.path)));
    }
    let file = File::open(&path)
        .map_err(|e| ToolError::Failed(format!("cannot open {}: {e}", args.path)))?;
    let mut reader = BufReader::new(file);

    let mut out = String::new();
    let mut shown = 0;
    let mut line_no = 0;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break; // EOF
        }
        line_no += 1;
        if line_no < start {
            continue;
        }
        if args.end_line.is_some_and(|end| line_no > end) {
            break;
        }

        let text = String::from_utf8_lossy(strip_newline(&buf));
        let line = format!("{line_no:>6}\t{text}\n");

        if shown == MAX_LINES {
            out.push_str(&cut_note(&format!("{MAX_LINES} lines"), line_no));
            break;
        }
        if out.len() + line.len() > MAX_BYTES {
            if shown == 0 {
                // A single line bigger than the whole budget: show its head.
                out.push_str(truncate_at_char(&line, MAX_BYTES));
                out.push('\n');
                out.push_str(&format!("[line {line_no} cut at {MAX_BYTES} bytes]"));
            } else {
                out.push_str(&cut_note(&format!("{MAX_BYTES} bytes"), line_no));
            }
            break;
        }
        out.push_str(&line);
        shown += 1;
    }

    if shown == 0 && out.is_empty() {
        return if line_no == 0 {
            Ok("[empty file]".into())
        } else {
            Err(ToolError::Failed(format!(
                "start_line {start} is past the end of the file ({line_no} lines)"
            )))
        };
    }
    Ok(out.trim_end_matches('\n').to_string())
}

/// Note appended when output stops early; `next` is the first line not shown.
fn cut_note(limit: &str, next: usize) -> String {
    format!("[cut at {limit}; continue with start_line={next}]")
}

fn strip_newline(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// Longest prefix of `s` that is at most `max` bytes and ends on a char boundary.
fn truncate_at_char(s: &str, max: usize) -> &str {
    let mut end = max.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
