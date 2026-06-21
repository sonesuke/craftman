use std::path::PathBuf;

use craftman::core::skill::{SkillRegistry, load_skills_from_dir};

#[test]
fn test_load_skills_from_dir() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let skills = load_skills_from_dir(&skills_dir).unwrap();
    assert!(!skills.is_empty(), "should load at least one skill");

    let skill = skills
        .iter()
        .find(|s| s.manifest.name == "arithmetic")
        .unwrap();
    assert_eq!(skill.manifest.name, "arithmetic");
    assert!(!skill.manifest.description.is_empty());
    assert!(!skill.instructions.is_empty());
}

#[test]
fn test_registry_load_and_skill_names() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    let names = registry.skill_names();
    assert!(names.contains(&"arithmetic"));
}

#[test]
fn test_registry_activate_skill() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    let instructions = registry.activate("arithmetic").unwrap();
    assert!(instructions.contains("Arithmetic"));
}

#[test]
fn test_registry_activate_unknown_skill() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    assert!(registry.activate("nonexistent").is_err());
}

#[test]
fn test_registry_has_skill() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    assert!(registry.has_skill("arithmetic"));
    assert!(!registry.has_skill("nonexistent"));
}

#[test]
fn test_skill_allowed_tools_parsed() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    // arithmetic declares `allowed-tools: calculator` — a harness directive
    // exposing the calculator tool when the skill is active.
    let tools = registry.allowed_tools_of("arithmetic");
    assert_eq!(tools, vec!["calculator".to_string()]);
    assert!(registry.allowed_tools_of("nonexistent").is_empty());
}

#[test]
fn test_load_skill_tool_definition() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    let tool = registry.load_skill_tool_definition();
    assert_eq!(tool.name, "load_skill");
    assert!(tool.description.contains("instructions"));
    assert!(!tool.parameters["required"].as_array().unwrap().is_empty());
}

#[test]
fn test_retrieve_finds_relevant_skill() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    let results = registry.retrieve("evaluate a math expression", 5);
    assert!(!results.is_empty(), "should find at least one skill");

    let top = &results[0];
    assert_eq!(top.name, "arithmetic");
    assert!(top.score > 0.0);
}

#[test]
fn test_retrieve_returns_empty_for_no_match() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    let results = registry.retrieve("xyzzy-nothing-matches-this", 5);
    assert!(results.is_empty());
}

#[test]
fn test_retrieve_respects_top_k() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    let results = registry.retrieve("arithmetic", 1);
    assert!(results.len() <= 1);
}

#[test]
fn test_registry_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(dir.path()).unwrap();

    assert!(registry.skill_names().is_empty());
}

#[test]
fn test_registry_nonexistent_dir() {
    let mut registry = SkillRegistry::new();
    registry
        .load_from_dir(&PathBuf::from("/nonexistent/path"))
        .unwrap();

    assert!(registry.skill_names().is_empty());
}
