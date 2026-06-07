use anyhow::Result;

use crate::core::llm::{InputItem, ResponseModel, ResponseRequest};

/// Application-level service that answers user questions via an LLM.
///
/// Generic over any `ResponseModel` backend — the service does not know
/// or care whether it is talking to Ollama, llama.cpp, or a mock.
pub struct AssistantService<L: ResponseModel> {
    llm: L,
}

impl<L: ResponseModel> AssistantService<L> {
    pub fn new(llm: L) -> Self {
        Self { llm }
    }

    /// Sends a user input string and returns the assistant's text response.
    pub async fn answer(&self, user_input: &str) -> Result<String> {
        let req = ResponseRequest {
            input: vec![InputItem::user(user_input)],
            instructions: None,
            model: String::new(),
            tools: vec![],
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            reasoning: None,
        };
        let resp = self.llm.create_response(req).await?;
        match resp.text() {
            Some(text) => Ok(text.to_string()),
            None => anyhow::bail!("No text content in response"),
        }
    }
}
