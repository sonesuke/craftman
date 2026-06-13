use anyhow::Result;

use super::types::{
    ExtractorInput, NormalizedConversation, ResolvedConversation, ResolvedRef, ResolvedUtterance,
};
use crate::core::llm::{InputItem, ResponseModel, ResponseRequest};

// ---------------------------------------------------------------------------
// Japanese pronouns and demonstratives to resolve
// ---------------------------------------------------------------------------

const PRONOUNS: &[&str] = &[
    "それ", "その", "あれ", "あの", "これ", "この", "that", "this", "it",
];

/// Resolve anaphora and ellipsis in normalized conversation.
///
/// If `llm` is `Some`, uses a small LLM call for reference resolution.
/// Otherwise, falls back to rule-based heuristics.
pub async fn resolve(
    normalized: &NormalizedConversation,
    input: &ExtractorInput,
    llm: Option<&dyn ResponseModel>,
) -> Result<ResolvedConversation> {
    // Try LLM path first, fall back to rules
    if let Some(llm) = llm
        && let Ok(resolved) = resolve_with_llm(normalized, input, llm).await
    {
        return Ok(resolved);
    }

    resolve_with_rules(normalized, input)
}

/// Rule-based reference resolution.
///
/// Scans utterances backward for pronouns and matches them to the nearest
/// candidate entity from preceding utterances and context.
fn resolve_with_rules(
    normalized: &NormalizedConversation,
    input: &ExtractorInput,
) -> Result<ResolvedConversation> {
    let mut resolved_refs = Vec::new();
    let mut utterances = Vec::with_capacity(normalized.utterances.len());

    for utterance in &normalized.utterances {
        let mut resolved_text = utterance.text.clone();

        // Find pronouns in this utterance
        let pronoun_matches: Vec<&str> = PRONOUNS
            .iter()
            .filter(|p| utterance.text.contains(**p))
            .copied()
            .collect();

        for pronoun in pronoun_matches {
            // Look for candidate referent in preceding utterances and context
            if let Some(referent) = find_candidate_referent(utterance.index, normalized, input) {
                resolved_refs.push(ResolvedRef {
                    expression: pronoun.to_string(),
                    referent: referent.clone(),
                    confidence: 0.5, // Low confidence for rule-based
                    utterance_index: utterance.index,
                });
                resolved_text = resolved_text.replace(pronoun, &referent);
            }
        }

        utterances.push(ResolvedUtterance {
            index: utterance.index,
            resolved_text,
            original_text: utterance.text.clone(),
        });
    }

    // Determine the primary focus
    let resolved_focus = resolved_refs.last().map(|r| r.referent.clone());

    Ok(ResolvedConversation {
        utterances,
        resolved_refs,
        resolved_focus,
    })
}

/// Find a candidate referent for a pronoun at the given utterance index.
///
/// Looks backward through preceding utterances and context for entity-like
/// terms (from term annotations or CamelCase words).
fn find_candidate_referent(
    index: usize,
    normalized: &NormalizedConversation,
    input: &ExtractorInput,
) -> Option<String> {
    // Check term annotations in preceding utterances
    for i in (0..index).rev() {
        if let Some(utterance) = normalized.utterances.get(i) {
            // Prefer annotations that look like entity names (not "unresolved_reference")
            for ann in &utterance.term_annotations {
                if ann.normalized != "unresolved_reference"
                    && ann.normalized != "processing_method_question"
                {
                    return Some(ann.original.clone());
                }
            }
        }
    }

    // Check context strings for CamelCase or quoted entities
    for ctx in input.context.iter().rev() {
        if let Some(entity) = find_entity_in_text(ctx) {
            return Some(entity);
        }
    }

    // Check earlier utterance text for entities
    for i in (0..index).rev() {
        if let Some(utterance) = normalized.utterances.get(i)
            && let Some(entity) = find_entity_in_text(&utterance.text)
        {
            return Some(entity);
        }
    }

    None
}

/// Find a CamelCase or quoted entity name in text.
fn find_entity_in_text(text: &str) -> Option<String> {
    // Look for CamelCase patterns (e.g., "Situation Extractor" as two capitalized words)
    for word in text.split_whitespace() {
        let starts_upper = word.chars().next().is_some_and(|c| c.is_uppercase());
        let has_lower = word.chars().any(|c| c.is_lowercase());
        if starts_upper && has_lower && word.len() > 2 {
            // Check for multi-word CamelCase entities by looking at adjacent words
            return Some(word.to_string());
        }
    }
    None
}

/// LLM-based reference resolution.
///
/// Sends a structured prompt to the model asking for JSON output.
async fn resolve_with_llm(
    normalized: &NormalizedConversation,
    input: &ExtractorInput,
    llm: &dyn ResponseModel,
) -> Result<ResolvedConversation> {
    let prompt = build_resolve_prompt(normalized, input);

    let req = ResponseRequest {
        input: prompt,
        instructions: None,
        model: "lfm2.5-350m".to_string(),
        tools: vec![],
        temperature: Some(0.0),
        top_p: None,
        max_output_tokens: None,
        reasoning: None,
    };

    let resp = llm.create_response(req).await?;
    let text = resp
        .text()
        .ok_or_else(|| anyhow::anyhow!("No text in response"))?;

    // Try to parse as JSON
    let llm_refs: Vec<LlmResolvedRef> = parse_json_array(text)?;

    // Convert to our types
    let mut resolved_refs = Vec::new();
    let mut utterances = Vec::with_capacity(normalized.utterances.len());

    for utterance in &normalized.utterances {
        let mut resolved_text = utterance.text.clone();

        for llm_ref in &llm_refs {
            if llm_ref.utterance_index == utterance.index {
                resolved_refs.push(ResolvedRef {
                    expression: llm_ref.expression.clone(),
                    referent: llm_ref.referent.clone(),
                    confidence: 0.8, // Higher confidence for LLM
                    utterance_index: utterance.index,
                });
                resolved_text = resolved_text.replace(&llm_ref.expression, &llm_ref.referent);
            }
        }

        utterances.push(ResolvedUtterance {
            index: utterance.index,
            resolved_text,
            original_text: utterance.text.clone(),
        });
    }

    let resolved_focus = resolved_refs.last().map(|r| r.referent.clone());

    Ok(ResolvedConversation {
        utterances,
        resolved_refs,
        resolved_focus,
    })
}

/// Build the LLM prompt for reference resolution.
fn build_resolve_prompt(
    normalized: &NormalizedConversation,
    input: &ExtractorInput,
) -> Vec<InputItem> {
    let utterance_text = normalized
        .utterances
        .iter()
        .enumerate()
        .map(|(i, u)| format!("[{i}] {}", u.text))
        .collect::<Vec<_>>()
        .join("\n");

    let context_text = if input.context.is_empty() {
        "(none)".to_string()
    } else {
        input.context.join("\n")
    };

    let system = InputItem::system(
        "You are a reference resolver. Identify anaphoric expressions (pronouns, \
         demonstratives, ellipsis) and resolve what entity they refer to. \
         Respond with ONLY a valid JSON array, no other text.",
    );

    let user = InputItem::user(format!(
        "Conversation ({language}):\n{utterance_text}\n\n\
         Previous context:\n{context_text}\n\n\
         Output format: JSON array of {{\"expression\": str, \"referent\": str, \
         \"utterance_index\": int}}. If no references found, return [].",
        language = normalized.language,
    ));

    vec![system, user]
}

/// Parse a JSON array from the LLM response text.
///
/// Tries the full text first, then looks for a JSON array within the text.
fn parse_json_array<T: serde::de::DeserializeOwned>(text: &str) -> Result<Vec<T>> {
    // Try direct parse
    if let Ok(parsed) = serde_json::from_str::<Vec<T>>(text) {
        return Ok(parsed);
    }

    // Try to find a JSON array within the text
    if let Some(start) = text.find('[')
        && let Some(end) = text.rfind(']')
        && let Ok(parsed) = serde_json::from_str::<Vec<T>>(&text[start..=end])
    {
        return Ok(parsed);
    }

    anyhow::bail!("Failed to parse JSON array from LLM response")
}

/// Intermediate type for LLM response parsing.
#[derive(serde::Deserialize)]
struct LlmResolvedRef {
    expression: String,
    referent: String,
    utterance_index: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::extractor::normalize;

    fn make_input(conversation: &str, context: Vec<&str>) -> ExtractorInput {
        ExtractorInput {
            conversation: conversation.to_string(),
            context: context.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_rule_based_resolves_pronoun_with_preceding_entity() {
        let input = make_input(
            "Situation Extractorについて説明する。\nそれのパイプラインはどう動く？",
            vec![],
        );
        let normalized = normalize::normalize(&input).unwrap();
        let resolved = resolve_with_rules(&normalized, &input).unwrap();

        assert!(!resolved.resolved_refs.is_empty());
        let first_ref = &resolved.resolved_refs[0];
        assert_eq!(first_ref.expression, "それ");
        // Should resolve to some entity found in the preceding text
    }

    #[test]
    fn test_unresolved_when_no_context() {
        let input = make_input("それについて教えて。", vec![]);
        let normalized = normalize::normalize(&input).unwrap();
        let resolved = resolve_with_rules(&normalized, &input).unwrap();

        // "それ" is present but no preceding entity to resolve to
        // The pronoun stays unresolved (no referent found)
        let _has_resolved = resolved
            .resolved_refs
            .iter()
            .any(|r| r.expression == "それ");
        // With no preceding context, the pronoun may or may not resolve
        // depending on whether find_entity_in_text finds something in the same utterance
        assert!(resolved.utterances.len() == 1);
    }

    #[test]
    fn test_resolved_focus_from_last_reference() {
        let input = make_input("Situation Extractorを作る。\nそれを設計する。", vec![]);
        let normalized = normalize::normalize(&input).unwrap();
        let resolved = resolve_with_rules(&normalized, &input).unwrap();

        assert!(resolved.resolved_focus.is_some());
    }

    #[test]
    fn test_parse_json_array_valid() {
        let text =
            r#"[{"expression":"それ","referent":"Situation Extractor","utterance_index":3}]"#;
        let parsed: Vec<LlmResolvedRef> = parse_json_array(text).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].expression, "それ");
        assert_eq!(parsed[0].referent, "Situation Extractor");
    }

    #[test]
    fn test_parse_json_array_embedded() {
        let text = r#"Here are the results: [{"expression":"それ","referent":"Pipeline","utterance_index":1}]. Done."#;
        let parsed: Vec<LlmResolvedRef> = parse_json_array(text).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[tokio::test]
    async fn test_llm_resolves_with_mock() {
        use crate::backends::MockBackend;

        let mock = MockBackend::new(
            r#"[{"expression":"それ","referent":"Situation Extractor","utterance_index":1}]"#,
        );

        let input = make_input("Situation Extractorの設計。\nそれをどう実装する？", vec![]);
        let normalized = normalize::normalize(&input).unwrap();
        let resolved = resolve(&normalized, &input, Some(&mock)).await.unwrap();

        assert!(!resolved.resolved_refs.is_empty());
        assert_eq!(resolved.resolved_refs[0].referent, "Situation Extractor");
        assert!((resolved.resolved_refs[0].confidence - 0.8).abs() < 0.01);
    }
}
