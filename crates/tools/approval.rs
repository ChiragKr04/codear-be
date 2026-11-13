//! Asking the user before a shell command runs.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};

/// Decides whether a shell command may run. `Err` carries the reason it may not,
/// which is passed back to the model.
pub trait Approver {
    fn approve(&self, command: &str) -> Result<(), String>;
}

/// Any `Fn(&str) -> Result<(), String>` is an approver (handy in tests).
impl<F: Fn(&str) -> Result<(), String>> Approver for F {
    fn approve(&self, command: &str) -> Result<(), String> {
        self(command)
    }
}

/// Shows the command on the terminal and asks `[y/N]`. Only `y` / `yes` approve.
///
/// Talks to `/dev/tty`, not stdin/stdout, so piped input can't answer for the
/// user and the prompt isn't mixed into tool output. Without a terminal
/// (e.g. under CI) every command is refused.
pub struct TerminalApprover;

impl Approver for TerminalApprover {
    fn approve(&self, command: &str) -> Result<(), String> {
        let mut tty = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .map_err(|e| format!("no terminal to ask the user for approval ({e})"))?;
        let ask = |tty: &mut std::fs::File| -> std::io::Result<String> {
            write!(tty, "\nThe agent wants to run:\n\n{}\n\nRun it? [y/N] ", display_command(command))?;
            tty.flush()?;
            let mut answer = String::new();
            BufReader::new(tty).read_line(&mut answer)?;
            Ok(answer)
        };
        let answer = ask(&mut tty).map_err(|e| format!("could not ask for approval ({e})"))?;
        match answer.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => Ok(()),
            _ => Err("the user denied it".into()),
        }
    }
}

/// `command` as shown in the prompt: each line indented, and control
/// characters escaped, so a `\r` or an ANSI escape code can't hide part of
/// the command from the user (e.g. `cat .env\recho hi` showing as `echo hi`).
pub fn display_command(command: &str) -> String {
    command
        .split('\n')
        .map(|line| {
            let safe: String = line
                .chars()
                .map(|c| if c.is_control() && c != '\t' { c.escape_default().to_string() } else { c.to_string() })
                .collect();
            format!("    {safe}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
