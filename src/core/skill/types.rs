use std::collections::HashMap;

use serde::Deserialize;

/// SKILL.md frontmatter (agentskills.io specification compliant).
///
/// See: <https://agentskills.io/specification>
#[derive(Debug, Clone, Deserialize)]
pub struct SkillManifest {
    /// Skill name (1–64 chars, lowercase alphanumeric + hyphens, no
    /// leading/trailing/consecutive hyphens). Must match the parent directory name.
    pub name: String,

    /// What the skill does and when to use it (1–1024 chars).
    pub description: String,

    /// Optional license name or reference to a bundled license file.
    #[serde(default)]
    pub license: Option<String>,

    /// Optional environment requirements (max 500 chars).
    #[serde(default)]
    pub compatibility: Option<String>,

    /// Optional arbitrary key-value metadata.
    #[serde(default)]
    pub metadata: Option<HashMap<String, String>>,
}

/// A fully loaded skill: manifest + instruction body.
pub struct Skill {
    pub manifest: SkillManifest,
    /// The Markdown body after the YAML frontmatter in SKILL.md.
    pub instructions: String,
}
