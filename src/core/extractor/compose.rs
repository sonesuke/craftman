use anyhow::Result;

use super::types::{
    Domain, ProblemType, ResolvedConversation, Signal, SignalKind, Situation, TaskType,
};

/// Compose signals into a `Situation` struct using rule-based taxonomy mapping.
pub fn compose(signals: &[Signal], resolved: &ResolvedConversation) -> Result<Situation> {
    let domain = determine_domain(signals);
    let component = determine_component(signals, resolved);
    let task_type = determine_task_type(signals);
    let problem_type = determine_problem_type(signals, &task_type);
    let stage = determine_stage(signals);
    let ambiguity = collect_ambiguity(signals, resolved);

    Ok(Situation {
        domain,
        component,
        task_type,
        problem_type,
        stage,
        ambiguity,
    })
}

/// Determine the domain from signals.
fn determine_domain(signals: &[Signal]) -> Domain {
    let text = all_signal_text(signals);

    if contains_any(
        &text,
        &[
            "pipeline",
            "extractor",
            "module",
            "component",
            "パイプライン",
            "モジュール",
            "コンポーネント",
        ],
    ) || has_artifact_matching(signals, |v| {
        v.contains("Extractor")
            || v.contains("Retriever")
            || v.contains("Registry")
            || v.contains("Backend")
    }) {
        return Domain::SoftwareEngineering;
    }

    if contains_any(
        &text,
        &["database", "data", "processing", "データ", "処理", "ETL"],
    ) {
        return Domain::DataProcessing;
    }

    if contains_any(
        &text,
        &["UI", "frontend", "button", "画面", "UI", "フロントエンド"],
    ) {
        return Domain::UserInterface;
    }

    if contains_any(
        &text,
        &[
            "deploy",
            "CI",
            "CD",
            "Docker",
            "infra",
            "デプロイ",
            "インフラ",
        ],
    ) {
        return Domain::DevOps;
    }

    Domain::General
}

/// Determine the component from signals.
fn determine_component(signals: &[Signal], resolved: &ResolvedConversation) -> Option<String> {
    // Prefer explicit Artifact signals
    if let Some(artifact) = signals.iter().find(|s| s.kind == SignalKind::Artifact) {
        return Some(artifact.value.clone());
    }

    // Fall back to resolved focus
    resolved.resolved_focus.clone()
}

/// Determine the task type from signals.
fn determine_task_type(signals: &[Signal]) -> TaskType {
    // Question signals take priority
    if signals.iter().any(|s| s.kind == SignalKind::Question) {
        return TaskType::Question;
    }

    let text = all_signal_text(signals);

    if contains_any(
        &text,
        &["implement", "build", "create", "実装", "作る", "作成"],
    ) {
        return TaskType::Implementation;
    }

    if contains_any(
        &text,
        &["debug", "fix", "error", "デバッグ", "修正", "エラー"],
    ) {
        return TaskType::Debugging;
    }

    if contains_any(
        &text,
        &[
            "design",
            "architect",
            "plan",
            "設計",
            "アーキテクチャ",
            "計画",
        ],
    ) {
        return TaskType::Design;
    }

    if contains_any(&text, &["analyze", "review", "分析", "レビュー"]) {
        return TaskType::Analysis;
    }

    if contains_any(&text, &["refactor", "clean", "リファクタ", "整理"]) {
        return TaskType::Refactoring;
    }

    TaskType::Question
}

/// Determine the problem type from signals and task type.
fn determine_problem_type(signals: &[Signal], task_type: &TaskType) -> ProblemType {
    match task_type {
        TaskType::Question => ProblemType::Clarification,
        TaskType::Debugging => ProblemType::WhatIsWrong,
        TaskType::Design => ProblemType::ArchitectureDecision,
        _ => {
            let text = all_signal_text(signals);
            if contains_any(&text, &["how", "どう", "方法"]) {
                ProblemType::HowToImplement
            } else {
                ProblemType::Exploration
            }
        }
    }
}

/// Determine the development stage from signals.
fn determine_stage(signals: &[Signal]) -> Option<String> {
    signals
        .iter()
        .find(|s| s.kind == SignalKind::Stage)
        .map(|s| {
            let lower = s.value.to_lowercase();
            if lower.contains("design") || lower.contains("設計") {
                "design"
            } else if lower.contains("implement") || lower.contains("実装") {
                "implementation"
            } else if lower.contains("test") || lower.contains("テスト") {
                "testing"
            } else if lower.contains("deploy") || lower.contains("デプロイ") {
                "deployment"
            } else if lower.contains("review") || lower.contains("レビュー") {
                "review"
            } else {
                "unknown"
            }
            .to_string()
        })
}

/// Collect ambiguity from signals and unresolved references.
fn collect_ambiguity(signals: &[Signal], resolved: &ResolvedConversation) -> Vec<String> {
    let mut ambiguity = Vec::new();

    for signal in signals {
        if signal.kind == SignalKind::Ambiguity {
            ambiguity.push(signal.value.clone());
        }
    }

    // Low-confidence resolved refs count as ambiguity
    for r#ref in &resolved.resolved_refs {
        if r#ref.confidence < 0.5 {
            ambiguity.push(format!("Unresolved reference: {}", r#ref.expression));
        }
    }

    ambiguity
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn all_signal_text(signals: &[Signal]) -> String {
    signals
        .iter()
        .map(|s| s.value.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|k| text.contains(k))
}

fn has_artifact_matching(signals: &[Signal], pred: impl Fn(&str) -> bool) -> bool {
    signals
        .iter()
        .filter(|s| s.kind == SignalKind::Artifact)
        .any(|s| pred(&s.value))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::extractor::types::ResolvedRef;

    fn make_resolved() -> ResolvedConversation {
        ResolvedConversation {
            utterances: vec![],
            resolved_refs: Vec::new(),
            resolved_focus: None,
        }
    }

    fn make_signal(kind: SignalKind, value: &str) -> Signal {
        Signal {
            kind,
            value: value.to_string(),
            source_index: 0,
            confidence: 0.7,
        }
    }

    #[test]
    fn test_software_engineering_domain() {
        let signals = vec![make_signal(SignalKind::Artifact, "Situation Extractor")];
        let situation = compose(&signals, &make_resolved()).unwrap();
        assert_eq!(situation.domain, Domain::SoftwareEngineering);
    }

    #[test]
    fn test_general_domain() {
        let signals = vec![make_signal(SignalKind::Question, "What time is it?")];
        let situation = compose(&signals, &make_resolved()).unwrap();
        assert_eq!(situation.domain, Domain::General);
    }

    #[test]
    fn test_question_task_type() {
        let signals = vec![make_signal(SignalKind::Question, "How does it work?")];
        let situation = compose(&signals, &make_resolved()).unwrap();
        assert_eq!(situation.task_type, TaskType::Question);
    }

    #[test]
    fn test_design_task_type() {
        let signals = vec![make_signal(
            SignalKind::Goal,
            "Situation Extractorの設計をする",
        )];
        let situation = compose(&signals, &make_resolved()).unwrap();
        assert_eq!(situation.task_type, TaskType::Design);
    }

    #[test]
    fn test_component_from_artifact() {
        let signals = vec![make_signal(SignalKind::Artifact, "Pipeline")];
        let situation = compose(&signals, &make_resolved()).unwrap();
        assert_eq!(situation.component, Some("Pipeline".to_string()));
    }

    #[test]
    fn test_component_from_resolved_focus() {
        let signals = vec![];
        let mut resolved = make_resolved();
        resolved.resolved_focus = Some("extractor".to_string());
        let situation = compose(&signals, &resolved).unwrap();
        assert_eq!(situation.component, Some("extractor".to_string()));
    }

    #[test]
    fn test_ambiguity_from_signals() {
        let signals = vec![make_signal(SignalKind::Ambiguity, "Maybe this or that")];
        let situation = compose(&signals, &make_resolved()).unwrap();
        assert_eq!(situation.ambiguity.len(), 1);
    }

    #[test]
    fn test_ambiguity_from_low_confidence_ref() {
        let signals = vec![];
        let mut resolved = make_resolved();
        resolved.resolved_refs.push(ResolvedRef {
            expression: "それ".to_string(),
            referent: "???".to_string(),
            confidence: 0.3,
            utterance_index: 0,
        });
        let situation = compose(&signals, &resolved).unwrap();
        assert_eq!(situation.ambiguity.len(), 1);
    }

    #[test]
    fn test_stage_extraction() {
        let signals = vec![make_signal(
            SignalKind::Stage,
            "設計段階でパイプラインを定義する",
        )];
        let situation = compose(&signals, &make_resolved()).unwrap();
        assert_eq!(situation.stage, Some("design".to_string()));
    }
}
