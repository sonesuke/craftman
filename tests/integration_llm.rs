use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use craftman::MockBackend;
use craftman::core::harness::{EventSink, Harness, HarnessEvent};
use craftman::core::llm::{OutputItem, ResponseOutput, Role, TokenUsage};
use craftman::core::skill::SkillRegistry;
use craftman::tools::ToolRegistry;
use craftman::tools::calculator::Calculator;

/// Sink that accumulates assistant text deltas for assertions.
struct CollectSink {
    text: Arc<Mutex<String>>,
}

impl EventSink for CollectSink {
    fn on_event(&mut self, event: &HarnessEvent) {
        if let HarnessEvent::AssistantDelta { text } = event
            && let Ok(mut g) = self.text.lock()
        {
            g.push_str(text);
        }
    }
}

/// With no skills/tools, Harness behaves as plain Q&A.
#[tokio::test]
async fn test_harness_plain_qa_with_mock() {
    let text = Arc::new(Mutex::new(String::new()));
    let mock = MockBackend::new("Rust is great!");
    let mut harness = Harness::new(mock, SkillRegistry::new(), ToolRegistry::new());
    harness.add_sink(Box::new(CollectSink {
        text: Arc::clone(&text),
    }));
    harness.submit("Tell me about Rust").await.unwrap();
    assert_eq!(text.lock().unwrap().as_str(), "Rust is great!");
}

/// The full retrieval -> activate_skill -> calculator -> answer chain, driven
/// by a scripted mock so it is deterministic (not the model's probabilistic
/// choice). This is the regression guard for the harness logic.
#[tokio::test]
async fn test_harness_skill_chain_with_mock() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(Box::new(Calculator));

    let mock = MockBackend::new("unused");
    mock.set_responses(vec![
        ResponseOutput {
            id: "1".to_string(),
            model: "mock".to_string(),
            output: vec![OutputItem::ToolCall {
                id: "call_1".to_string(),
                call_id: Some("call_1".to_string()),
                name: "activate_skill".to_string(),
                arguments: serde_json::json!({"name": "arithmetic"}),
            }],
            usage: TokenUsage::default(),
        },
        ResponseOutput {
            id: "2".to_string(),
            model: "mock".to_string(),
            output: vec![OutputItem::ToolCall {
                id: "call_2".to_string(),
                call_id: Some("call_2".to_string()),
                name: "calculator".to_string(),
                arguments: serde_json::json!({"expression": "123 * 456"}),
            }],
            usage: TokenUsage::default(),
        },
        ResponseOutput {
            id: "3".to_string(),
            model: "mock".to_string(),
            output: vec![OutputItem::Message {
                role: Role::Assistant,
                content: "56088".to_string(),
            }],
            usage: TokenUsage::default(),
        },
    ]);

    let text = Arc::new(Mutex::new(String::new()));
    let mut harness = Harness::new(mock, registry, tool_registry);
    harness.add_sink(Box::new(CollectSink {
        text: Arc::clone(&text),
    }));
    harness.submit("calculate 123 * 456").await.unwrap();

    // The final assistant message was the scripted answer; the calculator
    // tool was actually executed by the harness in between.
    assert_eq!(text.lock().unwrap().as_str(), "56088");
}

#[tokio::test]
async fn test_mock_response_model_directly() {
    use craftman::core::llm::{InputItem, ResponseModel, ResponseRequest};

    let mock = MockBackend::new("direct response");
    let req = ResponseRequest {
        input: vec![InputItem::user("test")],
        instructions: None,
        model: String::new(),
        tools: vec![],
        temperature: None,
        top_p: None,
        max_output_tokens: None,
        reasoning: None,
    };

    let resp = mock.create_response(req).await.unwrap();
    assert_eq!(resp.text(), Some("direct response"));
    assert_eq!(resp.model, "mock");
}

#[tokio::test]
async fn test_mock_embedding_model() {
    use craftman::core::llm::{EmbeddingModel, EmbeddingRequest};

    let mock = MockBackend::new("unused");
    let req = EmbeddingRequest {
        model: String::new(),
        input: vec!["hello".to_string()],
    };

    let resp = mock.embed(req).await.unwrap();
    assert_eq!(resp.model, "mock");
    assert_eq!(resp.embeddings.len(), 1);
    assert_eq!(resp.embeddings[0].len(), 4);
}

#[tokio::test]
async fn test_mock_set_response() {
    use craftman::core::llm::{InputItem, ResponseModel, ResponseRequest};

    let mock = MockBackend::new("original");
    mock.set_response("updated");

    let req = ResponseRequest {
        input: vec![InputItem::user("hi")],
        instructions: None,
        model: String::new(),
        tools: vec![],
        temperature: None,
        top_p: None,
        max_output_tokens: None,
        reasoning: None,
    };

    let resp = mock.create_response(req).await.unwrap();
    assert_eq!(resp.text(), Some("updated"));
}
