//! Harness — the agent runtime that orchestrates a turn.
//!
//! Owns session state (history, active skills) and runs skill retrieval ->
//! activation -> tool chain -> answer, emitting each step as a [`HarnessEvent`]
//! to every registered [`EventSink`]. The CLI and observers (logs, metrics)
//! consume those events without knowing the harness internals.

use std::collections::HashSet;

use anyhow::Result;
use serde::Serialize;

use crate::core::llm::{
    InputItem, OutputItem, ResponseModel, ResponseRequest, Role, StreamEvent, TokenUsage,
    ToolDefinition,
};
use crate::core::skill::SkillRegistry;
use crate::tools::ToolRegistry;

/// How many skills skill retrieval surfaces per user turn.
const RETRIEVAL_TOP_K: usize = 3;

/// An observable step emitted by the harness during a turn.
///
/// Consumers choose what to react to: the CLI renders deltas and tool calls;
/// a logger records the structured events (not deltas) as JSONL.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum HarnessEvent {
    /// Skill retrieval selected these `(name, description)` pairs for the turn.
    SkillRetrieved { skills: Vec<(String, String)> },
    /// A skill was activated (instructions loaded, allowed-tools exposed).
    SkillActivated { name: String },
    /// Tools currently exposed to the model (excludes the always-present
    /// activation tool).
    ToolsExposed { tools: Vec<String> },
    /// The model called a tool.
    ToolCalled {
        name: String,
        args: serde_json::Value,
    },
    /// The harness executed a tool and got this output.
    ToolResult { name: String, output: String },
    /// A chunk of reasoning (thinking) text.
    ReasoningDelta { text: String },
    /// A chunk of assistant answer text.
    AssistantDelta { text: String },
    /// The turn finished; carries token usage for observability.
    TurnComplete { usage: TokenUsage },
}

/// A consumer of [`HarnessEvent`]s — the boundary between the harness and the
/// CLI / observers.
pub trait EventSink: Send {
    fn on_event(&mut self, event: &HarnessEvent);
}

/// Fan an event out to every sink.
fn emit_all(sinks: &mut [Box<dyn EventSink>], event: &HarnessEvent) {
    for sink in sinks.iter_mut() {
        sink.on_event(event);
    }
}

/// The craftman runtime that orchestrates a turn.
///
/// Generic over any [`ResponseModel`] backend so it can be driven by a mock in
/// tests. Owns session state; emits events through [`EventSink`].
pub struct Harness<L: ResponseModel> {
    llm: L,
    registry: SkillRegistry,
    tool_registry: ToolRegistry,
    activate_tool: ToolDefinition,
    history: Vec<InputItem>,
    active_skills: HashSet<String>,
    sinks: Vec<Box<dyn EventSink>>,
}

impl<L: ResponseModel> Harness<L> {
    pub fn new(llm: L, registry: SkillRegistry, tool_registry: ToolRegistry) -> Self {
        let activate_tool = registry.activate_skill_tool_definition();
        Self {
            llm,
            registry,
            tool_registry,
            activate_tool,
            history: Vec::new(),
            active_skills: HashSet::new(),
            sinks: Vec::new(),
        }
    }

    /// Register an observer of harness events (CLI display, logger, ...).
    pub fn add_sink(&mut self, sink: Box<dyn EventSink>) {
        self.sinks.push(sink);
    }

    /// Skill names activated this session.
    pub fn active_skills(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.active_skills.iter().map(|s| s.as_str()).collect();
        v.sort();
        v
    }

    /// All indexed skill names (for `/skills`).
    pub fn skill_names(&self) -> Vec<&str> {
        self.registry.skill_names()
    }

    /// Reset session history and active skills (`/clear`).
    pub fn clear(&mut self) {
        self.history.clear();
        self.active_skills.clear();
    }

    /// Run one turn: push the user message, run retrieval, and loop over
    /// model round-trips (activating skills, dispatching tools) until the
    /// model answers. Emits events to all sinks.
    pub async fn submit(&mut self, user_msg: &str) -> Result<()> {
        self.history.push(InputItem::user(user_msg));
        let instructions = self.build_retrieval_instructions();

        loop {
            let exposed = self.exposed_tool_defs();
            emit_all(
                &mut self.sinks,
                &HarnessEvent::ToolsExposed {
                    tools: exposed.iter().map(|t| t.name.clone()).collect(),
                },
            );

            let mut tools = vec![self.activate_tool.clone()];
            tools.extend(exposed);

            let req = ResponseRequest {
                input: self.history.clone(),
                instructions: instructions.clone(),
                model: String::new(),
                tools,
                temperature: None,
                top_p: None,
                max_output_tokens: None,
                reasoning: None,
            };

            let mut response_text = String::new();
            let mut tool_calls: Vec<(Option<String>, String, serde_json::Value)> = Vec::new();
            let mut usage = TokenUsage::default();

            let sinks = &mut self.sinks;
            let llm = &self.llm;
            llm.stream_create_response(req, &mut |event| match event {
                StreamEvent::ReasoningDelta(d) => {
                    emit_all(sinks, &HarnessEvent::ReasoningDelta { text: d.clone() });
                }
                StreamEvent::TextDelta(d) => {
                    emit_all(sinks, &HarnessEvent::AssistantDelta { text: d.clone() });
                    response_text.push_str(&d);
                }
                StreamEvent::Done(resp) => {
                    usage = resp.usage.clone();
                    for item in &resp.output {
                        if let OutputItem::ToolCall {
                            call_id,
                            name,
                            arguments,
                            ..
                        } = item
                        {
                            tool_calls.push((call_id.clone(), name.clone(), arguments.clone()));
                        }
                    }
                }
            })
            .await?;

            if tool_calls.is_empty() {
                if !response_text.is_empty() {
                    self.history.push(InputItem::assistant(&response_text));
                }
                emit_all(&mut self.sinks, &HarnessEvent::TurnComplete { usage });
                return Ok(());
            }

            for (call_id, name, arguments) in &tool_calls {
                emit_all(
                    &mut self.sinks,
                    &HarnessEvent::ToolCalled {
                        name: name.clone(),
                        args: arguments.clone(),
                    },
                );

                self.history.push(InputItem::ToolCall {
                    id: call_id.clone().unwrap_or_default(),
                    call_id: call_id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                });

                let output = self.handle_tool_call(name, arguments).await;

                emit_all(
                    &mut self.sinks,
                    &HarnessEvent::ToolResult {
                        name: name.clone(),
                        output: output.clone(),
                    },
                );

                self.history.push(InputItem::ToolResult {
                    call_id: call_id.clone().unwrap_or_default(),
                    output,
                });
            }
        }
    }

    /// Tools exposed this round: the union of `allowed-tools` across active
    /// skills, resolved against the registry (de-duplicated).
    fn exposed_tool_defs(&self) -> Vec<ToolDefinition> {
        let names: Vec<String> = self
            .active_skills
            .iter()
            .flat_map(|skill| self.registry.allowed_tools_of(skill))
            .collect();
        self.tool_registry.definitions_for(&names)
    }

    /// Build the skill-retrieval instructions for the current turn and emit
    /// a [`HarnessEvent::SkillRetrieved`] event.
    fn build_retrieval_instructions(&mut self) -> Option<String> {
        let query = self.history.iter().rev().find_map(|item| match item {
            InputItem::Message {
                role: Role::User,
                content,
            } => Some(content.as_str()),
            _ => None,
        })?;
        let skills = self.registry.retrieve(query, RETRIEVAL_TOP_K);
        if skills.is_empty() {
            return None;
        }
        emit_all(
            &mut self.sinks,
            &HarnessEvent::SkillRetrieved {
                skills: skills
                    .iter()
                    .map(|s| (s.name.clone(), s.description.clone()))
                    .collect(),
            },
        );

        let mut out = String::from(
            "You are craftman, a CLI assistant. Relevant skills for this request \
             (call activate_skill with the exact name to activate one if useful):\n",
        );
        for skill in &skills {
            out.push_str(&format!("- \"{}\": {}\n", skill.name, skill.description));
        }
        out.push_str("If none are relevant, answer directly without activating a skill.");
        Some(out)
    }

    /// Dispatch a tool call. `activate_skill` activates a skill (and records
    /// it active so its `allowed-tools` are exposed, emitting
    /// [`HarnessEvent::SkillActivated`]); any other name dispatches to the
    /// matching registered tool.
    async fn handle_tool_call(&mut self, name: &str, arguments: &serde_json::Value) -> String {
        match name {
            "activate_skill" => {
                let skill_name = arguments["name"].as_str().unwrap_or("");
                match self.registry.activate(skill_name) {
                    Ok(instructions) => {
                        self.active_skills.insert(skill_name.to_string());
                        emit_all(
                            &mut self.sinks,
                            &HarnessEvent::SkillActivated {
                                name: skill_name.to_string(),
                            },
                        );
                        if instructions.is_empty() {
                            format!(
                                "Skill '{skill_name}' is now active and its tools are exposed. \
                                 Do not call activate_skill for '{skill_name}' again."
                            )
                        } else {
                            format!(
                                "Skill '{skill_name}' is now active and its tools are exposed. \
                                 Do not call activate_skill for '{skill_name}' again.\n\n\
                                 --- Skill Instructions ---\n{instructions}\n--- End of Instructions ---"
                            )
                        }
                    }
                    Err(e) => format!("Error: {e:#}"),
                }
            }
            _ => match self.tool_registry.invoke(name, arguments.clone()).await {
                Ok(out) => out,
                Err(e) => format!("Error: {e:#}"),
            },
        }
    }
}
