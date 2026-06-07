pub mod loader;
pub mod types;

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow};

pub use loader::{load_skill, load_skills_from_dir};
pub use types::{Skill, SkillManifest};

use crate::core::llm::ToolDefinition;

/// Registry of loaded skill definitions.
///
/// Implements the Agent Skills progressive disclosure pattern:
/// 1. **Discovery**: skill names + descriptions loaded at startup
/// 2. **Activation**: `load_skill` tool returns full instructions on demand
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

    /// Build a description string listing all available skills for the load_skill tool.
    fn skills_summary(&self) -> String {
        let mut summary = String::from(
            "Load a skill's full instructions into context by its exact name. \
             You must use the exact skill name as listed below.\n\nAvailable skills:\n",
        );
        let mut names: Vec<&String> = self.skills.keys().collect();
        names.sort();
        for name in names {
            let skill = &self.skills[name];
            summary.push_str(&format!("- \"{}\": {}\n", name, skill.manifest.description));
        }
        summary
    }

    /// Return the `load_skill` tool definition for the LLM.
    pub fn load_skill_tool_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "load_skill".to_string(),
            description: self.skills_summary(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the skill to load"
                    }
                },
                "required": ["name"]
            }),
        }
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
