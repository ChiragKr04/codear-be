use super::read_file::*;
use super::{Sandbox, ToolResult};

use std::fs;
use std::path::PathBuf;

/// Fresh temp dir unique to `name` holding `f.txt` with `content`, and a sandbox on it.
fn setup(name: &str, content: &[u8]) -> (Sandbox, PathBuf) {
    let dir = std::env::temp_dir().join(format!("codear-read-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("f.txt"), content).unwrap();
    (Sandbox::new(&dir).unwrap(), dir)
}

fn read(sb: &Sandbox, path: &str, start: Option<usize>, end: Option<usize>) -> ToolResult {
    read_file(sb, ReadFileArgs { path: path.into(), start_line: start, end_line: end })
}

fn numbered(lines: &[(usize, &str)]) -> String {
    lines.iter().map(|(n, t)| format!("{n:>6}\t{t}")).collect::<Vec<_>>().join("\n")
}

#[test]
fn reads_whole_file_numbered() {
    let (sb, dir) = setup("whole", b"a\r\nb\nc");
    assert_eq!(read(&sb, "f.txt", None, None).unwrap(), numbered(&[(1, "a"), (2, "b"), (3, "c")]));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reads_range_and_end_past_eof() {
    let (sb, dir) = setup("range", b"1\n2\n3\n4\n5\n");
    assert_eq!(read(&sb, "f.txt", Some(2), Some(3)).unwrap(), numbered(&[(2, "2"), (3, "3")]));
    assert_eq!(read(&sb, "f.txt", Some(4), Some(99)).unwrap(), numbered(&[(4, "4"), (5, "5")]));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_bad_ranges() {
    let (sb, dir) = setup("bad", b"x\n");
    assert!(read(&sb, "f.txt", Some(0), None).is_err());
    assert!(read(&sb, "f.txt", Some(3), Some(2)).is_err());
    assert!(read(&sb, "f.txt", Some(5), None).unwrap_err().to_string().contains("past the end"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_file_and_missing_file() {
    let (sb, dir) = setup("empty", b"");
    assert_eq!(read(&sb, "f.txt", None, None).unwrap(), "[empty file]");
    let err = read(&sb, "missing.txt", None, None).unwrap_err().to_string();
    assert!(err.starts_with("cannot open missing.txt:"), "{err}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_paths_outside_the_sandbox() {
    let (sb, dir) = setup("outside", b"x\n");
    for path in ["/etc/passwd", "../f.txt", "a/../../f.txt"] {
        let err = read(&sb, path, None, None).unwrap_err().to_string();
        assert!(err.contains("outside the repo"), "{path}: {err}");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn relative_paths_resolve_against_the_sandbox_not_cwd() {
    // `Cargo.toml` exists in the cwd during `cargo test`, but not in the sandbox.
    let (sb, dir) = setup("notcwd", b"x\n");
    assert!(read(&sb, "Cargo.toml", None, None).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_directory() {
    let (sb, dir) = setup("isdir", b"x\n");
    assert!(read(&sb, ".", None, None).unwrap_err().to_string().contains("is a directory"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cuts_at_max_lines() {
    let content: String = (1..=MAX_LINES + 10).map(|n| format!("{n}\n")).collect();
    let (sb, dir) = setup("maxlines", content.as_bytes());
    let out = read(&sb, "f.txt", None, None).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), MAX_LINES + 1);
    assert_eq!(
        *lines.last().unwrap(),
        format!("[cut at {MAX_LINES} lines; continue with start_line={}]", MAX_LINES + 1)
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cuts_at_max_bytes() {
    let line = "x".repeat(1000);
    let content: String = (0..200).map(|_| format!("{line}\n")).collect();
    let (sb, dir) = setup("maxbytes", content.as_bytes());
    let out = read(&sb, "f.txt", None, None).unwrap();
    let (body, note) = out.rsplit_once('\n').unwrap();
    assert!(body.len() <= MAX_BYTES);
    assert!(note.starts_with(&format!("[cut at {MAX_BYTES} bytes; continue with start_line=")));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn single_huge_line_is_truncated() {
    let (sb, dir) = setup("huge", "é".repeat(MAX_BYTES).as_bytes());
    let out = read(&sb, "f.txt", None, None).unwrap();
    assert!(out.ends_with(&format!("[line 1 cut at {MAX_BYTES} bytes]")));
    assert!(out.len() <= MAX_BYTES + 64);
    let _ = fs::remove_dir_all(&dir);
}
