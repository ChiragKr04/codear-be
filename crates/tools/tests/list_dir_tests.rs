use super::list_dir::*;
use super::{Sandbox, ToolResult};

use std::fs;
use std::path::PathBuf;

/// Creates a fresh, empty temp directory unique to `name`.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("codear-tools-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn list(sb: &Sandbox, path: &str, depth: Option<usize>) -> ToolResult {
    list_dir(sb, ListDirArgs { path: path.into(), depth })
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

    let out = list(&Sandbox::new(&dir).unwrap(), ".", None).unwrap();
    let body: Vec<&str> = out.lines().skip(1).collect();
    // depth 2: `deeper/` (level 3) is not shown.
    assert_eq!(body, ["  Cargo.toml", "  src/", "    main.rs", "    nested/"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_dir_respects_depth() {
    let dir = temp_dir("depth");
    fs::create_dir_all(dir.join("a/b")).unwrap();

    let out = list(&Sandbox::new(&dir).unwrap(), ".", Some(1)).unwrap();
    assert_eq!(out.lines().skip(1).collect::<Vec<_>>(), ["  a/"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_dir_rejects_non_directory() {
    let dir = temp_dir("nondir");
    let sb = Sandbox::new(&dir).unwrap();
    assert!(list(&sb, "missing", None).unwrap_err().to_string().contains("not a directory"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn list_dir_refuses_paths_outside_the_sandbox() {
    let dir = temp_dir("outside");
    let sb = Sandbox::new(&dir).unwrap();
    for path in ["/", "..", "/etc"] {
        let err = list(&sb, path, None).unwrap_err().to_string();
        assert!(err.contains("outside the repo"), "{path}: {err}");
    }
    let _ = fs::remove_dir_all(&dir);
}
