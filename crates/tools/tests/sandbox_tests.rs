use super::Sandbox;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

/// Fresh, empty temp directory unique to `name` (canonical, so asserts can compare paths).
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("codear-sandbox-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn refused(sb: &Sandbox, rel: &str) -> bool {
    sb.resolve(rel).is_err_and(|e| e.to_string().contains("outside the repo"))
}

#[test]
fn new_rejects_missing_dir_and_file() {
    assert!(Sandbox::new(Path::new("/definitely/not/here")).is_err());
    let dir = temp_dir("newfile");
    fs::write(dir.join("f"), "").unwrap();
    assert!(Sandbox::new(&dir.join("f")).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn root_is_canonical() {
    let dir = temp_dir("canon");
    let sb = Sandbox::new(&dir.join("./")).unwrap();
    assert_eq!(sb.root(), dir);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn resolves_existing_and_new_paths_inside() {
    let dir = temp_dir("inside");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/main.rs"), "").unwrap();
    let sb = Sandbox::new(&dir).unwrap();

    assert_eq!(sb.resolve("src/main.rs").unwrap(), dir.join("src/main.rs"));
    assert_eq!(sb.resolve("./src/main.rs").unwrap(), dir.join("src/main.rs"));
    assert_eq!(sb.resolve("new/deep/file.txt").unwrap(), dir.join("new/deep/file.txt"));
    assert_eq!(sb.resolve(".").unwrap(), dir);
    assert_eq!(sb.resolve("").unwrap(), dir);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn refuses_absolute_and_parent_paths() {
    let dir = temp_dir("lexical");
    let sb = Sandbox::new(&dir).unwrap();
    for rel in ["/etc/passwd", "/", "..", "../x", "a/../b", "a/b/../../.."] {
        assert!(refused(&sb, rel), "{rel}");
    }
    // Even an absolute path that points inside the sandbox is refused.
    assert!(refused(&sb, &dir.join("x").to_string_lossy()));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn refuses_symlinks_leading_outside() {
    let dir = temp_dir("escape");
    let outside = temp_dir("escape-target");
    fs::write(outside.join("secret"), "s").unwrap();
    symlink(outside.join("secret"), dir.join("file_link")).unwrap();
    symlink(&outside, dir.join("dir_link")).unwrap();
    let sb = Sandbox::new(&dir).unwrap();

    assert!(refused(&sb, "file_link"));
    assert!(refused(&sb, "dir_link"));
    assert!(refused(&sb, "dir_link/secret"));
    // A file that doesn't exist yet, under a link that leads out (a write target).
    assert!(refused(&sb, "dir_link/new/file.txt"));

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn refuses_broken_symlinks() {
    let dir = temp_dir("broken");
    symlink("/definitely/not/here", dir.join("dangling")).unwrap();
    let sb = Sandbox::new(&dir).unwrap();
    assert!(sb.resolve("dangling").is_err());
    assert!(sb.resolve("dangling/x").is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn allows_symlinks_staying_inside() {
    let dir = temp_dir("internal");
    fs::create_dir_all(dir.join("real")).unwrap();
    fs::write(dir.join("real/f"), "").unwrap();
    symlink(dir.join("real"), dir.join("alias")).unwrap();
    symlink("real/f", dir.join("rel_link")).unwrap();
    let sb = Sandbox::new(&dir).unwrap();

    assert_eq!(sb.resolve("alias/f").unwrap(), dir.join("real/f"));
    assert_eq!(sb.resolve("rel_link").unwrap(), dir.join("real/f"));
    let _ = fs::remove_dir_all(&dir);
}
