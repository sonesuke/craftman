use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;

use crate::app::logging::JsonlLogger;
use crate::backends::OllamaBackend;
use crate::core::harness::{EventSink, Harness, HarnessEvent};
use crate::core::skill::SkillRegistry;
use crate::tools::ToolRegistry;
use crate::tools::calculator::Calculator;

const ANSI_DIM: &str = "\x1b[2m";
const ANSI_RESET: &str = "\x1b[0m";

/// Run an interactive chat REPL.
///
/// The REPL owns only input/output and command handling; the agent logic
/// (skill retrieval, tool dispatch, session state) lives in the [`Harness`],
/// which emits events to a display sink (and an optional JSONL logger sink).
pub async fn run(url: &str, model: &str, skills_dir: &Path, log_file: Option<&Path>) -> Result<()> {
    let backend = OllamaBackend::new(url, model);

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

    let mut harness = Harness::new(backend, registry, tool_registry);
    harness.add_sink(Box::new(CliSink::default()));
    if let Some(path) = log_file {
        harness.add_sink(Box::new(JsonlLogger::new(path)?));
    }

    println!("Craftman Chat — model: {model}");
    println!("Type /quit or /exit to leave. /clear to reset history. /skills to list skills.");
    println!();

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
                harness.clear();
                println!("(history cleared)");
                continue;
            }
            "/skills" => {
                let names = harness.skill_names();
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

        if let Err(e) = harness.submit(input).await {
            eprintln!("Error: {e:#}");
        }
    }

    Ok(())
}

/// [`EventSink`] that renders harness events to the terminal.
///
/// Reasoning, assistant text, and tool activity are written to stderr
/// (diagnostics). A follow-up splits the user-facing answer to stdout.
#[derive(Default)]
struct CliSink {
    in_reasoning: bool,
}

impl EventSink for CliSink {
    fn on_event(&mut self, event: &HarnessEvent) {
        match event {
            HarnessEvent::SkillRetrieved { .. } | HarnessEvent::ToolsExposed { .. } => {}
            HarnessEvent::SkillActivated { name } => {
                eprintln!("{ANSI_DIM}[activate_skill: {name}]{ANSI_RESET}");
            }
            HarnessEvent::ToolCalled { name, args } => {
                let display = if name == "activate_skill" {
                    let skill_name = args["name"].as_str().unwrap_or("");
                    format!("[activate_skill: {skill_name}]")
                } else {
                    let arg = args
                        .as_object()
                        .and_then(|m| m.values().find_map(|v| v.as_str()))
                        .unwrap_or("");
                    format!("[{name}: {arg}]")
                };
                eprintln!("{ANSI_DIM}{display}{ANSI_RESET}");
            }
            HarnessEvent::ToolResult { .. } => {
                eprintln!("{ANSI_DIM}[tool result injected into context]{ANSI_RESET}");
            }
            HarnessEvent::ReasoningDelta { text } => {
                if !self.in_reasoning {
                    eprint!("{ANSI_DIM}thinking: ");
                    self.in_reasoning = true;
                }
                eprint!("{text}");
                let _ = io::stderr().flush();
            }
            HarnessEvent::AssistantDelta { text } => {
                if self.in_reasoning {
                    eprint!("{ANSI_RESET}");
                    eprintln!();
                    self.in_reasoning = false;
                }
                eprint!("{text}");
                let _ = io::stderr().flush();
            }
            HarnessEvent::TurnComplete { .. } => {
                if self.in_reasoning {
                    eprint!("{ANSI_RESET}");
                    eprintln!();
                    self.in_reasoning = false;
                }
                eprintln!();
                eprintln!();
            }
        }
    }
}
