//! `run_command`: run a shell command in the repo root.

use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde::Deserialize;

use super::{Sandbox, ToolError, ToolResult};

pub const DEFAULT_TIMEOUT_SECS: u64 = 60;
pub const MAX_TIMEOUT_SECS: u64 = 300;
/// Only the last this-many bytes of stdout/stderr are kept.
pub const MAX_OUTPUT_BYTES: usize = 30 * 1024;

const SIGKILL: i32 = 9;

unsafe extern "C" {
    /// libc `kill(2)`; a negative `pid` signals the whole process group.
    fn kill(pid: i32, sig: i32) -> i32;
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunCommandArgs {
    pub command: String,
    /// Defaults to `DEFAULT_TIMEOUT_SECS`, capped at `MAX_TIMEOUT_SECS`.
    pub timeout_secs: Option<u64>,
}

/// Runs `sh -c <command>` in the sandbox root with no stdin. Returns the exit code,
/// stdout and stderr, keeping the last `MAX_OUTPUT_BYTES` of each.
///
/// The command runs in its own process group. On timeout the whole group is
/// killed; once `sh` exits, anything it left running in the background is
/// killed too, so leftovers can neither hang the call (by holding the output
/// pipes open) nor outlive it. A non-zero exit is not an error: the model gets
/// the exit code and output either way.
///
/// Output looks like:
///
/// ```text
/// exit code: 0
/// stdout:
/// ...
/// stderr:
/// ...
/// ```
///
/// with `exit code: killed (timed out after Ns)` or `exit code: killed by signal N`
/// instead when there is no code, and `stdout (last B of T bytes):` when cut.
pub fn run_command(sb: &Sandbox, args: RunCommandArgs) -> ToolResult {
    if args.command.trim().is_empty() {
        return Err(ToolError::InvalidArgs("command is empty".into()));
    }
    let timeout = effective_timeout(args.timeout_secs);

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&args.command)
        .current_dir(sb.root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let pgid = child.id() as i32;

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    let stdout = thread::spawn(move || read_tail(stdout));
    let stderr = thread::spawn(move || read_tail(stderr));

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(child.wait());
    });

    let (status, timed_out) = match rx.recv_timeout(Duration::from_secs(timeout)) {
        Ok(status) => (status, false),
        Err(_) => {
            kill_group(pgid);
            (rx.recv().expect("waiter thread sends a status"), true)
        }
    };
    // Kill background leftovers so they release the pipes and the readers finish.
    kill_group(pgid);
    let status = status?;

    let stdout = stdout.join().expect("stdout reader panicked")?;
    let stderr = stderr.join().expect("stderr reader panicked")?;

    Ok(format!(
        "exit code: {}\n{}\n{}",
        describe_exit(status, timed_out, timeout),
        stdout.render("stdout"),
        stderr.render("stderr"),
    ))
}

/// The timeout actually used, in seconds: default when unset, within `1..=MAX_TIMEOUT_SECS`.
pub fn effective_timeout(requested: Option<u64>) -> u64 {
    requested.unwrap_or(DEFAULT_TIMEOUT_SECS).clamp(1, MAX_TIMEOUT_SECS)
}

fn kill_group(pgid: i32) {
    // SAFETY: plain syscall; fails harmlessly (ESRCH) if the group is already gone.
    unsafe {
        kill(-pgid, SIGKILL);
    }
}

fn describe_exit(status: ExitStatus, timed_out: bool, timeout: u64) -> String {
    if timed_out {
        return format!("killed (timed out after {timeout}s)");
    }
    match (status.code(), status.signal()) {
        (Some(code), _) => code.to_string(),
        (None, Some(sig)) => format!("killed by signal {sig}"),
        (None, None) => "unknown".into(),
    }
}

/// The end of a stream: its last `MAX_OUTPUT_BYTES` and how many bytes it had in total.
pub struct Tail {
    pub bytes: Vec<u8>,
    pub total: usize,
}

impl Tail {
    /// `<name>:\n<text>`, or `<name> (last B of T bytes):\n<text>` when cut.
    /// Text is lossy UTF-8, without a char split by the cut.
    pub fn render(&self, name: &str) -> String {
        let mut bytes = self.bytes.as_slice();
        let cut = self.total > bytes.len();
        if cut {
            // Drop UTF-8 continuation bytes orphaned by the cut (at most 3).
            let skip = bytes.iter().take(3).take_while(|b| (**b & 0xC0) == 0x80).count();
            bytes = &bytes[skip..];
        }
        let text = String::from_utf8_lossy(bytes);
        if cut {
            format!("{name} (last {} of {} bytes):\n{text}", bytes.len(), self.total)
        } else {
            format!("{name}:\n{text}")
        }
    }
}

/// Reads `r` to the end, keeping only the last `MAX_OUTPUT_BYTES`.
pub fn read_tail(mut r: impl Read) -> std::io::Result<Tail> {
    let mut bytes = Vec::new();
    let mut total = 0;
    let mut chunk = [0u8; 8192];
    loop {
        let n = match r.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        total += n;
        bytes.extend_from_slice(&chunk[..n]);
        // Trim in batches so we don't shift the buffer on every read.
        if bytes.len() > 2 * MAX_OUTPUT_BYTES {
            bytes.drain(..bytes.len() - MAX_OUTPUT_BYTES);
        }
    }
    if bytes.len() > MAX_OUTPUT_BYTES {
        bytes.drain(..bytes.len() - MAX_OUTPUT_BYTES);
    }
    Ok(Tail { bytes, total })
}
