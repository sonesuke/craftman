pub mod loader;
pub mod types;

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use async_trait::async_trait;

pub use loader::{load_skill, load_skills_from_dir};
pub use types::{Skill, SkillManifest};

use crate::core::llm::ToolDefinition;

/// A skill executor backed by Rust code.
#[async_trait]
pub trait SkillExecutor: Send + Sync {
    /// The skill name (must match the SKILL.md `name` field).
    fn name(&self) -> &str;

    /// Execute the skill with the given JSON arguments and return a text result.
    async fn execute(&self, arguments: serde_json::Value) -> Result<String>;
}

/// Registry of loaded skill definitions paired with their executors.
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
    executors: HashMap<String, Box<dyn SkillExecutor>>,
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
            executors: HashMap::new(),
        }
    }

    /// Load skills from a directory and register them (without executors).
    ///
    /// Executors must be registered separately via [`Self::register_executor`].
    pub fn load_from_dir(&mut self, dir: &Path) -> Result<()> {
        let skills = loader::load_skills_from_dir(dir)?;
        for skill in skills {
            let name = skill.manifest.name.clone();
            self.skills.insert(name, skill);
        }
        Ok(())
    }

    /// Register a Rust-backed executor for a skill.
    pub fn register_executor(&mut self, executor: Box<dyn SkillExecutor>) {
        self.executors.insert(executor.name().to_string(), executor);
    }

    /// Convert all loaded skills to LLM tool definitions.
    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.skills
            .values()
            .map(|skill| {
                let parameters = if skill.instructions.is_empty() {
                    serde_json::json!({
                        "type": "object",
                        "properties": {},
                    })
                } else {
                    serde_json::json!({
                        "type": "object",
                        "description": skill.instructions,
                        "properties": {},
                    })
                };

                ToolDefinition {
                    name: skill.manifest.name.clone(),
                    description: skill.manifest.description.clone(),
                    parameters,
                }
            })
            .collect()
    }

    /// Execute a skill by name.
    pub async fn execute(&self, name: &str, arguments: serde_json::Value) -> Result<String> {
        let executor = self
            .executors
            .get(name)
            .ok_or_else(|| anyhow!("No executor registered for skill: {name}"))?;

        executor.execute(arguments).await
    }

    /// Check if a skill executor is registered.
    pub fn has_executor(&self, name: &str) -> bool {
        self.executors.contains_key(name)
    }

    /// List loaded skill names.
    pub fn skill_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.skills.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}
