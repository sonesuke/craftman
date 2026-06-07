use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;

use crate::backends::OllamaBackend;
use crate::core::llm::{InputItem, OutputItem, ResponseRequest, StreamEvent};
use crate::core::skill::SkillRegistry;
use crate::skills;

const ANSI_DIM: &str = "\x1b[2m";
const ANSI_RESET: &str = "\x1b[0m";

/// Run an interactive chat REPL against the given Ollama backend.
pub async fn run(url: &str, model: &str, skills_dir: &Path) -> Result<()> {
    let backend = OllamaBackend::new(url, model);

    let mut registry = SkillRegistry::new();
    registry.load_from_dir(skills_dir)?;
    skills::register_builtin_executors(&mut registry);

    let tool_defs = registry.get_tool_definitions();
    if !tool_defs.is_empty() {
        let names = registry.skill_names().join(", ");
        println!("Loaded skills: {names}");
    }

    println!("Craftman Chat — model: {model}");
    println!("Type /quit or /exit to leave. /clear to reset history.");
    println!();

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

        // Send request with tools and handle the response loop (may involve
        // multiple round-trips if the LLM makes tool calls).
        match send_with_tools(&backend, &registry, &tool_defs, &mut history).await {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error: {e:#}");
                history.pop();
            }
        }
    }

    Ok(())
}

/// Send a request (with tools) and handle tool calls in a loop.
///
/// If the LLM responds with `ToolCall`s, this function executes each skill,
/// appends the results to history, and sends a follow-up request — repeating
/// until the LLM returns a plain text response.
async fn send_with_tools(
    backend: &OllamaBackend,
    registry: &SkillRegistry,
    tool_defs: &[crate::core::llm::ToolDefinition],
    history: &mut Vec<InputItem>,
) -> Result<()> {
    loop {
        let req = ResponseRequest {
            input: history.clone(),
            instructions: None,
            model: String::new(),
            tools: tool_defs.to_vec(),
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            reasoning: None,
        };

        let mut reasoning_text = String::new();
        let mut response_text = String::new();
        let mut in_reasoning = false;
        let mut tool_calls: Vec<(String, String, serde_json::Value)> = Vec::new();

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
                    // Collect tool calls from the response
                    for item in &resp.output {
                        if let OutputItem::ToolCall {
                            id,
                            name,
                            arguments,
                        } = item
                        {
                            tool_calls.push((id.clone(), name.clone(), arguments.clone()));
                        }
                    }
                }
            })
            .await?;

        if tool_calls.is_empty() {
            // No tool calls — final text response
            eprintln!();
            eprintln!();
            if !response_text.is_empty() {
                history.push(InputItem::assistant(&response_text));
            }
            return Ok(());
        }

        // Process tool calls
        eprintln!();
        for (id, name, arguments) in &tool_calls {
            eprintln!("{ANSI_DIM}[tool call: {name}]{ANSI_RESET}");

            // Add the tool call to history
            history.push(InputItem::ToolCall {
                id: id.clone(),
                name: name.clone(),
                arguments: arguments.clone(),
            });

            // Execute the skill
            let result = if registry.has_executor(name) {
                registry.execute(name, arguments.clone()).await
            } else {
                Err(anyhow::anyhow!("Unknown skill: {name}"))
            };

            let output_text = match result {
                Ok(text) => text,
                Err(e) => format!("Error: {e:#}"),
            };

            eprintln!("{ANSI_DIM}[tool result: {output_text}]{ANSI_RESET}");

            // Add tool result to history
            history.push(InputItem::ToolResult {
                call_id: id.clone(),
                output: output_text,
            });
        }

        // Loop back to send follow-up request with tool results
    }
}
