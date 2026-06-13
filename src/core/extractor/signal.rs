use anyhow::Result;

use super::types::{ResolvedConversation, Signal, SignalKind};
use crate::core::llm::{InputItem, ResponseModel, ResponseRequest};

// ---------------------------------------------------------------------------
// Keyword lists for rule-based signal extraction
// ---------------------------------------------------------------------------

const GOAL_KEYWORDS: &[&str] = &[
    "want to",
    "need to",
    "goal",
    "目的",
    "たい",
    "should",
    "must",
    "implement",
    "実装",
    "作る",
    "作成",
    "define",
    "定義",
];

const QUESTION_KEYWORDS: &[&str] = &["how", "what", "why", "where", "when", "どう", "何", "なぜ"];

const FOCUS_KEYWORDS: &[&str] = &[
    "about",
    "focus on",
    "concerning",
    "について",
    "関連",
    "対象",
];

const CONSTRAINT_KEYWORDS: &[&str] = &[
    "must", "cannot", "without", "only", "必要", "必須", "禁止", "制約", "条件",
];

const AMBIGUITY_KEYWORDS: &[&str] = &[
    "maybe",
    "perhaps",
    "or",
    "might",
    "かもしれない",
    "かも",
    "不明",
];

const DECISION_KEYWORDS: &[&str] = &["decide", "choose", "選択", "決定", "決めた"];

/// Extract signals from a resolved conversation.
///
/// If `llm` is `Some`, uses a small LLM call for structured extraction.
/// Otherwise, falls back to rule-based keyword matching.
pub async fn extract(
    resolved: &ResolvedConversation,
    llm: Option<&dyn ResponseModel>,
) -> Result<Vec<Signal>> {
    // Try LLM path first, fall back to rules
    if let Some(llm) = llm
        && let Ok(signals) = extract_with_llm(resolved, llm).await
    {
        return Ok(signals);
    }

    Ok(extract_with_rules(resolved))
}

/// Rule-based signal extraction using keyword matching.
fn extract_with_rules(resolved: &ResolvedConversation) -> Vec<Signal> {
    let mut signals = Vec::new();

    for utterance in &resolved.utterances {
        let text = &utterance.resolved_text;
        let lower = text.to_lowercase();

        // Question detection: ends with ? or ？
        if text.ends_with('?') || text.ends_with('？') {
            signals.push(Signal {
                kind: SignalKind::Question,
                value: text.clone(),
                source_index: utterance.index,
                confidence: 0.7,
            });
        }
        // Also check for question keywords
        else if QUESTION_KEYWORDS.iter().any(|k| lower.contains(k)) {
            signals.push(Signal {
                kind: SignalKind::Question,
                value: text.clone(),
                source_index: utterance.index,
                confidence: 0.5,
            });
        }

        // Goal signals
        if GOAL_KEYWORDS.iter().any(|k| lower.contains(k)) {
            signals.push(Signal {
                kind: SignalKind::Goal,
                value: text.clone(),
                source_index: utterance.index,
                confidence: 0.6,
            });
        }

        // Focus signals
        if FOCUS_KEYWORDS.iter().any(|k| lower.contains(k)) {
            signals.push(Signal {
                kind: SignalKind::Focus,
                value: text.clone(),
                source_index: utterance.index,
                confidence: 0.6,
            });
        }

        // Constraint signals
        if CONSTRAINT_KEYWORDS.iter().any(|k| lower.contains(k)) {
            signals.push(Signal {
                kind: SignalKind::Constraint,
                value: text.clone(),
                source_index: utterance.index,
                confidence: 0.6,
            });
        }

        // Ambiguity signals
        if AMBIGUITY_KEYWORDS.iter().any(|k| lower.contains(k)) {
            signals.push(Signal {
                kind: SignalKind::Ambiguity,
                value: text.clone(),
                source_index: utterance.index,
                confidence: 0.5,
            });
        }

        // Decision signals
        if DECISION_KEYWORDS.iter().any(|k| lower.contains(k)) {
            signals.push(Signal {
                kind: SignalKind::Decision,
                value: text.clone(),
                source_index: utterance.index,
                confidence: 0.6,
            });
        }

        // Artifact: detect CamelCase words or terms in backticks
        for word in text.split_whitespace() {
            let is_camel = word.chars().next().is_some_and(|c| c.is_uppercase())
                && word.chars().any(|c| c.is_lowercase())
                && word.len() > 3;
            let is_backticked = word.starts_with('`') && word.ends_with('`');
            if is_camel || is_backticked {
                let artifact = word.trim_matches('`').to_string();
                // Avoid duplicates for the same utterance
                let already = signals.iter().any(|s| {
                    s.kind == SignalKind::Artifact
                        && s.source_index == utterance.index
                        && s.value.contains(&artifact)
                });
                if !already {
                    signals.push(Signal {
                        kind: SignalKind::Artifact,
                        value: artifact,
                        source_index: utterance.index,
                        confidence: 0.5,
                    });
                }
            }
        }

        // Stage detection: look for process-related terms
        let stage_terms = [
            "design",
            "implement",
            "test",
            "deploy",
            "review",
            "設計",
            "実装",
            "テスト",
        ];
        if stage_terms.iter().any(|t| lower.contains(t)) {
            signals.push(Signal {
                kind: SignalKind::Stage,
                value: text.clone(),
                source_index: utterance.index,
                confidence: 0.5,
            });
        }
    }

    // If we have a resolved focus, add it as a Focus signal
    if let Some(focus) = &resolved.resolved_focus {
        let already = signals
            .iter()
            .any(|s| s.kind == SignalKind::Focus && s.value.contains(focus.as_str()));
        if !already {
            signals.push(Signal {
                kind: SignalKind::Focus,
                value: focus.clone(),
                source_index: 0,
                confidence: 0.8,
            });
        }
    }

    signals
}

/// LLM-based signal extraction.
async fn extract_with_llm(
    resolved: &ResolvedConversation,
    llm: &dyn ResponseModel,
) -> Result<Vec<Signal>> {
    let prompt = build_signal_prompt(resolved);

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

    let llm_signals: Vec<LlmSignal> = parse_json_array(text)?;

    Ok(llm_signals
        .into_iter()
        .filter_map(|s| {
            let kind = match s.kind.as_str() {
                "goal" => Some(SignalKind::Goal),
                "focus" => Some(SignalKind::Focus),
                "artifact" => Some(SignalKind::Artifact),
                "stage" => Some(SignalKind::Stage),
                "question" => Some(SignalKind::Question),
                "constraint" => Some(SignalKind::Constraint),
                "decision" => Some(SignalKind::Decision),
                "ambiguity" => Some(SignalKind::Ambiguity),
                _ => None,
            }?;
            Some(Signal {
                kind,
                value: s.value,
                source_index: s.utterance_index,
                confidence: s.confidence,
            })
        })
        .collect())
}

/// Build the LLM prompt for signal extraction.
fn build_signal_prompt(resolved: &ResolvedConversation) -> Vec<InputItem> {
    let text = resolved
        .utterances
        .iter()
        .enumerate()
        .map(|(i, u)| format!("[{i}] {}", u.resolved_text))
        .collect::<Vec<_>>()
        .join("\n");

    let system = InputItem::system(
        "You are a signal extractor. Identify signals in the conversation that \
         indicate goals, focus areas, artifacts, process stages, questions, \
         constraints, decisions, or ambiguities. \
         Respond with ONLY a valid JSON array, no other text.",
    );

    let user = InputItem::user(format!(
        "Conversation:\n{text}\n\n\
         Signal kinds: goal, focus, artifact, stage, question, constraint, decision, ambiguity\n\n\
         Output format: JSON array of {{\"kind\": str, \"value\": str, \
         \"utterance_index\": int, \"confidence\": float}}. If no signals found, return []."
    ));

    vec![system, user]
}

/// Parse a JSON array from text (reused pattern from resolve).
fn parse_json_array<T: serde::de::DeserializeOwned>(text: &str) -> Result<Vec<T>> {
    if let Ok(parsed) = serde_json::from_str::<Vec<T>>(text) {
        return Ok(parsed);
    }
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
struct LlmSignal {
    kind: String,
    value: String,
    utterance_index: usize,
    confidence: f64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::extractor::types::ResolvedUtterance;

    fn make_resolved(texts: &[&str]) -> ResolvedConversation {
        ResolvedConversation {
            utterances: texts
                .iter()
                .enumerate()
                .map(|(i, t)| ResolvedUtterance {
                    index: i,
                    resolved_text: t.to_string(),
                    original_text: t.to_string(),
                })
                .collect(),
            resolved_refs: Vec::new(),
            resolved_focus: None,
        }
    }

    #[test]
    fn test_extracts_question_from_question_mark() {
        let resolved = make_resolved(&["Situation Extractorをどう実装する？"]);
        let signals = extract_with_rules(&resolved);
        assert!(signals.iter().any(|s| s.kind == SignalKind::Question));
    }

    #[test]
    fn test_extracts_goal_signal() {
        let resolved = make_resolved(&["Situation Extractorを実装したい"]);
        let signals = extract_with_rules(&resolved);
        assert!(signals.iter().any(|s| s.kind == SignalKind::Goal));
    }

    #[test]
    fn test_extracts_artifact_from_camelcase() {
        let resolved = make_resolved(&["Situation Extractorについて"]);
        let signals = extract_with_rules(&resolved);
        assert!(signals.iter().any(|s| s.kind == SignalKind::Artifact));
    }

    #[test]
    fn test_extracts_constraint_signal() {
        let resolved = make_resolved(&["必須でエラーハンドリングが必要"]);
        let signals = extract_with_rules(&resolved);
        assert!(signals.iter().any(|s| s.kind == SignalKind::Constraint));
    }

    #[test]
    fn test_no_signals_from_empty() {
        let resolved = make_resolved(&[""]);
        let signals = extract_with_rules(&resolved);
        assert!(signals.is_empty());
    }

    #[tokio::test]
    async fn test_llm_extracts_with_mock() {
        use crate::backends::MockBackend;

        let mock = MockBackend::new(
            r#"[{"kind":"question","value":"How to implement?","utterance_index":0,"confidence":0.9}]"#,
        );

        let resolved = make_resolved(&["How should we implement this?"]);
        let signals = extract(&resolved, Some(&mock)).await.unwrap();

        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].kind, SignalKind::Question);
        assert!((signals[0].confidence - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_resolved_focus_adds_focus_signal() {
        let mut resolved = make_resolved(&["パイプラインの設計"]);
        resolved.resolved_focus = Some("situation_extractor".to_string());
        let signals = extract_with_rules(&resolved);
        assert!(
            signals
                .iter()
                .any(|s| s.kind == SignalKind::Focus && s.value == "situation_extractor")
        );
    }
}
