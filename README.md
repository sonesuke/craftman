# craftman

A CLI assistant powered by local LLMs via [Ollama](https://ollama.com), with an extensible skill system based on the [Agent Skills](https://agentskills.io/specification) open standard.

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
      --model <MODEL>            Ollama model to use [default: LiquidAI/lfm2.5-1.2b-instruct]
      --url <URL>                Ollama base URL [default: http://host.docker.internal:11434]
      --skills-dir <SKILLS_DIR>  Directory containing skill definitions (SKILL.md) [default: ./skills]
  -h, --help                     Print help
  -V, --version                  Print version
```

### Chat Commands

| Command | Description |
|---------|-------------|
| `/quit`, `/exit` | Exit the chat |
| `/clear` | Clear conversation history |
| `/skills` | List loaded skills |

## Skills

Skills extend the assistant with specialized knowledge using the [Agent Skills](https://agentskills.io/specification) format. Each skill is a directory containing a `SKILL.md` file with YAML frontmatter (metadata) and Markdown body (instructions).

### How it works (Progressive Disclosure)

1. **Discovery** — At startup, only skill `name` + `description` are loaded
2. **Activation** — When the LLM decides a skill is relevant, it calls `load_skill("skill-name")`
3. **Execution** — The full SKILL.md instructions are injected into the conversation context, and the LLM follows them

### Example Skill

```
skills/calculator/
└── SKILL.md
```

```markdown
---
name: calculator
description: Evaluate mathematical expressions and return the result
---

# Calculator

Evaluates a mathematical expression and returns the numeric result.

## Parameters
- expression (string, required): The expression to evaluate, e.g. "2 + 3 * 4"

## Examples
- "1 + 1" → "2"
- "2 * (3 + 4)" → "14"
```

### Creating a Skill

Create a directory under `skills/` with a `SKILL.md`:

```markdown
---
name: my-skill
description: What the skill does and when to use it
---

# Instructions

Step-by-step instructions for the LLM to follow when using this skill.
```

Skill name rules (per spec): lowercase letters, digits, and hyphens only. 1–64 characters.

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
skills/                          ← SKILL.md definitions (agentskills.io format)
└── calculator/
    └── SKILL.md

src/
├── main.rs              CLI entry point (clap)
├── lib.rs               Library exports
├── app/
│   ├── chat.rs          Interactive chat REPL with load_skill tool calling
│   └── assistant_service.rs  High-level Q&A service
├── backends/
│   ├── ollama.rs        Ollama backend (Responses API + embeddings + streaming)
│   └── mock.rs          Mock backend for testing
└── core/
    ├── llm/
    │   ├── mod.rs       ResponseModel / EmbeddingModel traits
    │   └── types.rs     Domain types (InputItem, OutputItem, StreamEvent, etc.)
    └── skill/
        ├── mod.rs       SkillRegistry with load_skill tool support
        ├── loader.rs    SKILL.md parser
        └── types.rs     SkillManifest, Skill
```

**Dependency flow:** `app` → `core` ← `backends` — the app layer only knows about core traits, not specific backends.
