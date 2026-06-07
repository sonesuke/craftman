pub mod calculator;

use crate::core::skill::SkillRegistry;

/// Register all built-in skill executors into the registry.
pub fn register_builtin_executors(registry: &mut SkillRegistry) {
    registry.register_executor(Box::new(calculator::Calculator));
}
