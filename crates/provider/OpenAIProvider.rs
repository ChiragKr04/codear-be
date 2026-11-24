#![allow(dead_code)] // public API surface; not every helper is used by the binary yet.

//! Non-streaming OpenAI Chat Completions provider, with function (tool) calling.
//!
//! One tool-calling turn:
//! 1. `send(&history, &tools)` → `Reply`.
//! 2. If `reply.tool_calls` is non-empty: push `reply.to_message()` onto the
//!    history, run each call, push `ChatMessage::tool_result(call.id, output)`
//!    for each, and `send` again.
//! 3. Otherwise `reply.content` is the model's final answer.
//!
//! All settings come from the environment (normally loaded from `.env`):
//! - `OPENAI_API_KEY`
//! - `OPENAI_BASE_URL`
//! - `OPENAI_MODEL`
//! - `OPENAI_TIMEOUT_SECS`

use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const ENV_API_KEY: &str = "OPENAI_API_KEY";
pub const ENV_BASE_URL: &str = "OPENAI_BASE_URL";
pub const ENV_MODEL: &str = "OPENAI_MODEL";
pub const ENV_TIMEOUT_SECS: &str = "OPENAI_TIMEOUT_SECS";

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

/// A message sent to the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    /// `None` (sent as `null`) only for an assistant turn that just calls tools.
    pub content: Option<String>,
    /// Tool calls made by an assistant turn, echoed back so the following
    /// `Role::Tool` messages can refer to them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// For `Role::Tool`: the `ToolCall::id` this message answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self { role, content: Some(content.into()), tool_calls: Vec::new(), tool_call_id: None }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    /// An assistant turn that calls tools, with optional accompanying text.
    pub fn assistant_tool_calls(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self { role: Role::Assistant, content, tool_calls, tool_call_id: None }
    }

    /// The output of running tool call `tool_call_id` (success or error text).
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// The only tool type Chat Completions has today.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolKind {
    #[default]
    Function,
}

/// A tool the model may call, as sent in the request's `tools` list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub kind: ToolKind,
    pub function: FunctionDefinition,
}

impl ToolDefinition {
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            kind: ToolKind::Function,
            function: FunctionDefinition {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema for the arguments object.
    pub parameters: serde_json::Value,
}

/// Body of `POST /chat/completions`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    /// Omitted when empty: the API rejects `"tools": []`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    /// Always `false`: this provider only does non-streaming calls.
    pub stream: bool,
}

impl ChatCompletionRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: Vec::new(),
            temperature: None,
            max_completion_tokens: None,
            stream: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Successful response from `POST /chat/completions`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

impl ChatCompletionResponse {
    /// Text content of the first choice, if any.
    pub fn first_content(&self) -> Option<&str> {
        self.choices.first()?.message.content.as_deref()
    }

    /// The first choice as a `Reply`; `EmptyResponse` if there are no choices.
    pub fn into_reply(self) -> Result<Reply, ProviderError> {
        let choice = self.choices.into_iter().next().ok_or(ProviderError::EmptyResponse)?;
        Ok(Reply {
            content: choice.message.content,
            refusal: choice.message.refusal,
            tool_calls: choice.message.tool_calls,
            finish_reason: choice.finish_reason,
            usage: self.usage,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// A message returned by the model. `content` can be `null`
/// (e.g. on refusals or tool calls).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ResponseMessage {
    pub role: Role,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub refusal: Option<String>,
    /// Missing or `null` when the model didn't call any tool.
    #[serde(default, deserialize_with = "null_as_default")]
    pub tool_calls: Vec<ToolCall>,
}

/// A tool call requested by the model. Sent back unchanged inside the
/// assistant message (`ChatMessage::tool_calls`) on the next request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Echo this in `ChatMessage::tool_result` to pair the result with the call.
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: ToolKind,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    /// JSON-encoded arguments object, exactly as the model wrote it. It may be
    /// invalid JSON or not match the schema, so validate when parsing.
    pub arguments: String,
}

/// The model's answer for one turn: text, tool calls, or both.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub content: Option<String>,
    pub refusal: Option<String>,
    /// Empty when the model answered without calling a tool.
    pub tool_calls: Vec<ToolCall>,
    /// e.g. `"stop"`, `"tool_calls"`, `"length"`.
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
}

impl Reply {
    /// The assistant message to append to the history before the tool results.
    pub fn to_message(&self) -> ChatMessage {
        ChatMessage::assistant_tool_calls(self.content.clone(), self.tool_calls.clone())
    }
}

/// Deserialize `null` as `T::default()` (some OpenAI-compatible servers send
/// `"tool_calls": null` instead of omitting it).
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Error body returned by the API on non-2xx responses.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ApiErrorResponse {
    pub error: ApiError,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ApiError {
    pub message: String,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub param: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ProviderError {
    MissingEnv(&'static str),
    InvalidEnv { name: &'static str, value: String },
    Http(reqwest::Error),
    Api { status: u16, message: String },
    Decode(serde_json::Error),
    EmptyResponse,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEnv(name) => write!(f, "{name} is not set (add it to .env)"),
            Self::InvalidEnv { name, value } => write!(f, "{name} has an invalid value: {value:?}"),
            Self::Http(e) => write!(f, "request to OpenAI failed: {e}"),
            Self::Api { status, message } => write!(f, "OpenAI API error ({status}): {message}"),
            Self::Decode(e) => write!(f, "could not decode OpenAI response: {e}"),
            Self::EmptyResponse => write!(f, "OpenAI returned no message content"),
        }
    }
}

impl std::error::Error for ProviderError {}

impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub timeout: Duration,
}

impl ProviderConfig {
    /// Read the config from process environment variables.
    pub fn from_env() -> Result<Self, ProviderError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Read the config through `lookup`, so it can be tested without touching the real env.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ProviderError> {
        let required = |name: &'static str| {
            lookup(name)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or(ProviderError::MissingEnv(name))
        };

        let timeout_raw = required(ENV_TIMEOUT_SECS)?;
        let timeout_secs: u64 = timeout_raw
            .parse()
            .ok()
            .filter(|&s| s > 0)
            .ok_or(ProviderError::InvalidEnv { name: ENV_TIMEOUT_SECS, value: timeout_raw })?;

        Ok(Self {
            api_key: required(ENV_API_KEY)?,
            base_url: required(ENV_BASE_URL)?.trim_end_matches('/').to_string(),
            model: required(ENV_MODEL)?,
            timeout: Duration::from_secs(timeout_secs),
        })
    }
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

pub struct OpenAIProvider {
    client: reqwest::blocking::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAIProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, ProviderError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(config.timeout)
            .build()?;
        Ok(Self {
            client,
            api_key: config.api_key,
            base_url: config.base_url,
            model: config.model,
        })
    }

    /// Build a provider from the `OPENAI_*` environment variables.
    pub fn from_env() -> Result<Self, ProviderError> {
        Self::new(ProviderConfig::from_env()?)
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Send a full chat completion request (non-streaming).
    pub fn chat(&self, request: &ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(request)
            .send()?;

        let status = response.status();
        let body = response.text()?;
        parse_response(status.as_u16(), &body)
    }

    /// Send the conversation so far plus the tools the model may call, and
    /// return its reply (text and/or tool calls). Pass `&[]` for no tools.
    pub fn send(&self, messages: &[ChatMessage], tools: &[ToolDefinition]) -> Result<Reply, ProviderError> {
        let mut request = ChatCompletionRequest::new(&self.model, messages.to_vec());
        request.tools = tools.to_vec();
        self.chat(&request)?.into_reply()
    }

    /// Convenience: send a single user prompt (no tools) and return the reply text.
    pub fn complete(&self, prompt: &str) -> Result<String, ProviderError> {
        self.send(&[ChatMessage::user(prompt)], &[])?
            .content
            .ok_or(ProviderError::EmptyResponse)
    }
}

/// Turn a raw HTTP status + body into a typed response or error.
fn parse_response(status: u16, body: &str) -> Result<ChatCompletionResponse, ProviderError> {
    if !(200..300).contains(&status) {
        let message = serde_json::from_str::<ApiErrorResponse>(body)
            .map(|e| e.error.message)
            .unwrap_or_else(|_| body.to_string());
        return Err(ProviderError::Api { status, message });
    }
    serde_json::from_str(body).map_err(ProviderError::Decode)
}

#[cfg(test)]
#[path = "tests/openai_provider_tests.rs"]
mod tests;
