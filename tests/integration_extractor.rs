use std::sync::Arc;

use craftman::MockBackend;
use craftman::core::extractor::types::{Domain, ExtractionOutput, TaskType};
use craftman::core::extractor::{ExtractorInput, SituationExtractor};

// ---------------------------------------------------------------------------
// Rule-based pipeline (no LLM)
// ---------------------------------------------------------------------------

#[test]
fn test_full_pipeline_rule_based() {
    let extractor = SituationExtractor::rule_based();

    let input = ExtractorInput {
        conversation:
            "Situation Extractorの設計について。\nパイプラインの各ステージをどう実装する？"
                .to_string(),
        context: vec![],
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(extractor.extract(input)).unwrap();

    assert!(
        !output.hypotheses.is_empty(),
        "should produce at least one hypothesis"
    );
    assert!(
        output.hypotheses[0].confidence > 0.0,
        "primary hypothesis should have non-zero confidence"
    );
}

#[test]
fn test_pipeline_detects_software_engineering_domain() {
    let extractor = SituationExtractor::rule_based();

    let input = ExtractorInput {
        conversation: "Pipelineモジュールを実装する。Extractorコンポーネントの設計。".to_string(),
        context: vec![],
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(extractor.extract(input)).unwrap();

    let top = &output.hypotheses[0];
    assert_eq!(top.situation.domain, Domain::SoftwareEngineering);
}

#[test]
fn test_pipeline_empty_conversation() {
    let extractor = SituationExtractor::rule_based();

    let input = ExtractorInput {
        conversation: "".to_string(),
        context: vec![],
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(extractor.extract(input)).unwrap();

    // Empty input should still produce a result (possibly empty hypotheses)
    // but should not error
    assert!(
        output.extraction_notes.missing_information.is_empty() || !output.hypotheses.is_empty()
    );
}

#[test]
fn test_pipeline_with_context() {
    let extractor = SituationExtractor::rule_based();

    let input = ExtractorInput {
        conversation: "それのパイプラインをどう実装する？".to_string(),
        context: vec!["Situation Extractorについて議論中".to_string()],
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(extractor.extract(input)).unwrap();

    assert!(!output.hypotheses.is_empty());
}

#[test]
fn test_pipeline_question_detection() {
    let extractor = SituationExtractor::rule_based();

    let input = ExtractorInput {
        conversation: "このパイプラインはどう動く？".to_string(),
        context: vec![],
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(extractor.extract(input)).unwrap();

    let top = &output.hypotheses[0];
    assert_eq!(top.situation.task_type, TaskType::Question);
}

// ---------------------------------------------------------------------------
// Mock LLM pipeline
// ---------------------------------------------------------------------------

#[test]
fn test_full_pipeline_with_mock_llm() {
    // Mock returns JSON signals for both resolve and signal stages.
    // Since MockBackend returns the same text for all calls, we provide a
    // response that works for both stages: an array of signal objects.
    let mock = MockBackend::new(
        r#"[{"kind":"artifact","value":"Situation Extractor","utterance_index":0,"confidence":0.9}]"#,
    );

    let extractor = SituationExtractor::new(Arc::new(mock));

    let input = ExtractorInput {
        conversation: "Situation Extractorのパイプライン設計。どうやって実装する？".to_string(),
        context: vec![],
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(extractor.extract(input)).unwrap();

    assert!(!output.hypotheses.is_empty());
    // The mock LLM returns an artifact signal, so the domain should be SoftwareEngineering
    assert_eq!(
        output.hypotheses[0].situation.domain,
        Domain::SoftwareEngineering
    );
}

#[test]
fn test_pipeline_json_serialization() {
    let extractor = SituationExtractor::rule_based();

    let input = ExtractorInput {
        conversation: "Extractorモジュールの設計について。".to_string(),
        context: vec![],
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(extractor.extract(input)).unwrap();

    // Verify the output can be serialized to JSON
    let json = serde_json::to_string_pretty(&output);
    assert!(json.is_ok(), "output should be JSON-serializable");

    // Verify it can be deserialized back
    let deserialized: Result<ExtractionOutput, _> = serde_json::from_str(&json.unwrap());
    assert!(deserialized.is_ok(), "output should roundtrip through JSON");
}

#[test]
fn test_pipeline_excludes_domain_general_for_software() {
    let extractor = SituationExtractor::rule_based();

    let input = ExtractorInput {
        conversation: "パイプラインのコンポーネントを実装する。".to_string(),
        context: vec![],
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(extractor.extract(input)).unwrap();

    // None of the hypotheses should be General when software terms are present
    let all_general = output
        .hypotheses
        .iter()
        .all(|h| h.situation.domain == Domain::General);
    assert!(
        !all_general,
        "at least one hypothesis should be SoftwareEngineering"
    );
}
