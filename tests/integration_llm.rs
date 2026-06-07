use craftman::{AssistantService, MockBackend};

#[tokio::test]
async fn test_assistant_service_with_mock() {
    let mock = MockBackend::new("Rust is great!");
    let service = AssistantService::new(mock);
    let answer = service.answer("Tell me about Rust").await.unwrap();
    assert_eq!(answer, "Rust is great!");
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
