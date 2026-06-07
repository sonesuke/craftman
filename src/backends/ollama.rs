use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::core::llm::{
    EmbeddingModel, EmbeddingRequest, EmbeddingResponse, InputItem, OutputItem, ReasoningLevel,
    ResponseModel, ResponseOutput, ResponseRequest, Role, StreamEvent, TokenUsage, ToolDefinition,
};

// ---------------------------------------------------------------------------
// Ollama Responses API types (private — never exposed to core)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct OllamaResponseRequest {
    model: String,
    input: Vec<OllamaInputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OllamaToolDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<OllamaReasoning>,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OllamaInputItem {
    Message {
        role: String,
        content: String,
    },
    #[serde(rename = "function_call")]
    ToolCall {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        name: String,
        #[serde(serialize_with = "serialize_value_as_json_string")]
        arguments: serde_json::Value,
    },
    #[serde(rename = "function_call_output")]
    ToolResult {
        call_id: String,
        output: String,
    },
}

#[derive(Serialize)]
struct OllamaReasoning {
    effort: String,
}

#[derive(Serialize)]
struct OllamaToolDefinition {
    #[serde(rename = "type")]
    tool_type: String,
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct OllamaResponseOutput {
    id: String,
    model: String,
    output: Vec<OllamaOutputItem>,
    usage: OllamaUsage,
}

#[derive(Deserialize)]
struct OllamaUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    #[allow(dead_code)]
    total_tokens: Option<u64>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OllamaOutputItem {
    Message {
        role: String,
        content: OllamaContent,
    },
    #[serde(rename = "function_call")]
    ToolCall {
        id: String,
        #[serde(default)]
        call_id: Option<String>,
        name: String,
        #[serde(deserialize_with = "deserialize_json_or_string")]
        arguments: serde_json::Value,
    },
    Reasoning {
        summary: OllamaSummary,
    },
}

/// The `content` field can be either a plain string or an array of
/// `output_text` objects (OpenAI Responses API format).
#[derive(Deserialize)]
#[serde(untagged)]
enum OllamaContent {
    Plain(String),
    Parts(Vec<OllamaTextPart>),
}

#[derive(Deserialize)]
struct OllamaTextPart {
    text: String,
}

/// The `summary` field in a reasoning item can be either a plain string
/// or an array of `summary_text` objects.
#[derive(Deserialize)]
#[serde(untagged)]
enum OllamaSummary {
    Plain(String),
    Parts(Vec<OllamaSummaryPart>),
}

#[derive(Deserialize)]
struct OllamaSummaryPart {
    text: String,
}

// Embedding types
#[derive(Serialize)]
struct OllamaEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    model: String,
    embeddings: Vec<Vec<f32>>,
    prompt_eval_count: Option<u64>,
}

// ---------------------------------------------------------------------------
// SSE streaming types (private)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SseDelta {
    delta: String,
}

#[derive(Deserialize)]
struct SseCompleted {
    response: OllamaResponseOutput,
}

// ---------------------------------------------------------------------------
// Deserializer helpers
// ---------------------------------------------------------------------------

/// Deserialize a field that may be a JSON object or a JSON-encoded string.
///
/// Ollama returns tool call `arguments` as a JSON string (`"{\"name\":\"x\"}"`)
/// while the OpenAI spec uses a JSON object. This handles both.
fn deserialize_json_or_string<'de, D>(de: D) -> std::result::Result<serde_json::Value, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    let value = serde_json::Value::deserialize(de)?;
    match &value {
        serde_json::Value::String(s) => serde_json::from_str(s)
            .map_err(|e| de::Error::custom(format!("invalid JSON in arguments string: {e}"))),
        _ => Ok(value),
    }
}

/// Serialize a serde_json::Value as a JSON string (not a JSON object).
///
/// Ollama requires `arguments` in function_call input items to be a string,
/// e.g. `"{\"name\":\"calculator\"}"` rather than `{"name":"calculator"}`.
fn serialize_value_as_json_string<S>(
    value: &serde_json::Value,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // If it's already a string, send as-is; otherwise serialize to a JSON string
    match value {
        serde_json::Value::String(s) => serializer.serialize_str(s),
        _ => serializer.serialize_str(&value.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

fn role_to_str(role: &Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

fn str_to_role(s: &str) -> Role {
    match s {
        "system" => Role::System,
        "user" => Role::User,
        _ => Role::Assistant,
    }
}

fn reasoning_effort_str(level: ReasoningLevel) -> &'static str {
    match level {
        ReasoningLevel::Low => "low",
        ReasoningLevel::Medium => "medium",
        ReasoningLevel::High => "high",
    }
}

fn to_ollama_input(items: &[InputItem]) -> Vec<OllamaInputItem> {
    items
        .iter()
        .map(|item| match item {
            InputItem::Message { role, content } => OllamaInputItem::Message {
                role: role_to_str(role).to_string(),
                content: content.clone(),
            },
            InputItem::ToolCall {
                id,
                call_id,
                name,
                arguments,
            } => OllamaInputItem::ToolCall {
                id: id.clone(),
                call_id: call_id.clone(),
                name: name.clone(),
                arguments: arguments.clone(),
            },
            InputItem::ToolResult { call_id, output } => OllamaInputItem::ToolResult {
                call_id: call_id.clone(),
                output: output.clone(),
            },
        })
        .collect()
}

fn to_ollama_tools(tools: &[ToolDefinition]) -> Vec<OllamaToolDefinition> {
    tools
        .iter()
        .map(|t| OllamaToolDefinition {
            tool_type: "function".to_string(),
            name: t.name.clone(),
            description: t.description.clone(),
            parameters: t.parameters.clone(),
        })
        .collect()
}

fn extract_text_from_content(content: OllamaContent) -> String {
    match content {
        OllamaContent::Plain(s) => s,
        OllamaContent::Parts(parts) => parts.into_iter().map(|p| p.text).collect(),
    }
}

fn extract_text_from_summary(summary: OllamaSummary) -> String {
    match summary {
        OllamaSummary::Plain(s) => s,
        OllamaSummary::Parts(parts) => parts.into_iter().map(|p| p.text).collect(),
    }
}

fn from_ollama_output(items: Vec<OllamaOutputItem>) -> Vec<OutputItem> {
    items
        .into_iter()
        .map(|item| match item {
            OllamaOutputItem::Message { role, content } => OutputItem::Message {
                role: str_to_role(&role),
                content: extract_text_from_content(content),
            },
            OllamaOutputItem::ToolCall {
                id,
                call_id,
                name,
                arguments,
            } => OutputItem::ToolCall {
                id,
                call_id,
                name,
                arguments,
            },
            OllamaOutputItem::Reasoning { summary } => OutputItem::Reasoning {
                summary: extract_text_from_summary(summary),
            },
        })
        .collect()
}

fn resolve_model<'a>(req_model: &'a str, default_model: &'a str) -> &'a str {
    if req_model.is_empty() {
        default_model
    } else {
        req_model
    }
}

// ---------------------------------------------------------------------------
// OllamaBackend
// ---------------------------------------------------------------------------

/// Backend that communicates with an [Ollama](https://ollama.com) server
/// via the OpenAI-compatible Responses API (`/v1/responses`).
///
/// Requires Ollama v0.13.3+ which supports the `/v1/responses` endpoint.
pub struct OllamaBackend {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaBackend {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    /// Creates a backend targeting the Ollama instance on the macOS host.
    ///
    /// Uses `host.docker.internal` so it works from inside a Docker container
    /// on Docker Desktop for Mac. When running natively on the host, use
    /// `new("http://localhost:11434", model)` instead.
    pub fn with_default_config(model: impl Into<String>) -> Self {
        Self::new("http://host.docker.internal:11434", model)
    }

    /// Stream a response, calling `on_event` for each SSE event.
    ///
    /// Returns the final `ResponseOutput` once the stream completes.
    pub async fn stream_create_response<F>(
        &self,
        req: ResponseRequest,
        mut on_event: F,
    ) -> Result<ResponseOutput>
    where
        F: FnMut(StreamEvent),
    {
        let model = resolve_model(&req.model, &self.model);

        let ollama_req = OllamaResponseRequest {
            model: model.to_string(),
            input: to_ollama_input(&req.input),
            instructions: req.instructions,
            tools: to_ollama_tools(&req.tools),
            temperature: req.temperature,
            top_p: req.top_p,
            max_output_tokens: req.max_output_tokens,
            reasoning: req.reasoning.map(|r| OllamaReasoning {
                effort: reasoning_effort_str(r).to_string(),
            }),
            stream: true,
        };

        let url = format!("{}/v1/responses", self.base_url);
        let resp = self.client.post(&url).json(&ollama_req).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama streaming request failed: HTTP {}", resp.status());
        }

        let mut buffer = String::new();
        let mut final_output: Option<ResponseOutput> = None;

        let mut resp = resp;

        while let Some(chunk) = resp.chunk().await? {
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE events (delimited by \n\n)
            while let Some(pos) = buffer.find("\n\n") {
                let event_block = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                let mut event_type = "";
                let mut data_str = "";

                for line in event_block.lines() {
                    if let Some(ev) = line.strip_prefix("event: ") {
                        event_type = ev.trim();
                    } else if let Some(d) = line.strip_prefix("data: ") {
                        data_str = d.trim();
                    }
                }

                if data_str.is_empty() {
                    continue;
                }

                match event_type {
                    "response.reasoning_summary_text.delta" => {
                        if let Ok(delta) = serde_json::from_str::<SseDelta>(data_str) {
                            on_event(StreamEvent::ReasoningDelta(delta.delta));
                        }
                    }
                    "response.output_text.delta" => {
                        if let Ok(delta) = serde_json::from_str::<SseDelta>(data_str) {
                            on_event(StreamEvent::TextDelta(delta.delta));
                        }
                    }
                    "response.completed" => {
                        if let Ok(completed) = serde_json::from_str::<SseCompleted>(data_str) {
                            let ollama_resp = completed.response;
                            let input_tokens = ollama_resp.usage.input_tokens.unwrap_or(0) as u32;
                            let output_tokens = ollama_resp.usage.output_tokens.unwrap_or(0) as u32;

                            let output = ResponseOutput {
                                id: ollama_resp.id,
                                model: ollama_resp.model,
                                output: from_ollama_output(ollama_resp.output),
                                usage: TokenUsage {
                                    input_tokens,
                                    output_tokens,
                                    total_tokens: input_tokens + output_tokens,
                                },
                            };

                            on_event(StreamEvent::Done(output.clone()));
                            final_output = Some(output);
                        }
                    }
                    _ => {
                        // Ignore other SSE event types
                    }
                }
            }
        }

        final_output.ok_or_else(|| anyhow::anyhow!("Stream ended without response.completed"))
    }
}

#[async_trait]
impl ResponseModel for OllamaBackend {
    async fn create_response(&self, req: ResponseRequest) -> Result<ResponseOutput> {
        let model = resolve_model(&req.model, &self.model);

        let ollama_req = OllamaResponseRequest {
            model: model.to_string(),
            input: to_ollama_input(&req.input),
            instructions: req.instructions,
            tools: to_ollama_tools(&req.tools),
            temperature: req.temperature,
            top_p: req.top_p,
            max_output_tokens: req.max_output_tokens,
            reasoning: req.reasoning.map(|r| OllamaReasoning {
                effort: reasoning_effort_str(r).to_string(),
            }),
            stream: false,
        };

        let url = format!("{}/v1/responses", self.base_url);
        let resp = self.client.post(&url).json(&ollama_req).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama responses request failed: HTTP {}", resp.status());
        }

        let ollama_resp: OllamaResponseOutput = resp.json().await?;

        let input_tokens = ollama_resp.usage.input_tokens.unwrap_or(0) as u32;
        let output_tokens = ollama_resp.usage.output_tokens.unwrap_or(0) as u32;

        Ok(ResponseOutput {
            id: ollama_resp.id,
            model: ollama_resp.model,
            output: from_ollama_output(ollama_resp.output),
            usage: TokenUsage {
                input_tokens,
                output_tokens,
                total_tokens: input_tokens + output_tokens,
            },
        })
    }
}

#[async_trait]
impl EmbeddingModel for OllamaBackend {
    async fn embed(&self, req: EmbeddingRequest) -> Result<EmbeddingResponse> {
        let model = resolve_model(&req.model, &self.model);

        let ollama_req = OllamaEmbedRequest {
            model: model.to_string(),
            input: req.input,
        };

        let url = format!("{}/api/embed", self.base_url);
        let resp = self.client.post(&url).json(&ollama_req).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!("Ollama embed request failed: HTTP {}", resp.status());
        }

        let ollama_resp: OllamaEmbedResponse = resp.json().await?;

        let input_tokens = ollama_resp.prompt_eval_count.unwrap_or(0) as u32;

        Ok(EmbeddingResponse {
            model: ollama_resp.model,
            embeddings: ollama_resp.embeddings,
            usage: TokenUsage {
                input_tokens,
                output_tokens: 0,
                total_tokens: input_tokens,
            },
        })
    }
}
