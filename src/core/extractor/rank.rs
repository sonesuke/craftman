use super::types::{
    Domain, Hypothesis, ProblemType, ResolvedConversation, ScoreBreakdown, Signal, SignalKind,
    Situation, TaskType,
};

/// Base confidence score before adjustments.
const BASE_SCORE: f64 = 0.5;

/// Score and rank situation hypotheses.
///
/// Produces the primary hypothesis from the composed situation,
/// and optionally generates alternative hypotheses if ambiguity exists.
pub fn rank(
    situation: Situation,
    signals: &[Signal],
    resolved: &ResolvedConversation,
) -> Vec<Hypothesis> {
    let mut hypotheses = Vec::new();

    // Primary hypothesis
    let primary_score = score_hypothesis(&situation, signals, resolved);
    hypotheses.push(Hypothesis {
        situation,
        confidence: (BASE_SCORE + net_score(&primary_score)).clamp(0.0, 1.0),
        score_breakdown: primary_score,
    });

    // Generate alternative hypotheses from ambiguity
    let alt_situations = generate_alternatives(&hypotheses[0].situation, signals);
    for alt in alt_situations {
        let alt_score = score_hypothesis(&alt, signals, resolved);
        hypotheses.push(Hypothesis {
            situation: alt,
            confidence: (BASE_SCORE + net_score(&alt_score)).clamp(0.0, 1.0),
            score_breakdown: alt_score,
        });
    }

    // Sort by confidence descending
    hypotheses.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    hypotheses
}

/// Score a single hypothesis based on the scoring rules.
fn score_hypothesis(
    situation: &Situation,
    signals: &[Signal],
    resolved: &ResolvedConversation,
) -> ScoreBreakdown {
    let explicit_keyword_bonus = calc_explicit_keyword_bonus(situation, signals);
    let recent_context_match_bonus = calc_recent_context_bonus(signals, resolved);
    let resolved_reference_bonus = calc_resolved_ref_bonus(resolved);
    let competing_candidates_penalty = calc_competing_penalty(signals);
    let unresolved_ref_penalty = calc_unresolved_penalty(resolved);

    ScoreBreakdown {
        explicit_keyword_bonus,
        recent_context_match_bonus,
        resolved_reference_bonus,
        competing_candidates_penalty,
        unresolved_ref_penalty,
    }
}

/// Calculate net score from breakdown.
fn net_score(breakdown: &ScoreBreakdown) -> f64 {
    breakdown.explicit_keyword_bonus
        + breakdown.recent_context_match_bonus
        + breakdown.resolved_reference_bonus
        + breakdown.competing_candidates_penalty
        + breakdown.unresolved_ref_penalty
}

/// Explicit keyword: +0.4 if signal value directly contains a taxonomy term.
fn calc_explicit_keyword_bonus(situation: &Situation, signals: &[Signal]) -> f64 {
    let taxonomy_terms = match &situation.component {
        Some(c) => vec![c.to_lowercase()],
        None => vec![],
    };

    // Also check domain-related terms
    let domain_terms = match situation.domain {
        Domain::SoftwareEngineering => vec!["pipeline", "extractor", "module", "component"],
        Domain::DataProcessing => vec!["data", "processing", "etl"],
        Domain::UserInterface => vec!["ui", "frontend", "button"],
        Domain::DevOps => vec!["deploy", "ci", "cd", "docker"],
        Domain::General => vec![],
    };

    for signal in signals {
        let lower = signal.value.to_lowercase();
        if taxonomy_terms.iter().any(|t| lower.contains(t.as_str()))
            || domain_terms.iter().any(|t| lower.contains(t))
        {
            return 0.4;
        }
    }

    0.0
}

/// Recent context match: +0.3 if signal comes from the last 2 utterances.
fn calc_recent_context_bonus(signals: &[Signal], resolved: &ResolvedConversation) -> f64 {
    let total = resolved.utterances.len();
    if total == 0 {
        return 0.0;
    }

    let recent_threshold = total.saturating_sub(2);
    let has_recent = signals.iter().any(|s| s.source_index >= recent_threshold);

    if has_recent { 0.3 } else { 0.0 }
}

/// Resolved reference: +0.1 if a high-confidence resolved ref supports this.
fn calc_resolved_ref_bonus(resolved: &ResolvedConversation) -> f64 {
    let has_confident = resolved.resolved_refs.iter().any(|r| r.confidence > 0.7);

    if has_confident { 0.1 } else { 0.0 }
}

/// Competing candidates: -0.2 if multiple Goal or Focus signals point in different directions.
fn calc_competing_penalty(signals: &[Signal]) -> f64 {
    let goals: Vec<&Signal> = signals
        .iter()
        .filter(|s| s.kind == SignalKind::Goal)
        .collect();
    let focuses: Vec<&Signal> = signals
        .iter()
        .filter(|s| s.kind == SignalKind::Focus)
        .collect();

    // Check if there are multiple goals with different values
    if goals.len() > 1 {
        let unique_values: std::collections::HashSet<&str> =
            goals.iter().map(|g| g.value.as_str()).collect();
        if unique_values.len() > 1 {
            return -0.2;
        }
    }

    if focuses.len() > 1 {
        let unique_values: std::collections::HashSet<&str> =
            focuses.iter().map(|f| f.value.as_str()).collect();
        if unique_values.len() > 1 {
            return -0.2;
        }
    }

    0.0
}

/// Unresolved reference: -0.3 if there are unresolved refs (low confidence).
fn calc_unresolved_penalty(resolved: &ResolvedConversation) -> f64 {
    let has_unresolved = resolved.resolved_refs.iter().any(|r| r.confidence < 0.5);

    if has_unresolved { -0.3 } else { 0.0 }
}

/// Generate alternative hypotheses by varying task_type or problem_type.
fn generate_alternatives(situation: &Situation, signals: &[Signal]) -> Vec<Situation> {
    let mut alternatives = Vec::new();

    // If there are ambiguity signals, create an alternative with different task type
    let has_ambiguity = signals.iter().any(|s| s.kind == SignalKind::Ambiguity);
    if has_ambiguity {
        // Flip task type to the next most likely
        let alt_task = match situation.task_type {
            TaskType::Design => TaskType::Implementation,
            TaskType::Implementation => TaskType::Design,
            TaskType::Question => TaskType::Analysis,
            _ => TaskType::Question,
        };

        let alt_problem = match situation.problem_type {
            ProblemType::HowToImplement => ProblemType::ArchitectureDecision,
            ProblemType::Clarification => ProblemType::Exploration,
            _ => ProblemType::Clarification,
        };

        if alt_task != situation.task_type || alt_problem != situation.problem_type {
            alternatives.push(Situation {
                domain: situation.domain.clone(),
                component: situation.component.clone(),
                task_type: alt_task,
                problem_type: alt_problem,
                stage: situation.stage.clone(),
                ambiguity: situation.ambiguity.clone(),
            });
        }
    }

    alternatives
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::extractor::types::{ResolvedRef, ResolvedUtterance};

    fn make_signal(kind: SignalKind, value: &str, source_index: usize) -> Signal {
        Signal {
            kind,
            value: value.to_string(),
            source_index,
            confidence: 0.7,
        }
    }

    fn make_resolved_with_utterances(count: usize) -> ResolvedConversation {
        ResolvedConversation {
            utterances: (0..count)
                .map(|i| ResolvedUtterance {
                    index: i,
                    resolved_text: format!("Utterance {i}"),
                    original_text: format!("Utterance {i}"),
                })
                .collect(),
            resolved_refs: Vec::new(),
            resolved_focus: None,
        }
    }

    fn make_situation() -> Situation {
        Situation {
            domain: Domain::SoftwareEngineering,
            component: Some("Situation Extractor".to_string()),
            task_type: TaskType::Design,
            problem_type: ProblemType::ArchitectureDecision,
            stage: Some("design".to_string()),
            ambiguity: Vec::new(),
        }
    }

    #[test]
    fn test_primary_hypothesis_base_score() {
        let situation = make_situation();
        let signals = vec![make_signal(SignalKind::Artifact, "Situation Extractor", 0)];
        let resolved = make_resolved_with_utterances(1);
        let hypotheses = rank(situation, &signals, &resolved);

        assert!(!hypotheses.is_empty());
        // Base 0.5 + explicit_keyword 0.4 + recent_context 0.3 + resolved_ref 0.0 = 1.2 → clamped to 1.0
        assert!(hypotheses[0].confidence >= 0.5);
    }

    #[test]
    fn test_sorts_by_confidence_descending() {
        let situation = make_situation();
        // Create ambiguity to trigger alternative hypothesis
        let signals = vec![
            make_signal(SignalKind::Artifact, "Situation Extractor", 0),
            make_signal(SignalKind::Ambiguity, "maybe this or that", 1),
        ];
        let resolved = make_resolved_with_utterances(2);
        let hypotheses = rank(situation, &signals, &resolved);

        // Should be sorted descending
        for i in 1..hypotheses.len() {
            assert!(hypotheses[i - 1].confidence >= hypotheses[i].confidence);
        }
    }

    #[test]
    fn test_penalty_for_unresolved_refs() {
        let situation = make_situation();
        let signals = vec![make_signal(SignalKind::Goal, "implement pipeline", 0)];
        let mut resolved = make_resolved_with_utterances(1);
        resolved.resolved_refs.push(ResolvedRef {
            expression: "それ".to_string(),
            referent: "???".to_string(),
            confidence: 0.2,
            utterance_index: 0,
        });

        let hypotheses = rank(situation, &signals, &resolved);
        assert!(hypotheses[0].score_breakdown.unresolved_ref_penalty < 0.0);
    }

    #[test]
    fn test_bonus_for_explicit_keyword() {
        let situation = make_situation();
        let signals = vec![make_signal(
            SignalKind::Goal,
            "implement Situation Extractor",
            0,
        )];
        let resolved = make_resolved_with_utterances(1);

        let hypotheses = rank(situation, &signals, &resolved);
        assert!(hypotheses[0].score_breakdown.explicit_keyword_bonus > 0.0);
    }

    #[test]
    fn test_no_alternatives_without_ambiguity() {
        let situation = make_situation();
        let signals = vec![make_signal(SignalKind::Artifact, "Extractor", 0)];
        let resolved = make_resolved_with_utterances(1);

        let hypotheses = rank(situation, &signals, &resolved);
        assert_eq!(hypotheses.len(), 1);
    }

    #[test]
    fn test_alternatives_with_ambiguity() {
        let situation = make_situation();
        let signals = vec![
            make_signal(SignalKind::Artifact, "Extractor", 0),
            make_signal(SignalKind::Ambiguity, "maybe", 1),
        ];
        let resolved = make_resolved_with_utterances(2);

        let hypotheses = rank(situation, &signals, &resolved);
        assert!(hypotheses.len() > 1);
    }
}
