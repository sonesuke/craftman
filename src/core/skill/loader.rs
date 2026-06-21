use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::types::Skill;
use crate::core::skill::types::SkillManifest;

/// Load a single skill from a directory containing a `SKILL.md`.
pub fn load_skill(dir: &Path) -> Result<Skill> {
    let skill_md = dir.join("SKILL.md");
    let content = fs::read_to_string(&skill_md)
        .with_context(|| format!("Failed to read {}", skill_md.display()))?;

    parse_skill_md(&content)
}

/// Load all skills found in subdirectories of the given directory.
///
/// Each immediate child directory that contains a `SKILL.md` is loaded.
pub fn load_skills_from_dir(dir: &Path) -> Result<Vec<Skill>> {
    let mut skills = Vec::new();

    if !dir.exists() {
        return Ok(skills);
    }

    let entries = fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").exists() {
            match load_skill(&path) {
                Ok(skill) => skills.push(skill),
                Err(e) => eprintln!("Warning: skipping skill at {}: {e:#}", path.display()),
            }
        }
    }

    Ok(skills)
}

/// Parse a SKILL.md file content into a [`Skill`].
fn parse_skill_md(content: &str) -> Result<Skill> {
    // Extract YAML frontmatter delimited by `---`
    let content = content.trim_start();
    if !content.starts_with("---") {
        anyhow::bail!("SKILL.md must start with YAML frontmatter (---)");
    }

    let rest = &content[3..];

    let frontmatter_end = rest
        .find("\n---")
        .with_context(|| "SKILL.md: missing closing --- for frontmatter")?;

    let yaml_str = &rest[..frontmatter_end];
    let instructions = rest[frontmatter_end + 4..].trim().to_string();

    let manifest: SkillManifest =
        serde_yaml::from_str(yaml_str).with_context(|| "SKILL.md: invalid frontmatter YAML")?;

    // Validate name per agentskills.io spec
    validate_skill_name(&manifest.name)?;

    Ok(Skill {
        manifest,
        instructions,
    })
}

/// Validate that a skill name conforms to the agentskills.io specification.
fn validate_skill_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 {
        anyhow::bail!("skill name must be 1–64 characters, got: {name}");
    }
    if name.starts_with('-') || name.ends_with('-') {
        anyhow::bail!("skill name must not start or end with a hyphen: {name}");
    }
    if name.contains("--") {
        anyhow::bail!("skill name must not contain consecutive hyphens: {name}");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        anyhow::bail!(
            "skill name must contain only lowercase letters, digits, and hyphens: {name}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_skill_md() {
        let content = "\
---
name: calculator
description: A simple calculator
---
# Calculator

Evaluates expressions.
";
        let skill = parse_skill_md(content).unwrap();
        assert_eq!(skill.manifest.name, "calculator");
        assert_eq!(skill.manifest.description, "A simple calculator");
        assert!(skill.instructions.starts_with("# Calculator"));
    }

    #[test]
    fn test_parse_skill_with_optional_fields() {
        let content = "\
---
name: my-tool
description: Does something
license: MIT
compatibility: requires curl
metadata:
  author: test
---
Instructions here.
";
        let skill = parse_skill_md(content).unwrap();
        assert_eq!(skill.manifest.name, "my-tool");
        assert_eq!(skill.manifest.license.as_deref(), Some("MIT"));
        assert_eq!(
            skill.manifest.compatibility.as_deref(),
            Some("requires curl")
        );
        assert_eq!(skill.instructions, "Instructions here.");
    }

    #[test]
    fn test_parse_allowed_tools() {
        let content = "\
---
name: my-skill
description: Does something
allowed-tools: calculator notes
---
Instructions here.
";
        let skill = parse_skill_md(content).unwrap();
        assert_eq!(
            skill.manifest.allowed_tools.as_deref(),
            Some("calculator notes")
        );
    }

    #[test]
    fn test_reject_invalid_names() {
        let cases = vec![
            ("-bad", "leading hyphen"),
            ("bad-", "trailing hyphen"),
            ("no--double", "consecutive hyphens"),
            ("UpperCase", "uppercase"),
            ("with space", "space"),
            ("", "empty"),
        ];
        for (name, reason) in cases {
            let content = format!("---\nname: {name}\ndescription: test\n---\nbody");
            assert!(parse_skill_md(&content).is_err(), "should reject: {reason}");
        }
    }

    #[test]
    fn test_reject_missing_frontmatter() {
        let content = "# No frontmatter\nSome text";
        assert!(parse_skill_md(content).is_err());
    }
}
