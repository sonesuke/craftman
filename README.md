# craftman

A CLI assistant powered by local LLMs via [Ollama](https://ollama.com).

## Quick Start

```bash
# Build and run
cargo run

# Specify model and Ollama URL
cargo run -- --model lfm2.5-thinking:latest --url http://localhost:11434
```

## Usage

```
$ craftman --help
A CLI assistant powered by local LLMs

Usage: craftman [OPTIONS]

Options:
      --model <MODEL>  Ollama model to use [default: LiquidAI/lfm2.5-1.2b-instruct]
      --url <URL>      Ollama base URL [default: http://host.docker.internal:11434]
  -h, --help           Print help
  -V, --version        Print version
```

### Chat Commands

| Command | Description |
|---------|-------------|
| `/quit`, `/exit` | Exit the chat |
| `/clear` | Clear conversation history |

## Development

```bash
# Format check
cargo fmt --all -- --check

# Lint
cargo clippy --all-targets -- -D warnings

# Unit tests
cargo test --lib --all

# E2E tests
cargo test --test e2e_cli

# All checks (fmt + clippy + test)
mise run pre-commit
```

## Architecture

```
src/
├── main.rs              CLI entry point (clap)
├── lib.rs               Library exports
├── app/
│   ├── chat.rs          Interactive chat REPL
│   └── assistant_service.rs  High-level Q&A service
├── backends/
│   ├── ollama.rs        Ollama backend (Responses API + embeddings)
│   └── mock.rs          Mock backend for testing
└── core/llm/
    ├── mod.rs           ResponseModel / EmbeddingModel traits
    └── types.rs         Domain types (InputItem, OutputItem, StreamEvent, etc.)
```

**Dependency flow:** `app` → `core` ← `backends` — the app layer only knows about core traits, not specific backends.
