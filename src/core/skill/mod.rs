pub mod loader;
pub mod types;

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};

pub use loader::{load_skill, load_skills_from_dir};
pub use types::{Skill, SkillManifest};

use crate::core::llm::ToolDefinition;

/// A search result from [`SkillRegistry::search`].
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub description: String,
    pub score: f64,
}

/// Registry of loaded skill definitions.
///
/// Implements the Agent Skills progressive disclosure pattern:
/// 1. **Discovery**: skill names + descriptions loaded at startup
/// 2. **Search**: `search_skills` tool finds relevant skills by keyword
/// 3. **Activation**: `load_skill` tool returns full instructions on demand
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Load skills from a directory (discovery phase).
    pub fn load_from_dir(&mut self, dir: &Path) -> Result<()> {
        let skills = loader::load_skills_from_dir(dir)?;
        for skill in skills {
            let name = skill.manifest.name.clone();
            self.skills.insert(name, skill);
        }
        Ok(())
    }

    /// Return the `search_skills` tool definition for the LLM.
    pub fn search_skills_tool_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_skills".to_string(),
            description: "Search for relevant skills by keywords. Returns matching skill names and descriptions. Use this to discover available skills before loading them with load_skill.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query describing what you need, e.g. 'calculate math', 'translate', 'code review'"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    /// Return the `load_skill` tool definition for the LLM.
    ///
    /// Does not list skills — the LLM should use `search_skills` first.
    pub fn load_skill_tool_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "load_skill".to_string(),
            description: "Load a skill's full instructions into context by its exact name. Use search_skills first to find the right skill.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Exact name of the skill to load"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    /// Search for skills matching the given query by keyword.
    ///
    /// Tokenises the query into words, then scores each skill by how many
    /// query words appear in its name or description (case-insensitive).
    /// Returns up to `top_k` results sorted by score descending.
    pub fn search(&self, query: &str, top_k: usize) -> Vec<SearchResult> {
        let lower = query.to_lowercase();
        let query_words: Vec<&str> = lower.split_whitespace().filter(|w| !w.is_empty()).collect();

        if query_words.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<SearchResult> = self
            .skills
            .values()
            .filter_map(|skill| {
                let haystack = format!("{} {}", skill.manifest.name, skill.manifest.description)
                    .to_lowercase();

                let matches = query_words.iter().filter(|w| haystack.contains(*w)).count();

                if matches == 0 {
                    return None;
                }

                let score = matches as f64 / query_words.len() as f64;

                Some(SearchResult {
                    name: skill.manifest.name.clone(),
                    description: skill.manifest.description.clone(),
                    score,
                })
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);
        results
    }

    /// Activate a skill by name, returning its full instructions.
    pub fn activate(&self, name: &str) -> Result<&str> {
        let skill = self
            .skills
            .get(name)
            .ok_or_else(|| anyhow!("Unknown skill: {name}"))?;
        Ok(&skill.instructions)
    }

    /// List loaded skill names.
    pub fn skill_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.skills.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Check if a skill is registered.
    pub fn has_skill(&self, name: &str) -> bool {
        self.skills.contains_key(name)
    }
}
