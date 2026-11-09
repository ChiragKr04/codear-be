use super::list_dir::*;
use super::ToolResult;

use std::fs;
use std::path::{Path, PathBuf};

/// Creates a fresh, empty temp directory unique to `name`.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("codear-tools-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn list(path: &Path, depth: Option<usize>) -> ToolResult {
    list_dir(ListDirArgs { path: path.to_string_lossy().into_owned(), depth })
}

#[test]
fn list_dir_shows_sorted_tree_and_skips_noise() {
    let dir = temp_dir("tree");
    fs::create_dir_all(dir.join("src/nested/deeper")).unwrap();
    fs::write(dir.join("src/main.rs"), "").unwrap();
    fs::write(dir.join("Cargo.toml"), "").unwrap();
    for skipped in SKIP {
        fs::create_dir_all(dir.join(skipped).join("inner")).unwrap();
    }

    let out = list(&dir, None).unwrap();
    let body: Vec<&str> = out.lines().skip(1).collect();
    // depth 2: `deeper/` (level 3) is not shown.
    assert_eq!(body, ["  Cargo.toml", "  src/", "    main.rs", "    nested/"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_dir_respects_depth() {
    let dir = temp_dir("depth");
    fs::create_dir_all(dir.join("a/b")).unwrap();

    let out = list(&dir, Some(1)).unwrap();
    assert_eq!(out.lines().skip(1).collect::<Vec<_>>(), ["  a/"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_dir_rejects_non_directory() {
    let err = list(Path::new("/definitely/not/here"), None).unwrap_err();
    assert!(err.to_string().contains("not a directory"));
}
