use super::write_file::*;
use super::ToolResult;

use std::fs;
use std::path::PathBuf;

/// Fresh, empty temp directory unique to `name`.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("codear-write-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &PathBuf, content: &str) -> ToolResult {
    write_file(WriteFileArgs {
        path: path.to_string_lossy().into_owned(),
        content: content.into(),
    })
}

#[test]
fn creates_file_and_parent_folders() {
    let dir = temp_dir("create");
    let path = dir.join("a/b/c/hello.txt");
    let out = write(&path, "hello\nworld\n").unwrap();
    assert_eq!(out, format!("created {} (+2 lines, 12 bytes)", path.display()));
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello\nworld\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn overwrites_and_reports_diff() {
    let dir = temp_dir("overwrite");
    let path = dir.join("f.txt");
    fs::write(&path, "a\nb\nc\nd\n").unwrap();
    let out = write(&path, "a\nB\nc\nd\ne\n").unwrap();
    assert_eq!(out, format!("updated {}: +2 -1 lines (now 5 lines, 10 bytes)", path.display()));
    assert_eq!(fs::read_to_string(&path).unwrap(), "a\nB\nc\nd\ne\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unchanged_content_is_reported() {
    let dir = temp_dir("unchanged");
    let path = dir.join("f.txt");
    fs::write(&path, "same\n").unwrap();
    assert_eq!(write(&path, "same\n").unwrap(), format!("unchanged {} (1 lines)", path.display()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_content_creates_empty_file() {
    let dir = temp_dir("empty");
    let path = dir.join("empty.txt");
    assert_eq!(write(&path, "").unwrap(), format!("created {} (+0 lines, 0 bytes)", path.display()));
    assert_eq!(fs::read(&path).unwrap(), b"");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_directory_and_empty_path() {
    let dir = temp_dir("isdir");
    assert!(write(&dir, "x").unwrap_err().to_string().contains("is a directory"));
    assert!(write(&PathBuf::from("  "), "x").is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn diff_counts() {
    let d = |a, b| {
        let d = line_diff(a, b);
        (d.added, d.removed)
    };
    assert_eq!(d("", "x\ny\n"), (2, 0));
    assert_eq!(d("x\ny\n", ""), (0, 2));
    assert_eq!(d("a\nb\nc\n", "a\nc\n"), (0, 1));
    assert_eq!(d("a\nb\nc\n", "c\nb\na\n"), (2, 2));
    // A trailing newline change is a changed line.
    assert_eq!(d("a\nb", "a\nb\n"), (1, 1));
}

#[test]
fn huge_diff_is_approximate() {
    let old: String = (0..3000).map(|n| format!("old {n}\n")).collect();
    let new: String = (0..3000).map(|n| format!("new {n}\n")).collect();
    let d = line_diff(&old, &new);
    assert_eq!((d.added, d.removed, d.approximate), (3000, 3000, true));
}
