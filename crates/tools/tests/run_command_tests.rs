use super::run_command::*;
use super::{Sandbox, ToolError, ToolResult};

use std::time::{Duration, Instant};

/// Sandbox on the cwd (the crate root under `cargo test`).
fn sb() -> Sandbox {
    Sandbox::new(&std::env::current_dir().unwrap()).unwrap()
}

fn run(command: &str) -> ToolResult {
    run_command(&sb(), RunCommandArgs { command: command.into(), timeout_secs: None })
}

#[test]
fn run_command_returns_exit_code_stdout_and_stderr() {
    let out = run("echo out; echo err >&2").unwrap();
    assert_eq!(out, "exit code: 0\nstdout:\nout\n\nstderr:\nerr\n");
}

#[test]
fn run_command_reports_nonzero_exit_as_ok() {
    let out = run("echo failing; exit 3").unwrap();
    assert!(out.starts_with("exit code: 3\nstdout:\nfailing\n"), "{out}");
}

#[test]
fn run_command_reports_signal_death() {
    let out = run("kill -TERM $$").unwrap();
    assert!(out.starts_with("exit code: killed by signal 15\n"), "{out}");
}

#[test]
fn run_command_has_no_stdin() {
    // `cat` would block forever on an inherited terminal stdin.
    let out = run("cat; echo done").unwrap();
    assert!(out.contains("stdout:\ndone\n"), "{out}");
}

#[test]
fn run_command_runs_in_sandbox_root() {
    let dir = std::env::temp_dir().join(format!("codear-run-root-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let sb = Sandbox::new(&dir).unwrap();
    let out = run_command(&sb, RunCommandArgs { command: "pwd -P".into(), timeout_secs: None })
        .unwrap();
    assert!(out.contains(&format!("stdout:\n{}\n", sb.root().display())), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_command_keeps_last_bytes_of_long_output() {
    // 40_000 'a's then a marker: only the tail survives.
    let out = run("head -c 40000 /dev/zero | tr '\\0' a; printf END").unwrap();
    let total = 40_000 + 3;
    assert!(
        out.contains(&format!("stdout (last {MAX_OUTPUT_BYTES} of {total} bytes):\n")),
        "{}",
        &out[..200]
    );
    assert!(out.contains("aEND\nstderr:\n"));
    let stdout = out.split("bytes):\n").nth(1).unwrap().split("\nstderr:").next().unwrap();
    assert_eq!(stdout.len(), MAX_OUTPUT_BYTES);
}

#[test]
fn run_command_times_out_and_kills() {
    let start = Instant::now();
    let out = run_command(&sb(), RunCommandArgs {
        command: "echo started; sleep 30".into(),
        timeout_secs: Some(1),
    })
    .unwrap();
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(out.starts_with("exit code: killed (timed out after 1s)\nstdout:\nstarted\n"), "{out}");
}

#[test]
fn run_command_does_not_wait_for_background_jobs() {
    let start = Instant::now();
    let out = run("sleep 30 & echo done").unwrap();
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(out.starts_with("exit code: 0\nstdout:\ndone\n"), "{out}");
}

#[test]
fn run_command_rejects_empty_command() {
    assert!(matches!(run("  "), Err(ToolError::InvalidArgs(_))));
}

#[test]
fn effective_timeout_defaults_and_clamps() {
    assert_eq!(effective_timeout(None), DEFAULT_TIMEOUT_SECS);
    assert_eq!(effective_timeout(Some(10)), 10);
    assert_eq!(effective_timeout(Some(10_000)), MAX_TIMEOUT_SECS);
    assert_eq!(effective_timeout(Some(0)), 1);
}

#[test]
fn tail_render_drops_char_split_by_cut() {
    // "é" is 0xC3 0xA9; a cut that keeps only 0xA9 must not show U+FFFD.
    let tail = Tail { bytes: vec![0xA9, b'o', b'k'], total: 10 };
    assert_eq!(tail.render("stdout"), "stdout (last 2 of 10 bytes):\nok");
}

#[test]
fn read_tail_keeps_last_bytes() {
    let data: Vec<u8> = (0..MAX_OUTPUT_BYTES * 3).map(|i| (i % 251) as u8).collect();
    let tail = read_tail(data.as_slice()).unwrap();
    assert_eq!(tail.total, data.len());
    assert_eq!(tail.bytes, &data[data.len() - MAX_OUTPUT_BYTES..]);
}
