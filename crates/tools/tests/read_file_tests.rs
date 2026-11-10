use super::read_file::*;
use super::ToolResult;

use std::fs;
use std::path::PathBuf;

/// Writes `content` to a fresh temp file unique to `name`.
fn temp_file(name: &str, content: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("codear-read-{name}-{}", std::process::id()));
    fs::write(&path, content).unwrap();
    path
}

fn read(path: &PathBuf, start: Option<usize>, end: Option<usize>) -> ToolResult {
    read_file(ReadFileArgs {
        path: path.to_string_lossy().into_owned(),
        start_line: start,
        end_line: end,
    })
}

fn numbered(lines: &[(usize, &str)]) -> String {
    lines.iter().map(|(n, t)| format!("{n:>6}\t{t}")).collect::<Vec<_>>().join("\n")
}

#[test]
fn reads_whole_file_numbered() {
    let path = temp_file("whole", b"a\r\nb\nc");
    assert_eq!(read(&path, None, None).unwrap(), numbered(&[(1, "a"), (2, "b"), (3, "c")]));
    let _ = fs::remove_file(&path);
}

#[test]
fn reads_range_and_end_past_eof() {
    let path = temp_file("range", b"1\n2\n3\n4\n5\n");
    assert_eq!(read(&path, Some(2), Some(3)).unwrap(), numbered(&[(2, "2"), (3, "3")]));
    assert_eq!(read(&path, Some(4), Some(99)).unwrap(), numbered(&[(4, "4"), (5, "5")]));
    let _ = fs::remove_file(&path);
}

#[test]
fn rejects_bad_ranges() {
    let path = temp_file("bad", b"x\n");
    assert!(read(&path, Some(0), None).is_err());
    assert!(read(&path, Some(3), Some(2)).is_err());
    assert!(read(&path, Some(5), None).unwrap_err().to_string().contains("past the end"));
    let _ = fs::remove_file(&path);
}

#[test]
fn empty_file_and_missing_file() {
    let path = temp_file("empty", b"");
    assert_eq!(read(&path, None, None).unwrap(), "[empty file]");
    let _ = fs::remove_file(&path);
    assert!(read(&PathBuf::from("/definitely/not/here"), None, None).is_err());
}

#[test]
fn cuts_at_max_lines() {
    let content: String = (1..=MAX_LINES + 10).map(|n| format!("{n}\n")).collect();
    let path = temp_file("maxlines", content.as_bytes());
    let out = read(&path, None, None).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), MAX_LINES + 1);
    assert_eq!(
        *lines.last().unwrap(),
        format!("[cut at {MAX_LINES} lines; continue with start_line={}]", MAX_LINES + 1)
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn cuts_at_max_bytes() {
    let line = "x".repeat(1000);
    let content: String = (0..200).map(|_| format!("{line}\n")).collect();
    let path = temp_file("maxbytes", content.as_bytes());
    let out = read(&path, None, None).unwrap();
    let (body, note) = out.rsplit_once('\n').unwrap();
    assert!(body.len() <= MAX_BYTES);
    assert!(note.starts_with(&format!("[cut at {MAX_BYTES} bytes; continue with start_line=")));
    let _ = fs::remove_file(&path);
}

#[test]
fn single_huge_line_is_truncated() {
    let path = temp_file("huge", "é".repeat(MAX_BYTES).as_bytes());
    let out = read(&path, None, None).unwrap();
    assert!(out.ends_with(&format!("[line 1 cut at {MAX_BYTES} bytes]")));
    assert!(out.len() <= MAX_BYTES + 64);
    let _ = fs::remove_file(&path);
}
