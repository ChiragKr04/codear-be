//! The agent loop: send the conversation plus the tool list, run every tool
//! call the model makes, feed the results back, and repeat until the model
//! answers without calling a tool.
//!
//! Three rules keep the conversation valid:
//! - The assistant turn that made the calls is appended before their results
//!   (`Reply::to_message`), with its text and tool calls unchanged.
//! - Every tool call gets exactly one `tool` message with the same id, even
//!   when the tool failed or the command was declined.
//! - A tool error never stops the loop: `tools::run` turns it into `ERROR: ...`
//!   text the model reads and reacts to.

use std::fmt;

use crate::provider::{ChatMessage, OpenAIProvider, ProviderError, Reply, ToolDefinition};
use crate::tools::{self, Approver, Sandbox};

/// How many model calls one task may take before the loop gives up.
pub const DEFAULT_MAX_TURNS: usize = 30;

pub const INSTRUCTIONS: &str = "\
You are a coding agent working inside a code repository. All paths are relative to the repo root.
Work like this:
1. Explore first: list_dir and search to find relevant files, then read_file before changing anything.
2. Make small, targeted changes with edit_file. Use write_file only for new files.
3. After changing code, run the tests with run_command and fix any failures, unless the user declined a test command earlier in this task.
4. If a tool returns ERROR, read the message and try a different approach. Never run a declined command again, or any other command that does the same thing, even after changing code.
5. When finished, reply with a short summary of what you changed and the test result. Do not call any tool in that final reply.";

/// Something that answers one chat turn: `OpenAIProvider` in the binary, a
/// scripted fake in tests.
pub trait Model {
    fn send(&self, messages: &[ChatMessage], tools: &[ToolDefinition]) -> Result<Reply, ProviderError>;
}

impl Model for OpenAIProvider {
    fn send(&self, messages: &[ChatMessage], tools: &[ToolDefinition]) -> Result<Reply, ProviderError> {
        OpenAIProvider::send(self, messages, tools)
    }
}

#[derive(Debug)]
pub enum AgentError {
    /// The request itself failed (network, API error, bad response).
    Provider(ProviderError),
    /// The model was still calling tools after this many turns.
    MaxTurns(usize),
    /// The model answered with no text, no refusal and no tool calls.
    EmptyReply,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(e) => write!(f, "{e}"),
            Self::MaxTurns(n) => write!(f, "stopped after {n} turns without finishing"),
            Self::EmptyReply => write!(f, "the model returned an empty reply"),
        }
    }
}

impl std::error::Error for AgentError {}

impl From<ProviderError> for AgentError {
    fn from(e: ProviderError) -> Self {
        Self::Provider(e)
    }
}

/// Every tool in `tools::specs()`, in the provider's wire format.
pub fn tool_definitions() -> Vec<ToolDefinition> {
    tools::specs()
        .into_iter()
        .map(|s| ToolDefinition::function(s.name, s.description, s.parameters))
        .collect()
}

/// Works on `task` inside `sb` until the model gives a final answer, which is
/// returned. Progress (tool calls and the first line of each result) goes to
/// stderr, so stdout carries only the answer.
pub fn run_agent(
    model: &dyn Model,
    sb: &Sandbox,
    approver: &dyn Approver,
    task: &str,
    max_turns: usize,
) -> Result<String, AgentError> {
    let tools = tool_definitions();
    let mut messages = vec![ChatMessage::system(INSTRUCTIONS), ChatMessage::user(task)];

    for turn in 1..=max_turns {
        let reply = model.send(&messages, &tools)?;

        if reply.tool_calls.is_empty() {
            return reply.content.or(reply.refusal).ok_or(AgentError::EmptyReply);
        }

        messages.push(reply.to_message());
        if let Some(text) = reply.content.as_deref().filter(|t| !t.trim().is_empty()) {
            eprintln!("{text}");
        }

        for call in &reply.tool_calls {
            let (name, args) = (&call.function.name, &call.function.arguments);
            eprintln!("[{turn}] → {name}({args})");
            let output = tools::run(sb, approver, name, args);
            eprintln!("     ← {}", output.lines().next().unwrap_or(""));
            messages.push(ChatMessage::tool_result(&call.id, output));
        }
    }

    Err(AgentError::MaxTurns(max_turns))
}

#[cfg(test)]
#[path = "tests/agent_tests.rs"]
mod tests;
