use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Role
// ---------------------------------------------------------------------------

/// Message role in a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

/// An input item for a Responses API request.
///
/// Can be a simple text message, or extended in the future with
/// file attachments, images, etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputItem {
    Message { role: Role, content: String },
}

impl InputItem {
    pub fn user(content: impl Into<String>) -> Self {
        Self::Message {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::Message {
            role: Role::Assistant,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::Message {
            role: Role::System,
            content: content.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Reasoning
// ---------------------------------------------------------------------------

/// Reasoning effort level for thinking models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningLevel {
    Low,
    Medium,
    High,
}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

/// A tool definition for function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// A tool call in a response output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// An output item in a Responses API response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputItem {
    Message {
        role: Role,
        content: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    Reasoning {
        summary: String,
    },
}

// ---------------------------------------------------------------------------
// Request / Response
// ---------------------------------------------------------------------------

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

/// A Responses API request.
///
/// Modeled after the OpenAI Responses API that Ollama v0.13.3+,
/// OpenAI, and llama.cpp are converging toward.
///
/// If `model` is empty, the backend uses its configured default model.
#[derive(Debug, Clone)]
pub struct ResponseRequest {
    pub input: Vec<InputItem>,
    pub instructions: Option<String>,
    pub model: String,
    pub tools: Vec<ToolDefinition>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub reasoning: Option<ReasoningLevel>,
}

/// A Responses API response.
#[derive(Debug, Clone)]
pub struct ResponseOutput {
    pub id: String,
    pub model: String,
    pub output: Vec<OutputItem>,
    pub usage: TokenUsage,
}

impl ResponseOutput {
    /// Extracts the first text content from the output items.
    pub fn text(&self) -> Option<&str> {
        self.output.iter().find_map(|item| match item {
            OutputItem::Message { content, .. } => Some(content.as_str()),
            _ => None,
        })
    }
}

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

/// An event emitted during a streaming response.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A chunk of reasoning (thinking) text.
    ReasoningDelta(String),
    /// A chunk of output text.
    TextDelta(String),
    /// The response is complete; carries the full `ResponseOutput`.
    Done(ResponseOutput),
}

// ---------------------------------------------------------------------------
// Embedding
// ---------------------------------------------------------------------------

/// An embedding request.
///
/// If `model` is empty, the backend uses its configured default model.
#[derive(Debug, Clone)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: Vec<String>,
}

/// An embedding response.
#[derive(Debug, Clone)]
pub struct EmbeddingResponse {
    pub model: String,
    pub embeddings: Vec<Vec<f32>>,
    pub usage: TokenUsage,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_item_constructors() {
        let item = InputItem::user("hello");
        assert!(matches!(
            item,
            InputItem::Message {
                role: Role::User,
                ..
            }
        ));

        let item = InputItem::assistant("hi there");
        assert!(matches!(
            item,
            InputItem::Message {
                role: Role::Assistant,
                ..
            }
        ));

        let item = InputItem::system("you are helpful");
        assert!(matches!(
            item,
            InputItem::Message {
                role: Role::System,
                ..
            }
        ));
    }

    #[test]
    fn test_output_item_text_extraction() {
        let output = ResponseOutput {
            id: "resp_123".to_string(),
            model: "test".to_string(),
            output: vec![
                OutputItem::Reasoning {
                    summary: "thinking...".to_string(),
                },
                OutputItem::Message {
                    role: Role::Assistant,
                    content: "Hello!".to_string(),
                },
            ],
            usage: TokenUsage::default(),
        };

        assert_eq!(output.text(), Some("Hello!"));
    }

    #[test]
    fn test_reasoning_level_serde() {
        let levels = vec![
            ReasoningLevel::Low,
            ReasoningLevel::Medium,
            ReasoningLevel::High,
        ];
        for level in levels {
            let json = serde_json::to_string(&level).unwrap();
            let deserialized: ReasoningLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(level, deserialized);
        }
    }

    #[test]
    fn test_input_item_serde_roundtrip() {
        let item = InputItem::user("hello");
        let json = serde_json::to_string(&item).unwrap();
        let deserialized: InputItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deserialized);
    }

    #[test]
    fn test_output_item_serde_roundtrip() {
        let item = OutputItem::Message {
            role: Role::Assistant,
            content: "test".to_string(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let deserialized: OutputItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deserialized);
    }
}
