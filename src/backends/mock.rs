use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;

use crate::core::llm::{
    EmbeddingModel, EmbeddingRequest, EmbeddingResponse, OutputItem, ResponseModel, ResponseOutput,
    ResponseRequest, Role, TokenUsage,
};

/// A mock backend for testing.
///
/// Returns a configurable fixed response. The response can be changed
/// between calls via `set_response()`.
pub struct MockBackend {
    text_response: Arc<Mutex<String>>,
    embedding_response: Arc<Mutex<Vec<Vec<f32>>>>,
}

impl MockBackend {
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            text_response: Arc::new(Mutex::new(response.into())),
            embedding_response: Arc::new(Mutex::new(vec![vec![0.1; 4]])),
        }
    }

    pub fn set_response(&self, text: impl Into<String>) {
        *self.text_response.lock().unwrap() = text.into();
    }

    pub fn set_embedding(&self, embeddings: Vec<Vec<f32>>) {
        *self.embedding_response.lock().unwrap() = embeddings;
    }
}

#[async_trait]
impl ResponseModel for MockBackend {
    async fn create_response(&self, _req: ResponseRequest) -> Result<ResponseOutput> {
        let text = self.text_response.lock().unwrap().clone();
        Ok(ResponseOutput {
            id: format!(
                "resp_mock_{}",
                std::sync::atomic::AtomicU64::new(0).load(std::sync::atomic::Ordering::Relaxed)
            ),
            model: "mock".to_string(),
            output: vec![OutputItem::Message {
                role: Role::Assistant,
                content: text,
            }],
            usage: TokenUsage::default(),
        })
    }
}

#[async_trait]
impl EmbeddingModel for MockBackend {
    async fn embed(&self, _req: EmbeddingRequest) -> Result<EmbeddingResponse> {
        let embeddings = self.embedding_response.lock().unwrap().clone();
        Ok(EmbeddingResponse {
            model: "mock".to_string(),
            embeddings,
            usage: TokenUsage::default(),
        })
    }
}
