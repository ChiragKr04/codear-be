use std::path::Path;

use crate::agent;
use crate::provider::OpenAIProvider;
use crate::tools::{self, Sandbox, TerminalApprover};

/// Run the CLI with the given arguments (excluding the program name).
pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("echo") => {
            let text = args[1..].join(" ");
            println!("{}", capitalize(&text));
            Ok(())
        }
        Some("agent") => {
            let (dir, prompt, max_turns) = parse_agent_args(&args[1..])?;
            run_agent(dir, &prompt, max_turns)
        }
        Some("tool") => {
            let (repo, name, json) = parse_tool_args(&args[1..])?;
            let sb = open_sandbox(repo)?;
            let output = tools::dispatch(&sb, &TerminalApprover, name, &json).map_err(|e| format!("tool {name}: {e}"))?;
            println!("{output}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command: {other}\n\n{}", usage())),
        None => Err(usage()),
    }
}

/// Parse `[--max-turns <n>] <dir> <prompt...>` for the `agent` command.
/// `--max-turns` may appear anywhere after `agent`; without it,
/// `agent::DEFAULT_MAX_TURNS` is used.
fn parse_agent_args(args: &[String]) -> Result<(&Path, String, usize), String> {
    let mut max_turns = agent::DEFAULT_MAX_TURNS;
    let mut rest = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--max-turns" {
            let Some(n) = iter.next() else {
                return Err(format!("agent: --max-turns needs a <n>\n\n{}", usage()));
            };
            max_turns = parse_max_turns(n)?;
        } else if let Some(n) = arg.strip_prefix("--max-turns=") {
            max_turns = parse_max_turns(n)?;
        } else {
            rest.push(arg.as_str());
        }
    }

    let [dir, rest @ ..] = rest.as_slice() else {
        return Err(format!("agent: missing <dir>\n\n{}", usage()));
    };
    let prompt = rest.join(" ");
    if prompt.trim().is_empty() {
        return Err(format!("agent: missing <prompt>\n\n{}", usage()));
    }
    let dir = Path::new(*dir);
    if !dir.is_dir() {
        return Err(format!("agent: not a directory: {}", dir.display()));
    }
    Ok((dir, prompt, max_turns))
}

/// A positive turn count for `--max-turns`.
fn parse_max_turns(s: &str) -> Result<usize, String> {
    match s.parse::<usize>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("agent: --max-turns must be a positive integer, got {s:?}")),
    }
}

/// Parse `[--repo <path>] <name> [json-args]` for the `tool` command.
/// `--repo` may appear anywhere after `tool`. Without JSON args, the tool's
/// sample args are used.
fn parse_tool_args(args: &[String]) -> Result<(Option<&Path>, &str, String), String> {
    let mut repo = None;
    let mut rest = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--repo" {
            let Some(path) = iter.next() else {
                return Err(format!("tool: --repo needs a <path>\n\n{}", usage()));
            };
            repo = Some(Path::new(path.as_str()));
        } else if let Some(path) = arg.strip_prefix("--repo=") {
            repo = Some(Path::new(path));
        } else {
            rest.push(arg.as_str());
        }
    }

    let [name, json @ ..] = rest.as_slice() else {
        return Err(format!("tool: missing <name>\n\n{}", usage()));
    };
    let Some(sample) = tools::sample_args(name) else {
        return Err(format!(
            "tool: unknown tool: {name} (expected one of: {})",
            tools::TOOL_NAMES.join(", ")
        ));
    };
    let json = json.join(" ");
    let json = if json.trim().is_empty() { sample.to_string() } else { json };
    Ok((repo, name, json))
}

/// The sandbox tools run in: `repo`, or the current directory when not given.
fn open_sandbox(repo: Option<&Path>) -> Result<Sandbox, String> {
    let repo = match repo {
        Some(repo) => repo.to_path_buf(),
        None => std::env::current_dir().map_err(|e| format!("tool: no current directory: {e}"))?,
    };
    Sandbox::new(&repo).map_err(|e| format!("tool: --repo {}: {e}", repo.display()))
}


/// Runs the agent loop on `prompt` inside `dir` and prints its final answer.
fn run_agent(dir: &Path, prompt: &str, max_turns: usize) -> Result<(), String> {
    let provider = OpenAIProvider::from_env().map_err(|e| e.to_string())?;
    let sb = Sandbox::new(dir).map_err(|e| format!("agent: {}: {e}", dir.display()))?;
    let answer = agent::run_agent(&provider, &sb, &TerminalApprover, prompt, max_turns)
        .map_err(|e| format!("agent: {e}"))?;
    println!("{answer}");
    print_changes(sb.root());
    Ok(())
}

/// Prints what git says changed under `root`, so the model's own summary of
/// its edits can be checked against the files. Best effort: a missing `git`
/// or a non-repo directory prints a note instead of failing the run.
fn print_changes(root: &Path) {
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git").args(args).current_dir(root).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };

    // `git diff --stat` skips untracked files, so list new files separately.
    let (Some(stat), Some(untracked)) = (
        git(&["diff", "--stat"]),
        git(&["ls-files", "--others", "--exclude-standard"]),
    ) else {
        println!("\nFiles changed: unknown ({} is not a git repository)", root.display());
        return;
    };

    println!("\nFiles changed:");
    if stat.trim().is_empty() && untracked.trim().is_empty() {
        println!(" (none)");
        return;
    }
    print!("{stat}");
    for path in untracked.lines() {
        println!(" {path} (new)");
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn usage() -> String {
    "usage: agentcli <command> [args]\n\ncommands:\n  echo <text>            print the text with its first letter capitalized\n  agent [--max-turns <n>] <dir> <prompt>\n                         work on the task inside <dir> with the tools below, looping\n                         until OpenAI gives a final answer, then print it\n                         (gives up after <n> turns; default 30)\n                         (progress goes to stderr; run_command asks for approval)\n                         (needs OPENAI_API_KEY; optional OPENAI_MODEL, OPENAI_BASE_URL)\n  tool [--repo <path>] <name> [json]\n                         run a tool directly with JSON args (sample args if omitted),\n                         inside <path> (default: current directory);\n                         run_command asks for approval on the terminal first\n                         tools: list_dir, read_file, search, write_file, edit_file, run_command".to_string()
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
