use super::search::*;
use super::{ToolError, ToolResult};

use std::fs;
use std::path::{Path, PathBuf};

/// Creates a fresh, empty temp directory unique to `name`.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("codear-search-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(pattern: &str, path: &Path, glob: Option<&str>) -> ToolResult {
    search(SearchArgs {
        pattern: pattern.into(),
        path: Some(path.to_string_lossy().into_owned()),
        glob: glob.map(Into::into),
    })
}

/// Output lines with the temp dir prefix removed, for readable asserts.
fn lines(out: &str, dir: &Path) -> Vec<String> {
    let prefix = format!("{}/", dir.display());
    out.lines().map(|l| l.replace(&prefix, "")).collect()
}

#[test]
fn search_finds_matches_in_name_order() {
    let dir = temp_dir("basic");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/main.rs"), "fn main() {}\nlet x = 1;\nfn helper() {}\n").unwrap();
    fs::write(dir.join("a.txt"), "no match here\nfn in text\n").unwrap();

    let out = run(r"^fn \w+", &dir, None).unwrap();
    assert_eq!(
        lines(&out, &dir),
        ["a.txt:2: fn in text", "src/main.rs:1: fn main() {}", "src/main.rs:3: fn helper() {}"]
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_respects_gitignore_without_git_repo() {
    let dir = temp_dir("gitignore");
    fs::write(dir.join(".gitignore"), "ignored/\n*.log\n").unwrap();
    fs::create_dir_all(dir.join("ignored")).unwrap();
    fs::create_dir_all(dir.join("target")).unwrap();
    fs::write(dir.join("ignored/x.rs"), "needle\n").unwrap();
    fs::write(dir.join("debug.log"), "needle\n").unwrap();
    fs::write(dir.join("target/out.rs"), "needle\n").unwrap();
    fs::write(dir.join("kept.rs"), "needle\n").unwrap();

    let out = run("needle", &dir, None).unwrap();
    assert_eq!(lines(&out, &dir), ["kept.rs:1: needle"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_filters_by_glob_at_any_depth() {
    let dir = temp_dir("glob");
    fs::create_dir_all(dir.join("deep/er")).unwrap();
    fs::write(dir.join("deep/er/a.rs"), "needle\n").unwrap();
    fs::write(dir.join("deep/er/a.md"), "needle\n").unwrap();

    let out = run("needle", &dir, Some("*.rs")).unwrap();
    assert_eq!(lines(&out, &dir), ["deep/er/a.rs:1: needle"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_skips_binary_files() {
    let dir = temp_dir("binary");
    fs::write(dir.join("bin.dat"), b"needle\0\x01\x02").unwrap();
    fs::write(dir.join("text.txt"), "needle\n").unwrap();

    let out = run("needle", &dir, None).unwrap();
    assert_eq!(lines(&out, &dir), ["text.txt:1: needle"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_accepts_a_single_file_path() {
    let dir = temp_dir("file");
    let file = dir.join("one.rs");
    fs::write(&file, "a\nneedle\n").unwrap();

    let out = run("needle", &file, None).unwrap();
    assert_eq!(lines(&out, &dir), ["one.rs:2: needle"]);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_caps_matches_with_note() {
    let dir = temp_dir("cap");
    fs::write(dir.join("many.txt"), "hit\n".repeat(MAX_MATCHES + 50)).unwrap();

    let out = run("hit", &dir, None).unwrap();
    let got = lines(&out, &dir);
    assert_eq!(got.len(), MAX_MATCHES + 1);
    assert_eq!(got[MAX_MATCHES - 1], format!("many.txt:{MAX_MATCHES}: hit"));
    assert!(got[MAX_MATCHES].starts_with(&format!("[cut at {MAX_MATCHES} matches")));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_notes_cut_when_limit_hit_at_file_boundary() {
    let dir = temp_dir("boundary");
    fs::write(dir.join("a.txt"), "hit\n".repeat(MAX_MATCHES)).unwrap();
    fs::write(dir.join("b.txt"), "hit\n").unwrap();

    let out = run("hit", &dir, None).unwrap();
    assert_eq!(out.lines().count(), MAX_MATCHES + 1);
    assert!(out.lines().last().unwrap().starts_with("[cut at"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_exactly_max_matches_has_no_note() {
    let dir = temp_dir("exact");
    fs::write(dir.join("many.txt"), "hit\n".repeat(MAX_MATCHES)).unwrap();

    let out = run("hit", &dir, None).unwrap();
    assert_eq!(out.lines().count(), MAX_MATCHES);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_clips_long_lines() {
    let dir = temp_dir("long");
    fs::write(dir.join("min.js"), format!("needle{}\n", "é".repeat(MAX_LINE_CHARS * 2))).unwrap();

    let out = run("needle", &dir, None).unwrap();
    let text = lines(&out, &dir)[0].strip_prefix("min.js:1: ").unwrap().to_string();
    assert_eq!(text.chars().count(), MAX_LINE_CHARS + 1); // + the `…` marker
    assert!(text.ends_with('…'));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_reports_no_matches() {
    let dir = temp_dir("none");
    fs::write(dir.join("a.txt"), "nothing\n").unwrap();
    assert_eq!(run("needle", &dir, None).unwrap(), "[no matches]");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn search_rejects_bad_regex_and_missing_path() {
    let dir = temp_dir("errors");
    assert!(matches!(run("(", &dir, None), Err(ToolError::InvalidArgs(_))));
    assert!(matches!(
        run("x", Path::new("/definitely/not/here"), None),
        Err(ToolError::Failed(_))
    ));
    let _ = fs::remove_dir_all(&dir);
}
