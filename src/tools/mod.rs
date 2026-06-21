//! Executable tools exposed to the model as function calls.
//!
//! Tools are registered here and surfaced to the model only through the
//! `allowed-tools` of active skills (see `docs/adr/0002-*`). Tool results flow
//! back to the model via the existing tool-result path in the chat loop.

use std::collections::HashSet;

use anyhow::{Result, anyhow};
use async_trait::async_trait;

use crate::core::llm::ToolDefinition;

pub mod calculator;

/// An executable tool the model can invoke by name with JSON arguments.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique function name (e.g. "calculator").
    fn name(&self) -> &str;
    /// What the tool does — shown to the model.
    fn description(&self) -> &str;
    /// JSON Schema describing the tool's parameters.
    fn parameters(&self) -> serde_json::Value;
    /// Execute the tool; the returned string is fed back to the model.
    async fn call(&self, args: serde_json::Value) -> Result<String>;
}

/// Registry of executable tools.
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    /// Resolve the given tool names to definitions, de-duplicated and in
    /// registration order. Unknown names are skipped silently — a skill may
    /// declare a tool this build doesn't register.
    pub fn definitions_for(&self, names: &[String]) -> Vec<ToolDefinition> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut out = Vec::new();
        for tool in &self.tools {
            let name = tool.name();
            if names.iter().any(|n| n == name) && seen.insert(name) {
                out.push(ToolDefinition {
                    name: name.to_string(),
                    description: tool.description().to_string(),
                    parameters: tool.parameters(),
                });
            }
        }
        out
    }

    /// Invoke a registered tool by name.
    pub async fn invoke(&self, name: &str, args: serde_json::Value) -> Result<String> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| anyhow!("Unknown tool: {name}"))?;
        tool.call(args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo {
        name: &'static str,
    }

    #[async_trait]
    impl Tool for Echo {
        fn name(&self) -> &str {
            self.name
        }
        fn description(&self) -> &str {
            "echo"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn call(&self, args: serde_json::Value) -> Result<String> {
            Ok(args.to_string())
        }
    }

    #[tokio::test]
    async fn test_invoke_dispatches_by_name() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(Echo { name: "alpha" }));
        reg.register(Box::new(Echo { name: "beta" }));

        let out = reg
            .invoke("beta", serde_json::json!({"x": 1}))
            .await
            .unwrap();
        assert!(out.contains("\"x\":1"));
        assert!(reg.invoke("nope", serde_json::json!({})).await.is_err());
    }

    #[test]
    fn test_definitions_for_dedups_and_skips_unknown() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(Echo { name: "alpha" }));
        reg.register(Box::new(Echo { name: "beta" }));

        let names = vec![
            "beta".to_string(),
            "missing".to_string(),
            "beta".to_string(),
        ];
        let defs = reg.definitions_for(&names);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "beta");
    }
}
