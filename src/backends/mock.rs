use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;

use crate::core::llm::{
    EmbeddingModel, EmbeddingRequest, EmbeddingResponse, OutputItem, ResponseModel, ResponseOutput,
    ResponseRequest, Role, StreamEvent, TokenUsage,
};

/// A mock backend for testing.
///
/// Holds a queue of scripted `ResponseOutput`s. Each `create_response` /
/// `stream_create_response` call pops the next one, so a multi-round tool
/// chain (e.g. `activate_skill` -> `calculator` -> final answer) can be
/// replayed deterministically.
pub struct MockBackend {
    responses: Arc<Mutex<VecDeque<ResponseOutput>>>,
    embedding_response: Arc<Mutex<Vec<Vec<f32>>>>,
}

fn mock_message(text: String) -> ResponseOutput {
    ResponseOutput {
        id: "resp_mock".to_string(),
        model: "mock".to_string(),
        output: vec![OutputItem::Message {
            role: Role::Assistant,
            content: text,
        }],
        usage: TokenUsage::default(),
    }
}

impl MockBackend {
    /// Create a mock that replies with the given text to every call.
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(vec![mock_message(
                response.into(),
            )]))),
            embedding_response: Arc::new(Mutex::new(vec![vec![0.1; 4]])),
        }
    }

    /// Replace the single canned text response.
    pub fn set_response(&self, text: impl Into<String>) {
        let mut g = self.responses.lock().unwrap();
        g.clear();
        g.push_back(mock_message(text.into()));
    }

    /// Replace the response queue (front = first call). Used to script a
    /// sequence of outputs across multiple round-trips.
    pub fn set_responses(&self, responses: Vec<ResponseOutput>) {
        let mut g = self.responses.lock().unwrap();
        *g = responses.into();
    }

    pub fn set_embedding(&self, embeddings: Vec<Vec<f32>>) {
        *self.embedding_response.lock().unwrap() = embeddings;
    }
}

#[async_trait]
impl ResponseModel for MockBackend {
    async fn create_response(&self, _req: ResponseRequest) -> Result<ResponseOutput> {
        let g = self.responses.lock().unwrap();
        Ok(g.front()
            .cloned()
            .unwrap_or_else(|| mock_message(String::new())))
    }

    async fn stream_create_response(
        &self,
        _req: ResponseRequest,
        on_event: &mut (dyn FnMut(StreamEvent) + Send),
    ) -> Result<ResponseOutput> {
        let resp = { self.responses.lock().unwrap().pop_front() }
            .unwrap_or_else(|| mock_message(String::new()));
        if let Some(text) = resp.text() {
            on_event(StreamEvent::TextDelta(text.to_string()));
        }
        on_event(StreamEvent::Done(resp.clone()));
        Ok(resp)
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
