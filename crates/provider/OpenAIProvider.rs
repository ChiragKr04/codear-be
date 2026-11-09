#![allow(dead_code)] // public API surface; not every helper is used by the binary yet.

//! Non-streaming OpenAI Chat Completions provider.
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
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: Role::System, content: content.into() }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

/// Body of `POST /chat/completions`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
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

    /// Convenience: send a single user prompt and return the reply text.
    pub fn complete(&self, prompt: &str) -> Result<String, ProviderError> {
        let request = ChatCompletionRequest::new(&self.model, vec![ChatMessage::user(prompt)]);
        let response = self.chat(&request)?;
        response
            .first_content()
            .map(str::to_string)
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
