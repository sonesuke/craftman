use std::io::{self, Write};

use anyhow::Result;

use crate::backends::OllamaBackend;
use crate::core::llm::{InputItem, ResponseRequest, StreamEvent};

const ANSI_DIM: &str = "\x1b[2m";
const ANSI_RESET: &str = "\x1b[0m";

/// Run an interactive chat REPL against the given Ollama backend.
pub async fn run(url: &str, model: &str) -> Result<()> {
    let backend = OllamaBackend::new(url, model);

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
            // EOF (Ctrl-D)
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
            _ => {}
        }

        history.push(InputItem::user(input));

        let req = ResponseRequest {
            input: history.clone(),
            instructions: None,
            model: String::new(),
            tools: vec![],
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            reasoning: None,
        };

        let mut reasoning_text = String::new();
        let mut response_text = String::new();
        let mut in_reasoning = false;

        match backend
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
                    // Transition from reasoning to text output
                    if in_reasoning {
                        eprint!("{ANSI_RESET}");
                        eprintln!();
                        in_reasoning = false;
                    }
                    eprint!("{delta}");
                    let _ = io::stderr().flush();
                    response_text.push_str(&delta);
                }
                StreamEvent::Done(_) => {
                    if in_reasoning {
                        eprint!("{ANSI_RESET}");
                        eprintln!();
                        in_reasoning = false;
                    }
                }
            })
            .await
        {
            Ok(_) => {
                eprintln!();
                eprintln!();
                if !response_text.is_empty() {
                    history.push(InputItem::assistant(&response_text));
                }
            }
            Err(e) => {
                eprintln!();
                eprintln!("Error: {e:#}");
                // Remove the user message so the history stays consistent
                history.pop();
            }
        }
    }

    Ok(())
}
