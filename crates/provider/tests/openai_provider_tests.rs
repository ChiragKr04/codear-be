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
        ChatMessage { role: Role::Developer, content: "d".into() },
        ChatMessage { role: Role::Tool, content: "t".into() },
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
