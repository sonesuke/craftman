pub mod compose;
pub mod normalize;
pub mod rank;
pub mod resolve;
pub mod signal;
pub mod types;

use std::sync::Arc;

use anyhow::Result;

use crate::core::llm::ResponseModel;

pub use types::*;

/// The top-level Situation Extractor.
///
/// Owns an optional LLM backend and orchestrates the five pipeline stages:
///
/// ```text
/// INPUT → Normalize → Reference Resolve → Signal Extract →
///         Situation Compose → Hypothesis Rank → OUTPUT
/// ```
pub struct SituationExtractor {
    llm: Option<Arc<dyn ResponseModel>>,
}

impl SituationExtractor {
    /// Create with an LLM backend for stages 2 (resolve) and 3 (signal extract).
    pub fn new(llm: Arc<dyn ResponseModel>) -> Self {
        Self { llm: Some(llm) }
    }

    /// Create without any LLM — all stages run rule-based.
    pub fn rule_based() -> Self {
        Self { llm: None }
    }

    /// Run the full extraction pipeline on the given input.
    pub async fn extract(&self, input: ExtractorInput) -> Result<ExtractionOutput> {
        // Stage 1: Normalize (rule-based)
        let normalized = normalize::normalize(&input)?;

        // Stage 2: Reference Resolve (rule-based + optional LLM)
        let resolved = resolve::resolve(&normalized, &input, self.llm.as_deref()).await?;

        // Stage 3: Signal Extract (rule-based + optional LLM)
        let signals = signal::extract(&resolved, self.llm.as_deref()).await?;

        // Stage 4: Situation Compose (rule-based)
        let situation = compose::compose(&signals, &resolved)?;

        // Stage 5: Hypothesis Rank (rule-based)
        let hypotheses = rank::rank(situation, &signals, &resolved);

        // Build extraction notes
        let notes = build_extraction_notes(&resolved, &signals, &hypotheses);

        Ok(ExtractionOutput {
            hypotheses,
            extraction_notes: notes,
        })
    }
}

/// Build extraction notes summarizing uncertainties and metadata.
fn build_extraction_notes(
    resolved: &types::ResolvedConversation,
    signals: &[types::Signal],
    hypotheses: &[types::Hypothesis],
) -> ExtractionNotes {
    let mut notes = ExtractionNotes::default();

    // Low-confidence hypotheses require clarification
    for h in hypotheses {
        if h.confidence < 0.4 {
            notes.requires_clarification.push(format!(
                "Low confidence ({:.2}): {:?}",
                h.confidence, h.situation
            ));
        }
    }

    // Ambiguity signals → uncertainties
    for signal in signals {
        if signal.kind == types::SignalKind::Ambiguity {
            notes.uncertainties.push(signal.value.clone());
        }
    }

    // Low-confidence resolved refs → missing information
    for r#ref in &resolved.resolved_refs {
        if r#ref.confidence < 0.5 {
            notes.missing_information.push(format!(
                "Unresolved reference: '{}' may not refer to '{}'",
                r#ref.expression, r#ref.referent
            ));
        } else {
            notes
                .resolved_references
                .push(format!("{} → {}", r#ref.expression, r#ref.referent));
        }
    }

    // Low-confidence signals → uncertainties
    for signal in signals {
        if signal.confidence < 0.5 {
            notes.uncertainties.push(format!(
                "Low confidence signal: {:?} = {}",
                signal.kind, signal.value
            ));
        }
    }

    notes
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::extractor::types::{ResolvedRef, ResolvedUtterance, Signal, SignalKind};

    #[test]
    fn test_build_extraction_notes_high_confidence() {
        let resolved = types::ResolvedConversation {
            utterances: vec![ResolvedUtterance {
                index: 0,
                resolved_text: "test".to_string(),
                original_text: "test".to_string(),
            }],
            resolved_refs: vec![ResolvedRef {
                expression: "それ".to_string(),
                referent: "Pipeline".to_string(),
                confidence: 0.9,
                utterance_index: 0,
            }],
            resolved_focus: Some("Pipeline".to_string()),
        };
        let signals = vec![Signal {
            kind: SignalKind::Goal,
            value: "implement pipeline".to_string(),
            source_index: 0,
            confidence: 0.8,
        }];
        let hypotheses = vec![types::Hypothesis {
            situation: types::Situation {
                domain: types::Domain::SoftwareEngineering,
                component: Some("Pipeline".to_string()),
                task_type: types::TaskType::Implementation,
                problem_type: types::ProblemType::HowToImplement,
                stage: None,
                ambiguity: vec![],
            },
            confidence: 0.9,
            score_breakdown: types::ScoreBreakdown::default(),
        }];

        let notes = build_extraction_notes(&resolved, &signals, &hypotheses);

        assert!(notes.requires_clarification.is_empty());
        assert!(notes.resolved_references.len() == 1);
    }

    #[test]
    fn test_build_extraction_notes_low_confidence() {
        let resolved = types::ResolvedConversation {
            utterances: vec![],
            resolved_refs: vec![ResolvedRef {
                expression: "それ".to_string(),
                referent: "???".to_string(),
                confidence: 0.2,
                utterance_index: 0,
            }],
            resolved_focus: None,
        };
        let signals = vec![Signal {
            kind: SignalKind::Ambiguity,
            value: "maybe this or that".to_string(),
            source_index: 0,
            confidence: 0.3,
        }];
        let hypotheses = vec![types::Hypothesis {
            situation: types::Situation {
                domain: types::Domain::General,
                component: None,
                task_type: types::TaskType::Question,
                problem_type: types::ProblemType::Clarification,
                stage: None,
                ambiguity: vec![],
            },
            confidence: 0.3,
            score_breakdown: types::ScoreBreakdown::default(),
        }];

        let notes = build_extraction_notes(&resolved, &signals, &hypotheses);

        assert!(!notes.requires_clarification.is_empty());
        assert!(!notes.missing_information.is_empty());
        assert!(!notes.uncertainties.is_empty());
    }
}
