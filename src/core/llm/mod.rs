pub mod types;

use anyhow::Result;
use async_trait::async_trait;

pub use types::{
    EmbeddingRequest, EmbeddingResponse, InputItem, OutputItem, ReasoningLevel, ResponseOutput,
    ResponseRequest, Role, StreamEvent, TokenUsage, ToolCall, ToolDefinition,
};

/// Trait for models that support the Responses API.
///
/// Based on the OpenAI Responses API that Ollama v0.13.3+, OpenAI,
/// and llama.cpp are converging toward.
#[async_trait]
pub trait ResponseModel: Send + Sync {
    async fn create_response(&self, req: ResponseRequest) -> Result<ResponseOutput>;

    /// Stream a response, calling `on_event` for each `StreamEvent`.
    ///
    /// Returns the final `ResponseOutput` once the stream completes.
    async fn stream_create_response(
        &self,
        req: ResponseRequest,
        on_event: &mut (dyn FnMut(StreamEvent) + Send),
    ) -> Result<ResponseOutput>;
}

/// Trait for embedding models.
///
/// Separate from `ResponseModel` because embedding and text generation
/// often have different optimal implementations.
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    async fn embed(&self, req: EmbeddingRequest) -> Result<EmbeddingResponse>;
}
