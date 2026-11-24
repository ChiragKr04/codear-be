use super::*;
use crate::provider::{FunctionCall, Role, ToolCall, ToolKind};

use std::cell::RefCell;
use std::collections::VecDeque;

/// Replays canned replies in order and records every conversation it was sent.
struct ScriptedModel {
    replies: RefCell<VecDeque<Result<Reply, ProviderError>>>,
    seen: RefCell<Vec<Vec<ChatMessage>>>,
}

impl ScriptedModel {
    fn new(replies: Vec<Result<Reply, ProviderError>>) -> Self {
        Self { replies: RefCell::new(replies.into()), seen: RefCell::new(Vec::new()) }
    }

    fn calls(&self) -> usize {
        self.seen.borrow().len()
    }

    /// The conversation sent on call `i` (0-based).
    fn sent(&self, i: usize) -> Vec<ChatMessage> {
        self.seen.borrow()[i].clone()
    }
}

impl Model for ScriptedModel {
    fn send(&self, messages: &[ChatMessage], tools: &[ToolDefinition]) -> Result<Reply, ProviderError> {
        assert_eq!(tools.len(), tools::TOOL_NAMES.len(), "every tool is offered");
        self.seen.borrow_mut().push(messages.to_vec());
        self.replies.borrow_mut().pop_front().expect("model called more times than scripted")
    }
}

fn text(s: &str) -> Result<Reply, ProviderError> {
    Ok(Reply {
        content: Some(s.into()),
        refusal: None,
        tool_calls: vec![],
        finish_reason: Some("stop".into()),
        usage: None,
    })
}

fn calls(list: &[(&str, &str, &str)]) -> Result<Reply, ProviderError> {
    Ok(Reply {
        content: None,
        refusal: None,
        tool_calls: list
            .iter()
            .map(|(id, name, args)| ToolCall {
                id: id.to_string(),
                kind: ToolKind::Function,
                function: FunctionCall { name: name.to_string(), arguments: args.to_string() },
            })
            .collect(),
        finish_reason: Some("tool_calls".into()),
        usage: None,
    })
}

/// Sandbox on the cwd (the crate root under `cargo test`).
fn sb() -> Sandbox {
    Sandbox::new(&std::env::current_dir().unwrap()).unwrap()
}

fn yes(_: &str) -> Result<(), String> {
    Ok(())
}

fn no(_: &str) -> Result<(), String> {
    Err("declined by user".into())
}

fn agent(model: &ScriptedModel, approver: &dyn Approver) -> Result<String, AgentError> {
    run_agent(model, &sb(), approver, "do the task", DEFAULT_MAX_TURNS)
}

/// The `tool` messages at the end of a conversation, as (call id, content).
fn tool_results(msgs: &[ChatMessage]) -> Vec<(String, String)> {
    msgs.iter()
        .filter(|m| m.role == Role::Tool)
        .map(|m| (m.tool_call_id.clone().unwrap(), m.content.clone().unwrap()))
        .collect()
}

#[test]
fn plain_answer_ends_the_loop_at_once() {
    let model = ScriptedModel::new(vec![text("nothing to do")]);
    assert_eq!(agent(&model, &yes).unwrap(), "nothing to do");
    assert_eq!(model.calls(), 1);

    let first = model.sent(0);
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].role, Role::System);
    assert_eq!(first[0].content.as_deref(), Some(INSTRUCTIONS));
    assert_eq!(first[1], ChatMessage::user("do the task"));
}

#[test]
fn tool_call_is_run_and_its_result_sent_back() {
    let model = ScriptedModel::new(vec![
        calls(&[("call_1", "read_file", r#"{"path": "Cargo.toml", "end_line": 1}"#)]),
        text("done"),
    ]);
    assert_eq!(agent(&model, &yes).unwrap(), "done");
    assert_eq!(model.calls(), 2);

    let second = model.sent(1);
    assert_eq!(second.len(), 4, "system, user, assistant tool call, tool result");
    let assistant = &second[2];
    assert_eq!(assistant.role, Role::Assistant);
    assert_eq!(assistant.tool_calls[0].id, "call_1");

    let results = tool_results(&second);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "call_1");
    assert!(results[0].1.contains("[package]"), "{}", results[0].1);
}

#[test]
fn every_call_in_a_turn_gets_one_result_in_order() {
    let model = ScriptedModel::new(vec![
        calls(&[
            ("a", "list_dir", r#"{"path": "crates", "depth": 1}"#),
            ("b", "search", r#"{"pattern": "fn run_agent", "glob": "agent.rs"}"#),
            ("c", "read_file", r#"{"path": "Cargo.toml", "end_line": 1}"#),
        ]),
        text("done"),
    ]);
    agent(&model, &yes).unwrap();

    let ids: Vec<String> = tool_results(&model.sent(1)).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, ["a", "b", "c"]);
}

#[test]
fn tool_errors_and_declined_commands_are_fed_back_not_fatal() {
    let model = ScriptedModel::new(vec![
        calls(&[
            ("bad_tool", "delete_everything", "{}"),
            ("bad_json", "read_file", "{oops"),
            ("declined", "run_command", r#"{"command": "echo hi"}"#),
        ]),
        text("gave up politely"),
    ]);
    assert_eq!(agent(&model, &no).unwrap(), "gave up politely");

    let results = tool_results(&model.sent(1));
    assert_eq!(results.len(), 3);
    for (id, content) in &results {
        assert!(content.starts_with("ERROR:"), "{id}: {content}");
    }
}

#[test]
fn history_keeps_growing_across_turns() {
    let model = ScriptedModel::new(vec![
        calls(&[("t1", "list_dir", r#"{"path": ".", "depth": 1}"#)]),
        calls(&[("t2", "read_file", r#"{"path": "Cargo.toml", "end_line": 1}"#)]),
        text("done"),
    ]);
    agent(&model, &yes).unwrap();

    let last = model.sent(2);
    let ids: Vec<String> = tool_results(&last).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, ["t1", "t2"]);
    assert_eq!(last.len(), 6);
}

#[test]
fn stops_after_max_turns() {
    let looping = || calls(&[("x", "list_dir", r#"{"path": ".", "depth": 1}"#)]);
    let model = ScriptedModel::new(vec![looping(), looping(), looping()]);
    let err = run_agent(&model, &sb(), &yes, "task", 3).unwrap_err();
    assert!(matches!(err, AgentError::MaxTurns(3)), "{err:?}");
    assert_eq!(model.calls(), 3);
}

#[test]
fn provider_error_stops_the_loop() {
    let model = ScriptedModel::new(vec![Err(ProviderError::Api { status: 500, message: "boom".into() })]);
    let err = agent(&model, &yes).unwrap_err();
    assert!(matches!(err, AgentError::Provider(ProviderError::Api { status: 500, .. })), "{err:?}");
}

#[test]
fn refusal_is_returned_as_the_answer() {
    let refusal = Ok(Reply {
        content: None,
        refusal: Some("I can't help with that.".into()),
        tool_calls: vec![],
        finish_reason: Some("stop".into()),
        usage: None,
    });
    let model = ScriptedModel::new(vec![refusal]);
    assert_eq!(agent(&model, &yes).unwrap(), "I can't help with that.");
}

#[test]
fn empty_reply_is_an_error() {
    let empty = Ok(Reply { content: None, refusal: None, tool_calls: vec![], finish_reason: None, usage: None });
    let model = ScriptedModel::new(vec![empty]);
    assert!(matches!(agent(&model, &yes), Err(AgentError::EmptyReply)));
}
