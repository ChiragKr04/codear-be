use super::edit_file::*;
use super::ToolResult;

use std::fs;
use std::path::PathBuf;

/// Fresh, empty temp directory unique to `name`.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("codear-edit-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn edit(path: &PathBuf, old_str: &str, new_str: &str) -> ToolResult {
    edit_file(EditFileArgs {
        path: path.to_string_lossy().into_owned(),
        old_str: old_str.into(),
        new_str: new_str.into(),
    })
}

#[test]
fn replaces_single_match() {
    let dir = temp_dir("single");
    let path = dir.join("f.rs");
    fs::write(&path, "fn a() {}\nfn b() {\n    old();\n}\n").unwrap();
    let out = edit(&path, "    old();\n", "    new();\n    more();\n").unwrap();
    assert_eq!(out, format!("edited {} at line 3: -1 +2 lines (now 5 lines)", path.display()));
    assert_eq!(fs::read_to_string(&path).unwrap(), "fn a() {}\nfn b() {\n    new();\n    more();\n}\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn deletes_with_empty_new_str() {
    let dir = temp_dir("delete");
    let path = dir.join("f.txt");
    fs::write(&path, "keep\ndrop\nkeep too\n").unwrap();
    edit(&path, "drop\n", "").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "keep\nkeep too\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn zero_matches_names_count_and_leaves_file() {
    let dir = temp_dir("zero");
    let path = dir.join("f.txt");
    fs::write(&path, "hello\n").unwrap();
    let err = edit(&path, "goodbye", "x").unwrap_err().to_string();
    assert!(err.contains("0 matches"), "{err}");
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn multiple_matches_names_count_and_lines() {
    let dir = temp_dir("many");
    let path = dir.join("f.txt");
    fs::write(&path, "x = 1\ny = 2\nx = 1\nx = 1\n").unwrap();
    let err = edit(&path, "x = 1", "x = 9").unwrap_err().to_string();
    assert!(err.contains("matches 3 times"), "{err}");
    assert!(err.contains("at lines 1, 3, 4"), "{err}");
    assert_eq!(fs::read_to_string(&path).unwrap(), "x = 1\ny = 2\nx = 1\nx = 1\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn many_matches_list_is_capped() {
    let dir = temp_dir("capped");
    let path = dir.join("f.txt");
    fs::write(&path, "z\n".repeat(15)).unwrap();
    let err = edit(&path, "z", "y").unwrap_err().to_string();
    assert!(err.contains("matches 15 times"), "{err}");
    assert!(err.contains("and 5 more"), "{err}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn overlapping_matches_are_ambiguous() {
    let dir = temp_dir("overlap");
    let path = dir.join("f.txt");
    fs::write(&path, "aaa").unwrap();
    let err = edit(&path, "aa", "b").unwrap_err().to_string();
    assert!(err.contains("matches 2 times"), "{err}");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_bad_args() {
    let dir = temp_dir("badargs");
    let path = dir.join("f.txt");
    fs::write(&path, "abc\n").unwrap();
    assert!(edit(&path, "", "x").unwrap_err().to_string().contains("old_str is empty"));
    assert!(edit(&path, "abc", "abc").unwrap_err().to_string().contains("identical"));
    assert!(edit(&dir, "abc", "x").unwrap_err().to_string().contains("is a directory"));
    assert!(edit(&dir.join("missing.txt"), "abc", "x").is_err());
    assert!(edit(&PathBuf::from("  "), "abc", "x").is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_non_utf8_without_touching_it() {
    let dir = temp_dir("binary");
    let path = dir.join("f.bin");
    fs::write(&path, b"ab\xffcd").unwrap();
    assert!(edit(&path, "ab", "x").unwrap_err().to_string().contains("not valid UTF-8"));
    assert_eq!(fs::read(&path).unwrap(), b"ab\xffcd");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn find_all_handles_multibyte_chars() {
    assert_eq!(find_all("éé·éé", "éé"), vec![0, 6]);
    assert_eq!(find_all("ééé", "éé"), vec![0, 2]);
    assert_eq!(find_all("abc", "x"), Vec::<usize>::new());
}
