use std::path::PathBuf;

use clap::Parser;

use craftman::app::chat;

#[derive(Parser)]
#[command(
    name = "craftman",
    version,
    about = "A CLI assistant powered by local LLMs"
)]
struct Cli {
    /// Ollama model to use
    #[arg(long, default_value = "LiquidAI/lfm2.5-1.2b-instruct")]
    model: String,

    /// Ollama base URL
    #[arg(long, default_value = "http://host.docker.internal:11434")]
    url: String,

    /// Directory containing skill definitions (SKILL.md)
    #[arg(long, default_value = "./skills")]
    skills_dir: PathBuf,

    /// Log structured harness events to this file (JSONL format)
    #[arg(long)]
    log: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(e) = chat::run(&cli.url, &cli.model, &cli.skills_dir, cli.log.as_deref()).await {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}
