use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;

use crate::backends::OllamaBackend;
use crate::core::llm::{InputItem, OutputItem, ResponseRequest, StreamEvent};
use crate::core::skill::SkillRegistry;

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

    let search_tool = registry.search_skills_tool_definition();
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

        match send_turn(&backend, &registry, &search_tool, &load_tool, &mut history).await {
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
async fn send_turn(
    backend: &OllamaBackend,
    registry: &SkillRegistry,
    search_tool: &crate::core::llm::ToolDefinition,
    load_tool: &crate::core::llm::ToolDefinition,
    history: &mut Vec<InputItem>,
) -> Result<()> {
    loop {
        let req = ResponseRequest {
            input: history.clone(),
            instructions: None,
            model: String::new(),
            tools: vec![search_tool.clone(), load_tool.clone()],
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

/// Format a tool call for display.
fn format_tool_call(name: &str, arguments: &serde_json::Value) -> String {
    match name {
        "search_skills" => {
            let query = arguments["query"].as_str().unwrap_or("");
            format!("[search_skills: {query}]")
        }
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
        "search_skills" => {
            let query = arguments["query"].as_str().unwrap_or("");
            let results = registry.search(query, 3);
            if results.is_empty() {
                "No skills found matching your query.".to_string()
            } else {
                let mut out = "Found matching skills:\n".to_string();
                for r in &results {
                    out.push_str(&format!("- \"{}\": {}\n", r.name, r.description));
                }
                out.push_str("Use load_skill with the exact name to activate a skill.");
                out
            }
        }
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
