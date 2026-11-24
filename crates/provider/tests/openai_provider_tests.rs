use super::*;
use serde_json::json;
use std::time::Duration;

const SUCCESS_BODY: &str = r#"{
    "id": "chatcmpl-abc123",
    "object": "chat.completion",
    "created": 1741569952,
    "model": "gpt-4o-mini-2024-07-18",
    "choices": [{
        "index": 0,
        "message": {
            "role": "assistant",
            "content": "Hello! How can I help you today?",
            "refusal": null,
            "annotations": []
        },
        "logprobs": null,
        "finish_reason": "stop"
    }],
    "usage": {
        "prompt_tokens": 9,
        "completion_tokens": 10,
        "total_tokens": 19,
        "prompt_tokens_details": { "cached_tokens": 0 }
    },
    "service_tier": "default",
    "system_fingerprint": "fp_123"
}"#;

// ----- request serialization -----

#[test]
fn request_serializes_minimal_fields() {
    let req = ChatCompletionRequest::new("gpt-4o-mini", vec![ChatMessage::user("Hello")]);
    let value = serde_json::to_value(&req).unwrap();
    assert_eq!(
        value,
        json!({
            "model": "gpt-4o-mini",
            "messages": [{ "role": "user", "content": "Hello" }],
            "stream": false
        })
    );
}

#[test]
fn request_omits_none_and_includes_some_optionals() {
    let mut req = ChatCompletionRequest::new("gpt-4o-mini", vec![ChatMessage::user("hi")]);
    req.max_completion_tokens = Some(64);
    let value = serde_json::to_value(&req).unwrap();
    assert_eq!(value["max_completion_tokens"], 64);
    assert!(value.get("temperature").is_none());

    req.temperature = Some(0.5);
    let value = serde_json::to_value(&req).unwrap();
    assert_eq!(value["temperature"], 0.5);
}

#[test]
fn request_is_never_streaming() {
    let req = ChatCompletionRequest::new("m", vec![]);
    assert!(!req.stream);
    assert_eq!(serde_json::to_value(&req).unwrap()["stream"], false);
}

#[test]
fn roles_serialize_lowercase() {
    let msgs = vec![
        ChatMessage::system("s"),
        ChatMessage::user("u"),
        ChatMessage::assistant("a"),
        ChatMessage::new(Role::Developer, "d"),
        ChatMessage::tool_result("call_1", "t"),
    ];
    let roles: Vec<String> = msgs
        .iter()
        .map(|m| serde_json::to_value(m).unwrap()["role"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(roles, ["system", "user", "assistant", "developer", "tool"]);
}

#[test]
fn request_round_trips() {
    let mut req = ChatCompletionRequest::new(
        "gpt-4o-mini",
        vec![ChatMessage::system("be brief"), ChatMessage::user("Hello")],
    );
    req.temperature = Some(0.25);
    let json = serde_json::to_string(&req).unwrap();
    let back: ChatCompletionRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(back, req);
}

// ----- response deserialization -----

#[test]
fn response_deserializes_full_body_ignoring_unknown_fields() {
    let resp: ChatCompletionResponse = serde_json::from_str(SUCCESS_BODY).unwrap();
    assert_eq!(resp.id, "chatcmpl-abc123");
    assert_eq!(resp.object, "chat.completion");
    assert_eq!(resp.created, 1741569952);
    assert_eq!(resp.model, "gpt-4o-mini-2024-07-18");
    assert_eq!(resp.choices.len(), 1);

    let choice = &resp.choices[0];
    assert_eq!(choice.index, 0);
    assert_eq!(choice.finish_reason.as_deref(), Some("stop"));
    assert_eq!(choice.message.role, Role::Assistant);
    assert_eq!(choice.message.refusal, None);

    assert_eq!(
        resp.usage,
        Some(Usage { prompt_tokens: 9, completion_tokens: 10, total_tokens: 19 })
    );
    assert_eq!(resp.first_content(), Some("Hello! How can I help you today?"));
}

#[test]
fn response_handles_null_content_and_refusal() {
    let body = json!({
        "id": "x", "object": "chat.completion", "created": 1, "model": "m",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": null, "refusal": "I can't help with that." },
            "finish_reason": "stop"
        }]
    });
    let resp: ChatCompletionResponse = serde_json::from_value(body).unwrap();
    assert_eq!(resp.first_content(), None);
    assert_eq!(resp.choices[0].message.refusal.as_deref(), Some("I can't help with that."));
    assert_eq!(resp.usage, None);
}

#[test]
fn response_with_no_choices_has_no_content() {
    let body = r#"{"id":"x","object":"chat.completion","created":1,"model":"m","choices":[]}"#;
    let resp: ChatCompletionResponse = serde_json::from_str(body).unwrap();
    assert_eq!(resp.first_content(), None);
}

#[test]
fn error_body_deserializes() {
    let body = r#"{
        "error": {
            "message": "Incorrect API key provided: sk-xxx.",
            "type": "invalid_request_error",
            "param": null,
            "code": "invalid_api_key"
        }
    }"#;
    let err: ApiErrorResponse = serde_json::from_str(body).unwrap();
    assert_eq!(err.error.message, "Incorrect API key provided: sk-xxx.");
    assert_eq!(err.error.kind.as_deref(), Some("invalid_request_error"));
    assert_eq!(err.error.param, None);
    assert_eq!(err.error.code.as_deref(), Some("invalid_api_key"));
}

// ----- tool calling: what goes in -----

fn read_file_tool() -> ToolDefinition {
    ToolDefinition::function(
        "read_file",
        "Read a file",
        json!({
            "type": "object",
            "properties": { "path": { "type": "string" } },
            "required": ["path"],
            "additionalProperties": false
        }),
    )
}

fn call(id: &str, name: &str, arguments: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        kind: ToolKind::Function,
        function: FunctionCall { name: name.into(), arguments: arguments.into() },
    }
}

#[test]
fn request_without_tools_omits_tools_field() {
    let req = ChatCompletionRequest::new("m", vec![ChatMessage::user("hi")]);
    assert!(serde_json::to_value(&req).unwrap().get("tools").is_none());
}

#[test]
fn request_serializes_tools_in_openai_format() {
    let mut req = ChatCompletionRequest::new("m", vec![ChatMessage::user("hi")]);
    req.tools = vec![read_file_tool()];
    let value = serde_json::to_value(&req).unwrap();
    assert_eq!(
        value["tools"],
        json!([{
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a file",
                "parameters": {
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"],
                    "additionalProperties": false
                }
            }
        }])
    );
}

#[test]
fn plain_messages_omit_tool_fields() {
    let value = serde_json::to_value(ChatMessage::user("hi")).unwrap();
    assert_eq!(value, json!({ "role": "user", "content": "hi" }));
}

#[test]
fn assistant_tool_call_message_serializes_with_null_content() {
    let msg = ChatMessage::assistant_tool_calls(None, vec![call("call_1", "read_file", r#"{"path":"a.rs"}"#)]);
    assert_eq!(
        serde_json::to_value(&msg).unwrap(),
        json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": { "name": "read_file", "arguments": "{\"path\":\"a.rs\"}" }
            }]
        })
    );
}

#[test]
fn tool_result_message_serializes_with_call_id() {
    let msg = ChatMessage::tool_result("call_1", "fn main() {}");
    assert_eq!(
        serde_json::to_value(&msg).unwrap(),
        json!({ "role": "tool", "content": "fn main() {}", "tool_call_id": "call_1" })
    );
}

#[test]
fn request_with_tool_history_round_trips() {
    let mut req = ChatCompletionRequest::new(
        "m",
        vec![
            ChatMessage::user("read a.rs"),
            ChatMessage::assistant_tool_calls(Some("Reading.".into()), vec![call("c1", "read_file", "{}")]),
            ChatMessage::tool_result("c1", "contents"),
        ],
    );
    req.tools = vec![read_file_tool()];
    let back: ChatCompletionRequest = serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
    assert_eq!(back, req);
}

// ----- tool calling: what comes out -----

const TOOL_CALL_BODY: &str = r#"{
    "id": "chatcmpl-tool",
    "object": "chat.completion",
    "created": 1741569952,
    "model": "gpt-4o-mini",
    "choices": [{
        "index": 0,
        "message": {
            "role": "assistant",
            "content": null,
            "refusal": null,
            "tool_calls": [
                {
                    "id": "call_abc",
                    "type": "function",
                    "function": { "name": "read_file", "arguments": "{\"path\":\"src/main.rs\"}" }
                },
                {
                    "id": "call_def",
                    "type": "function",
                    "function": { "name": "list_dir", "arguments": "{\"path\":\".\"}" }
                }
            ]
        },
        "finish_reason": "tool_calls"
    }],
    "usage": { "prompt_tokens": 50, "completion_tokens": 20, "total_tokens": 70 }
}"#;

#[test]
fn response_with_tool_calls_becomes_reply() {
    let reply = parse_response(200, TOOL_CALL_BODY).unwrap().into_reply().unwrap();
    assert_eq!(reply.content, None);
    assert_eq!(reply.finish_reason.as_deref(), Some("tool_calls"));
    assert_eq!(
        reply.tool_calls,
        vec![
            call("call_abc", "read_file", r#"{"path":"src/main.rs"}"#),
            call("call_def", "list_dir", r#"{"path":"."}"#),
        ]
    );
    assert_eq!(reply.usage.map(|u| u.total_tokens), Some(70));
}

#[test]
fn reply_to_message_echoes_the_tool_calls() {
    let reply = parse_response(200, TOOL_CALL_BODY).unwrap().into_reply().unwrap();
    let msg = reply.to_message();
    assert_eq!(msg.role, Role::Assistant);
    assert_eq!(msg.content, None);
    assert_eq!(msg.tool_calls, reply.tool_calls);
    assert_eq!(msg.tool_call_id, None);
}

#[test]
fn text_response_becomes_reply_without_tool_calls() {
    let reply = parse_response(200, SUCCESS_BODY).unwrap().into_reply().unwrap();
    assert_eq!(reply.content.as_deref(), Some("Hello! How can I help you today?"));
    assert!(reply.tool_calls.is_empty());
    assert_eq!(reply.finish_reason.as_deref(), Some("stop"));
}

#[test]
fn null_tool_calls_and_missing_type_are_accepted() {
    // Some OpenAI-compatible servers send `tool_calls: null`, or omit a call's `type`.
    let null_calls = json!({
        "id": "x", "object": "chat.completion", "created": 1, "model": "m",
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": "hi", "tool_calls": null } }]
    });
    let reply = serde_json::from_value::<ChatCompletionResponse>(null_calls).unwrap().into_reply().unwrap();
    assert!(reply.tool_calls.is_empty());

    let no_type = json!({ "id": "c", "function": { "name": "search", "arguments": "{}" } });
    let tc: ToolCall = serde_json::from_value(no_type).unwrap();
    assert_eq!(tc.kind, ToolKind::Function);
}

#[test]
fn reply_from_no_choices_is_empty_response() {
    let body = r#"{"id":"x","object":"chat.completion","created":1,"model":"m","choices":[]}"#;
    let resp: ChatCompletionResponse = serde_json::from_str(body).unwrap();
    assert!(matches!(resp.into_reply(), Err(ProviderError::EmptyResponse)));
}

// ----- status/body -> result mapping -----

#[test]
fn parse_response_ok() {
    let resp = parse_response(200, SUCCESS_BODY).unwrap();
    assert_eq!(resp.first_content(), Some("Hello! How can I help you today?"));
}

#[test]
fn parse_response_api_error_uses_error_message() {
    let body = r#"{"error":{"message":"Rate limit reached","type":"requests","param":null,"code":"rate_limit_exceeded"}}"#;
    match parse_response(429, body) {
        Err(ProviderError::Api { status, message }) => {
            assert_eq!(status, 429);
            assert_eq!(message, "Rate limit reached");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[test]
fn parse_response_non_json_error_keeps_raw_body() {
    match parse_response(502, "Bad Gateway") {
        Err(ProviderError::Api { status, message }) => {
            assert_eq!(status, 502);
            assert_eq!(message, "Bad Gateway");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[test]
fn parse_response_malformed_success_is_decode_error() {
    assert!(matches!(parse_response(200, "{not json"), Err(ProviderError::Decode(_))));
}

// ----- config from env -----

fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: std::collections::HashMap<String, String> =
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    move |name| map.get(name).cloned()
}

const FULL_ENV: &[(&str, &str)] = &[
    (ENV_API_KEY, "sk-test"),
    (ENV_BASE_URL, "http://localhost:8080/v1/"),
    (ENV_MODEL, "gpt-test"),
    (ENV_TIMEOUT_SECS, "30"),
];

fn env_without(skip: &str) -> Vec<(&'static str, &'static str)> {
    FULL_ENV.iter().copied().filter(|(k, _)| *k != skip).collect()
}

fn env_with(name: &'static str, value: &'static str) -> Vec<(&'static str, &'static str)> {
    let mut env = env_without(name);
    env.push((name, value));
    env
}

#[test]
fn config_reads_all_values_and_trims_base_url() {
    let cfg = ProviderConfig::from_lookup(lookup(FULL_ENV)).unwrap();
    assert_eq!(
        cfg,
        ProviderConfig {
            api_key: "sk-test".into(),
            base_url: "http://localhost:8080/v1".into(),
            model: "gpt-test".into(),
            timeout: Duration::from_secs(30),
        }
    );
}

#[test]
fn config_reports_each_missing_variable() {
    for name in [ENV_API_KEY, ENV_BASE_URL, ENV_MODEL, ENV_TIMEOUT_SECS] {
        match ProviderConfig::from_lookup(lookup(&env_without(name))) {
            Err(ProviderError::MissingEnv(missing)) => assert_eq!(missing, name),
            other => panic!("expected MissingEnv({name}), got {other:?}"),
        }
    }
}

#[test]
fn config_treats_blank_value_as_missing() {
    let result = ProviderConfig::from_lookup(lookup(&env_with(ENV_API_KEY, "   ")));
    assert!(matches!(result, Err(ProviderError::MissingEnv(ENV_API_KEY))));
}

#[test]
fn config_rejects_invalid_timeout() {
    for bad in ["abc", "0", "-5", "1.5"] {
        match ProviderConfig::from_lookup(lookup(&env_with(ENV_TIMEOUT_SECS, bad))) {
            Err(ProviderError::InvalidEnv { name, value }) => {
                assert_eq!(name, ENV_TIMEOUT_SECS);
                assert_eq!(value, bad);
            }
            other => panic!("expected InvalidEnv for {bad:?}, got {other:?}"),
        }
    }
}

#[test]
fn provider_is_built_from_config() {
    let cfg = ProviderConfig::from_lookup(lookup(FULL_ENV)).unwrap();
    let p = OpenAIProvider::new(cfg).unwrap();
    assert_eq!(p.base_url, "http://localhost:8080/v1");
    assert_eq!(p.model(), "gpt-test");
}
