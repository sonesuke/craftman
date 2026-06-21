use std::collections::HashSet;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;

use crate::backends::OllamaBackend;
use crate::core::llm::{InputItem, OutputItem, ResponseRequest, Role, StreamEvent, ToolDefinition};
use crate::core::skill::SkillRegistry;
use crate::tools::ToolRegistry;
use crate::tools::calculator::Calculator;

/// How many skills skill retrieval surfaces per user turn.
const RETRIEVAL_TOP_K: usize = 3;

const ANSI_DIM: &str = "\x1b[2m";
const ANSI_RESET: &str = "\x1b[0m";

/// Run an interactive chat REPL against the given Ollama backend.
pub async fn run(url: &str, model: &str, skills_dir: &Path, log_file: Option<&Path>) -> Result<()> {
    let mut backend = OllamaBackend::new(url, model);
    if let Some(path) = log_file {
        backend.with_log_file(path);
    }

    let mut registry = SkillRegistry::new();
    registry.load_from_dir(skills_dir)?;

    let names = registry.skill_names();
    if names.is_empty() {
        println!("No skills loaded.");
    } else {
        println!("Available skills: {}", names.join(", "));
    }

    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(Box::new(Calculator));

    println!("Craftman Chat — model: {model}");
    println!("Type /quit or /exit to leave. /clear to reset history. /skills to list skills.");
    println!();

    let activate_tool = registry.activate_skill_tool_definition();

    let mut history: Vec<InputItem> = Vec::new();
    // Skills activated via activate_skill this session. Their `allowed-tools` are
    // exposed to the model; /clear resets both history and this set.
    let mut active_skills: HashSet<String> = HashSet::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush()?;

        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            println!();
            break;
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        match input {
            "/quit" | "/exit" => break,
            "/clear" => {
                history.clear();
                active_skills.clear();
                println!("(history cleared)");
                continue;
            }
            "/skills" => {
                let names = registry.skill_names();
                if names.is_empty() {
                    println!("(no skills loaded)");
                } else {
                    for name in names {
                        println!("- {name}");
                    }
                }
                continue;
            }
            _ => {}
        }

        history.push(InputItem::user(input));

        match send_turn(
            &backend,
            &registry,
            &tool_registry,
            &activate_tool,
            &mut active_skills,
            &mut history,
        )
        .await
        {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error: {e:#}");
                history.pop();
            }
        }
    }

    Ok(())
}

/// Send one turn (may involve multiple round-trips for tool calls).
///
/// Skill retrieval runs once per turn (relevant skills injected via `instructions`).
/// Tools are exposed only through the `allowed-tools` of active skills, so the
/// `tools` array is rebuilt every iteration — activating a skill mid-turn makes
/// its tools available on the next round-trip.
async fn send_turn(
    backend: &OllamaBackend,
    registry: &SkillRegistry,
    tool_registry: &ToolRegistry,
    activate_tool: &ToolDefinition,
    active_skills: &mut HashSet<String>,
    history: &mut Vec<InputItem>,
) -> Result<()> {
    let instructions = build_retrieval_instructions(registry, history);

    loop {
        let mut tools = vec![activate_tool.clone()];
        tools.extend(exposed_tool_defs(registry, tool_registry, active_skills));

        let req = ResponseRequest {
            input: history.clone(),
            instructions: instructions.clone(),
            model: String::new(),
            tools,
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            reasoning: None,
        };

        let mut response_text = String::new();
        let mut in_reasoning = false;
        let mut tool_calls: Vec<(Option<String>, String, serde_json::Value)> = Vec::new();

        let _output = backend
            .stream_create_response(req, |event| match event {
                StreamEvent::ReasoningDelta(delta) => {
                    if !in_reasoning {
                        eprint!("{ANSI_DIM}thinking: ");
                        in_reasoning = true;
                    }
                    eprint!("{delta}");
                    let _ = io::stderr().flush();
                }
                StreamEvent::TextDelta(delta) => {
                    if in_reasoning {
                        eprint!("{ANSI_RESET}");
                        eprintln!();
                        in_reasoning = false;
                    }
                    eprint!("{delta}");
                    let _ = io::stderr().flush();
                    response_text.push_str(&delta);
                }
                StreamEvent::Done(resp) => {
                    if in_reasoning {
                        eprint!("{ANSI_RESET}");
                        eprintln!();
                        in_reasoning = false;
                    }
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
            eprintln!();
            eprintln!();
            if !response_text.is_empty() {
                history.push(InputItem::assistant(&response_text));
            }
            return Ok(());
        }

        // Handle tool calls
        eprintln!();
        for (call_id, name, arguments) in &tool_calls {
            let display = format_tool_call(name, arguments);
            eprintln!("{ANSI_DIM}{display}{ANSI_RESET}");

            history.push(InputItem::ToolCall {
                id: call_id.clone().unwrap_or_default(),
                call_id: call_id.clone(),
                name: name.clone(),
                arguments: arguments.clone(),
            });

            let output_text =
                handle_tool_call(registry, tool_registry, active_skills, name, arguments).await;

            eprintln!("{ANSI_DIM}[tool result injected into context]{ANSI_RESET}");

            history.push(InputItem::ToolResult {
                call_id: call_id.clone().unwrap_or_default(),
                output: output_text,
            });
        }

        // Loop: send follow-up with tool results so the LLM can respond
    }
}

/// Tool definitions exposed this turn: the union of `allowed-tools` across all
/// active skills, resolved against the registry (de-duplicated).
fn exposed_tool_defs(
    registry: &SkillRegistry,
    tool_registry: &ToolRegistry,
    active_skills: &HashSet<String>,
) -> Vec<ToolDefinition> {
    let names: Vec<String> = active_skills
        .iter()
        .flat_map(|skill| registry.allowed_tools_of(skill))
        .collect();
    tool_registry.definitions_for(&names)
}

/// Build the skill retrieval instructions for the current turn.
///
/// Retrieves the `RETRIEVAL_TOP_K` skills most relevant to the latest user
/// message and lists them so the model can `activate_skill` the right one.
/// Returns `None` when there is no user message or no skill matches, leaving
/// the request without special instructions (as before).
fn build_retrieval_instructions(registry: &SkillRegistry, history: &[InputItem]) -> Option<String> {
    let query = last_user_message(history)?;
    let skills = registry.retrieve(query, RETRIEVAL_TOP_K);
    if skills.is_empty() {
        return None;
    }

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

/// Return the content of the most recent user message in `history`, if any.
fn last_user_message(history: &[InputItem]) -> Option<&str> {
    history.iter().rev().find_map(|item| match item {
        InputItem::Message {
            role: Role::User,
            content,
        } => Some(content.as_str()),
        _ => None,
    })
}

/// Format a tool call for display.
fn format_tool_call(name: &str, arguments: &serde_json::Value) -> String {
    match name {
        "activate_skill" => {
            let skill_name = arguments["name"].as_str().unwrap_or("");
            format!("[activate_skill: {skill_name}]")
        }
        _ => {
            let arg = arguments
                .as_object()
                .and_then(|m| m.values().find_map(|v| v.as_str()))
                .unwrap_or("");
            format!("[{name}: {arg}]")
        }
    }
}

/// Handle a single tool call from the LLM.
///
/// `activate_skill` activates a skill — injecting its instructions and recording it
/// active so its `allowed-tools` are exposed. Any other name dispatches to the
/// matching registered tool.
async fn handle_tool_call(
    registry: &SkillRegistry,
    tool_registry: &ToolRegistry,
    active_skills: &mut HashSet<String>,
    name: &str,
    arguments: &serde_json::Value,
) -> String {
    match name {
        "activate_skill" => {
            let skill_name = arguments["name"].as_str().unwrap_or("");
            match registry.activate(skill_name) {
                Ok(instructions) => {
                    active_skills.insert(skill_name.to_string());
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
        _ => match tool_registry.invoke(name, arguments.clone()).await {
            Ok(out) => out,
            Err(e) => format!("Error: {e:#}"),
        },
    }
}
