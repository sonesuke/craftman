use std::path::PathBuf;

use craftman::core::skill::{SkillRegistry, load_skills_from_dir};

#[test]
fn test_load_skills_from_dir() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let skills = load_skills_from_dir(&skills_dir).unwrap();
    assert!(!skills.is_empty(), "should load at least one skill");

    let calc = skills
        .iter()
        .find(|s| s.manifest.name == "calculator")
        .unwrap();
    assert_eq!(calc.manifest.name, "calculator");
    assert!(!calc.manifest.description.is_empty());
    assert!(!calc.instructions.is_empty());
}

#[test]
fn test_registry_load_and_skill_names() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    let names = registry.skill_names();
    assert!(names.contains(&"calculator"));
}

#[test]
fn test_registry_activate_skill() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    let instructions = registry.activate("calculator").unwrap();
    assert!(instructions.contains("Calculator"));
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

    assert!(registry.has_skill("calculator"));
    assert!(!registry.has_skill("nonexistent"));
}

#[test]
fn test_load_skill_tool_definition() {
    let skills_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut registry = SkillRegistry::new();
    registry.load_from_dir(&skills_dir).unwrap();

    let tool = registry.load_skill_tool_definition();
    assert_eq!(tool.name, "load_skill");
    assert!(tool.description.contains("calculator"));
    assert!(!tool.parameters["required"].as_array().unwrap().is_empty());
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
