pub mod loader;
pub mod types;

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};

pub use loader::{load_skill, load_skills_from_dir};
pub use types::{Skill, SkillManifest};

use crate::core::llm::ToolDefinition;
use crate::core::retriever::Retriever;

/// A search result from [`SkillRegistry::retrieve`].
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub description: String,
    pub score: f64,
}

/// Registry of loaded skill definitions.
///
/// Implements a ToolRAG-style retrieval flow (inspired by TinyAgent):
/// 1. **Discovery**: skill names + descriptions loaded at startup
/// 2. **Retrieve**: [`SkillRegistry::retrieve`] ranks skills against the
///    current query with BM25 so only the relevant subset is surfaced
/// 3. **Activation**: the `load_skill` tool returns full instructions on demand
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
    /// BM25 index over `(name, description)`, kept in sync with `skills`.
    retriever: Retriever,
    /// Skill names in the same order they were passed to `retriever`, so a
    /// ranked index can be mapped back to a skill.
    retrieval_order: Vec<String>,
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
            retriever: Retriever::from_docs(&[]),
            retrieval_order: Vec::new(),
        }
    }

    /// Load skills from a directory (discovery phase).
    pub fn load_from_dir(&mut self, dir: &Path) -> Result<()> {
        let skills = loader::load_skills_from_dir(dir)?;
        for skill in skills {
            let name = skill.manifest.name.clone();
            self.skills.insert(name, skill);
        }
        self.rebuild_index();
        Ok(())
    }

    /// Rebuild the BM25 index from the currently loaded skills.
    fn rebuild_index(&mut self) {
        // Collect owned copies first so we don't hold an immutable borrow of
        // `self.skills` while assigning to `self` below.
        let owned: Vec<(String, String)> = self
            .skills
            .values()
            .map(|s| (s.manifest.name.clone(), s.manifest.description.clone()))
            .collect();
        let order: Vec<String> = owned.iter().map(|(n, _)| n.clone()).collect();
        let docs: Vec<(&str, &str)> = owned
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_str()))
            .collect();
        self.retriever = Retriever::from_docs(&docs);
        self.retrieval_order = order;
    }

    /// Return the `load_skill` tool definition for the LLM.
    ///
    /// Relevant skills are surfaced automatically each turn (ToolRAG), so the
    /// model uses this only to activate a skill it has been shown.
    pub fn load_skill_tool_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "load_skill".to_string(),
            description: "Load a skill's full instructions into context by its exact name. \
                Relevant skills are already shown to you each turn; use this to activate the one you need."
                .to_string(),
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

    /// Retrieve the `top_k` skills most relevant to `query` using BM25.
    ///
    /// Scores each skill's `name` (weighted higher) and `description` against
    /// the query and returns matches with a positive score, best first.
    pub fn retrieve(&self, query: &str, top_k: usize) -> Vec<SearchResult> {
        self.retriever
            .search(query, top_k)
            .into_iter()
            .filter_map(|hit| {
                let name = self.retrieval_order.get(hit.index)?;
                let skill = self.skills.get(name)?;
                Some(SearchResult {
                    name: skill.manifest.name.clone(),
                    description: skill.manifest.description.clone(),
                    score: hit.score,
                })
            })
            .collect()
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
