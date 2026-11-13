use super::*;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn agent_args_join_prompt_words() {
    let args = s(&[".", "Hello", "there"]);
    let (dir, prompt) = parse_agent_args(&args).unwrap();
    assert_eq!(dir, Path::new("."));
    assert_eq!(prompt, "Hello there");
}

#[test]
fn agent_args_require_dir_and_prompt() {
    assert!(parse_agent_args(&s(&[])).unwrap_err().contains("missing <dir>"));
    assert!(parse_agent_args(&s(&["."])).unwrap_err().contains("missing <prompt>"));
    assert!(parse_agent_args(&s(&[".", "  "])).unwrap_err().contains("missing <prompt>"));
}

#[test]
fn agent_args_reject_missing_directory() {
    let err = parse_agent_args(&s(&["/definitely/not/here", "hi"])).unwrap_err();
    assert!(err.contains("not a directory"));
}

#[test]
fn tool_args_use_sample_when_json_omitted() {
    let args = s(&["list_dir"]);
    let (repo, name, json) = parse_tool_args(&args).unwrap();
    assert_eq!(repo, None);
    assert_eq!(name, "list_dir");
    assert_eq!(json, tools::sample_args("list_dir").unwrap());
}

#[test]
fn tool_args_join_json_words() {
    let args = s(&["read_file", r#"{"path":"#, r#""a.rs"}"#]);
    let (_, _, json) = parse_tool_args(&args).unwrap();
    assert_eq!(json, r#"{"path": "a.rs"}"#);
}

#[test]
fn tool_args_reject_missing_or_unknown_name() {
    assert!(parse_tool_args(&s(&[])).unwrap_err().contains("missing <name>"));
    assert!(parse_tool_args(&s(&["nope"])).unwrap_err().contains("unknown tool: nope"));
}

#[test]
fn every_tool_runs_with_its_sample_args() {
    // Sample args assume the crate root (e.g. `src/main.rs`), the cwd under `cargo test`.
    let sb = open_sandbox(None).unwrap();
    for name in tools::TOOL_NAMES {
        let json = tools::sample_args(name).unwrap();
        let out = tools::dispatch(&sb, name, json);
        assert!(out.is_ok(), "{name}: {out:?}");
    }
}

#[test]
fn tool_args_take_repo_anywhere() {
    let args = s(&["--repo", "/tmp/x", "list_dir"]);
    let (repo, name, json) = parse_tool_args(&args).unwrap();
    assert_eq!((repo, name), (Some(Path::new("/tmp/x")), "list_dir"));
    assert_eq!(json, tools::sample_args("list_dir").unwrap());

    let args = s(&["read_file", r#"{"path":"a"}"#, "--repo=/tmp/y"]);
    let (repo, name, json) = parse_tool_args(&args).unwrap();
    assert_eq!((repo, name), (Some(Path::new("/tmp/y")), "read_file"));
    assert_eq!(json, r#"{"path":"a"}"#);
}

#[test]
fn tool_args_reject_repo_without_path() {
    assert!(parse_tool_args(&s(&["list_dir", "--repo"])).unwrap_err().contains("--repo needs"));
}

#[test]
fn open_sandbox_rejects_missing_directory() {
    let err = open_sandbox(Some(Path::new("/definitely/not/here"))).unwrap_err();
    assert!(err.contains("--repo /definitely/not/here"), "{err}");
}

#[test]
fn tools_resolve_paths_against_repo_not_cwd() {
    // `src/main.rs` exists in the cwd under `cargo test`, but not under `crates/`.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates");
    let sb = open_sandbox(Some(&repo)).unwrap();
    assert!(tools::dispatch(&sb, "read_file", r#"{"path": "src/main.rs"}"#).is_err());
    assert!(tools::dispatch(&sb, "read_file", r#"{"path": "cli/cli.rs"}"#).is_ok());
}
