use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;

use crate::backends::OllamaBackend;
use crate::core::llm::{InputItem, OutputItem, ResponseRequest, Role, StreamEvent};
use crate::core::skill::SkillRegistry;

/// How many skills ToolRAG surfaces per user turn.
const TOOLRAG_TOP_K: usize = 3;

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

    println!("Craftman Chat — model: {model}");
    println!("Type /quit or /exit to leave. /clear to reset history. /skills to list skills.");
    println!();

    let load_tool = registry.load_skill_tool_definition();

    let mut history: Vec<InputItem> = Vec::new();
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

        match send_turn(&backend, &registry, &load_tool, &mut history).await {
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
/// ToolRAG runs once per turn: the skills most relevant to the latest user
/// message are retrieved and injected via `instructions`, so the model sees
/// only the relevant subset instead of the whole catalog.
async fn send_turn(
    backend: &OllamaBackend,
    registry: &SkillRegistry,
    load_tool: &crate::core::llm::ToolDefinition,
    history: &mut Vec<InputItem>,
) -> Result<()> {
    let instructions = build_toolrag_instructions(registry, history);

    loop {
        let req = ResponseRequest {
            input: history.clone(),
            instructions: instructions.clone(),
            model: String::new(),
            tools: vec![load_tool.clone()],
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            reasoning: None,
        };

        let mut reasoning_text = String::new();
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
                    reasoning_text.push_str(&delta);
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

            let output_text = handle_tool_call(registry, name, arguments);

            eprintln!("{ANSI_DIM}[tool result injected into context]{ANSI_RESET}");

            history.push(InputItem::ToolResult {
                call_id: call_id.clone().unwrap_or_default(),
                output: output_text,
            });
        }

        // Loop: send follow-up with tool results so the LLM can respond
    }
}

/// Build the ToolRAG system instructions for the current turn.
///
/// Retrieves the `TOOLRAG_TOP_K` skills most relevant to the latest user
/// message and lists them so the model can `load_skill` the right one.
/// Returns `None` when there is no user message or no skill matches, leaving
/// the request without special instructions (as before).
fn build_toolrag_instructions(registry: &SkillRegistry, history: &[InputItem]) -> Option<String> {
    let query = last_user_message(history)?;
    let skills = registry.retrieve(query, TOOLRAG_TOP_K);
    if skills.is_empty() {
        return None;
    }

    let mut out = String::from(
        "You are craftman, a CLI assistant. \
         Relevant skills for this request (call load_skill with the exact name to activate one if useful):\n",
    );
    for skill in &skills {
        out.push_str(&format!("- \"{}\": {}\n", skill.name, skill.description));
    }
    out.push_str("If none are relevant, answer directly without loading a skill.");
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
        "load_skill" => {
            let skill_name = arguments["name"].as_str().unwrap_or("");
            format!("[load_skill: {skill_name}]")
        }
        _ => format!("[{name}]"),
    }
}

/// Handle a single tool call from the LLM.
fn handle_tool_call(registry: &SkillRegistry, name: &str, arguments: &serde_json::Value) -> String {
    match name {
        "load_skill" => {
            let skill_name = arguments["name"].as_str().unwrap_or("");
            match registry.activate(skill_name) {
                Ok(instructions) => {
                    if instructions.is_empty() {
                        format!(
                            "Skill '{skill_name}' is now loaded and active. \
                             Do not call load_skill for '{skill_name}' again."
                        )
                    } else {
                        format!(
                            "Skill '{skill_name}' is now loaded and active. \
                             Do not call load_skill for '{skill_name}' again.\n\n\
                             --- Skill Instructions ---\n{instructions}\n--- End of Instructions ---"
                        )
                    }
                }
                Err(e) => format!("Error: {e:#}"),
            }
        }
        _ => format!("Unknown tool: {name}"),
    }
}
