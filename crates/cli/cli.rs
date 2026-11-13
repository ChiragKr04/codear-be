use std::path::Path;

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
            let (dir, prompt) = parse_agent_args(&args[1..])?;
            run_agent(dir, &prompt)
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

/// Parse `<dir> <prompt...>` for the `agent` command.
fn parse_agent_args(args: &[String]) -> Result<(&Path, String), String> {
    let [dir, rest @ ..] = args else {
        return Err(format!("agent: missing <dir>\n\n{}", usage()));
    };
    let prompt = rest.join(" ");
    if prompt.trim().is_empty() {
        return Err(format!("agent: missing <prompt>\n\n{}", usage()));
    }
    let dir = Path::new(dir);
    if !dir.is_dir() {
        return Err(format!("agent: not a directory: {}", dir.display()));
    }
    Ok((dir, prompt))
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

fn run_agent(_dir: &Path, prompt: &str) -> Result<(), String> {
    let provider = OpenAIProvider::from_env().map_err(|e| e.to_string())?;
    let reply = provider.complete(prompt).map_err(|e| e.to_string())?;
    println!("{reply}");
    Ok(())
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn usage() -> String {
    "usage: agentcli <command> [args]\n\ncommands:\n  echo <text>            print the text with its first letter capitalized\n  agent <dir> <prompt>   send the prompt to OpenAI and print the reply\n                         (needs OPENAI_API_KEY; optional OPENAI_MODEL, OPENAI_BASE_URL)\n  tool [--repo <path>] <name> [json]\n                         run a tool directly with JSON args (sample args if omitted),\n                         inside <path> (default: current directory);\n                         run_command asks for approval on the terminal first\n                         tools: list_dir, read_file, search, write_file, edit_file, run_command".to_string()
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
