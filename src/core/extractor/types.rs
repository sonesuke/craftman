use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

/// Raw input to the extractor pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractorInput {
    /// The recent conversation text to analyze.
    pub conversation: String,
    /// Optional preceding context (earlier turns) for reference resolution.
    pub context: Vec<String>,
}

// ---------------------------------------------------------------------------
// Stage 1: Normalize
// ---------------------------------------------------------------------------

/// Output of the normalization stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedConversation {
    /// Individual utterances, split from the raw conversation.
    pub utterances: Vec<Utterance>,
    /// Detected language code (e.g. "ja", "en").
    pub language: String,
}

/// A single utterance within a normalized conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Utterance {
    /// 0-based index in the conversation.
    pub index: usize,
    /// The utterance text with decorations stripped but structure preserved.
    pub text: String,
    /// Special content blocks detected (quotes, code, lists).
    pub preserved_blocks: Vec<PreservedBlock>,
    /// Normalized term annotations found in this utterance.
    pub term_annotations: Vec<TermAnnotation>,
}

/// A preserved structural block within an utterance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PreservedBlock {
    Quote {
        content: String,
    },
    CodeBlock {
        language: Option<String>,
        content: String,
    },
    BulletList {
        items: Vec<String>,
    },
}

/// A normalized term annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermAnnotation {
    /// The original surface form (e.g., "INとOUT").
    pub original: String,
    /// The normalized form (e.g., "interface_schema").
    pub normalized: String,
}

// ---------------------------------------------------------------------------
// Stage 2: Reference Resolve
// ---------------------------------------------------------------------------

/// Output of the reference resolution stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedConversation {
    /// Utterances with references resolved.
    pub utterances: Vec<ResolvedUtterance>,
    /// All resolved references found.
    pub resolved_refs: Vec<ResolvedRef>,
    /// The primary focus after resolution.
    pub resolved_focus: Option<String>,
}

/// An utterance with resolved references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedUtterance {
    pub index: usize,
    /// The utterance text with anaphora/ellipsis resolved.
    pub resolved_text: String,
    /// The original normalized text (before resolution).
    pub original_text: String,
}

/// A resolved reference (anaphora resolution result).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedRef {
    /// The anaphoric expression (e.g., "それ").
    pub expression: String,
    /// What it was resolved to (e.g., "Situation Extractor").
    pub referent: String,
    /// Confidence of the resolution (0.0–1.0).
    pub confidence: f64,
    /// Utterance index where the reference appeared.
    pub utterance_index: usize,
}

// ---------------------------------------------------------------------------
// Stage 3: Signal Extract
// ---------------------------------------------------------------------------

/// Kinds of signals that can be extracted from conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    Goal,
    Focus,
    Artifact,
    Stage,
    Question,
    Constraint,
    Decision,
    Ambiguity,
}

/// A signal extracted from conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub kind: SignalKind,
    /// The signal text/value.
    pub value: String,
    /// 0-based utterance index where the signal was found.
    pub source_index: usize,
    /// Confidence from extraction (0.0–1.0).
    pub confidence: f64,
}

// ---------------------------------------------------------------------------
// Stage 4: Situation Compose
// ---------------------------------------------------------------------------

/// Domain classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    SoftwareEngineering,
    DataProcessing,
    UserInterface,
    DevOps,
    General,
}

/// Task type classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Implementation,
    Debugging,
    Design,
    Analysis,
    Refactoring,
    Question,
}

/// Problem type classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemType {
    HowToImplement,
    WhatIsWrong,
    ArchitectureDecision,
    Clarification,
    Exploration,
}

/// A composed situation from signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Situation {
    pub domain: Domain,
    pub component: Option<String>,
    pub task_type: TaskType,
    pub problem_type: ProblemType,
    pub stage: Option<String>,
    pub ambiguity: Vec<String>,
}

// ---------------------------------------------------------------------------
// Stage 5: Hypothesis Rank
// ---------------------------------------------------------------------------

/// A scored situation hypothesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub situation: Situation,
    /// Overall confidence score (0.0–1.0), sorted descending.
    pub confidence: f64,
    /// Breakdown of scoring factors.
    pub score_breakdown: ScoreBreakdown,
}

/// Breakdown of scoring factors for a hypothesis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub explicit_keyword_bonus: f64,
    pub recent_context_match_bonus: f64,
    pub resolved_reference_bonus: f64,
    pub competing_candidates_penalty: f64,
    pub unresolved_ref_penalty: f64,
}

// ---------------------------------------------------------------------------
// Extraction Notes
// ---------------------------------------------------------------------------

/// Uncertainty and metadata notes from the extraction process.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionNotes {
    pub requires_clarification: Vec<String>,
    pub missing_information: Vec<String>,
    pub resolved_references: Vec<String>,
    pub uncertainties: Vec<String>,
}

// ---------------------------------------------------------------------------
// Final Output
// ---------------------------------------------------------------------------

/// The complete output of the Situation Extractor pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionOutput {
    pub hypotheses: Vec<Hypothesis>,
    pub extraction_notes: ExtractionNotes,
}
