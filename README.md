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

### How it works (skill retrieval)

Inspired by [TinyAgent](https://github.com/SqueezeAILab/TinyAgent)'s ToolRAG, craftman's skill retrieval surfaces only the skills relevant to each message instead of handing the model the whole catalog:

1. **Indexing** — At startup, skill `name` + `description` are read and indexed
2. **Retrieval** — Each turn, the skills most relevant to the user's message are ranked with BM25 (matches on `name` weighted above `description`) and injected into the request, so the model never reasons over irrelevant skills
3. **Activation** — The model calls `activate_skill("skill-name")` to pull in the full SKILL.md instructions for the one it needs

Retrieval is dependency-free keyword matching (BM25) — no embedding model required. The engine lives in [`src/core/retriever.rs`](src/core/retriever.rs) and can be swapped for an embedding-based retriever later.

### Example Skill

```
skills/arithmetic/
└── SKILL.md
```

```markdown
---
name: arithmetic
description: Guidance for evaluating arithmetic expressions. Enables the calculator tool.
allowed-tools: calculator
---

# Arithmetic

When the user asks to evaluate an arithmetic expression, use the `calculator` tool with the expression exactly as given.
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
│   ├── chat.rs          Interactive chat REPL: per-turn skill retrieval + activate_skill
│   └── assistant_service.rs  High-level Q&A service
├── backends/
│   ├── ollama.rs        Ollama backend (Responses API + embeddings + streaming)
│   └── mock.rs          Mock backend for testing
└── core/
    ├── llm/
    │   ├── mod.rs       ResponseModel / EmbeddingModel traits
    │   └── types.rs     Domain types (InputItem, OutputItem, StreamEvent, etc.)
    ├── retriever.rs     BM25 skill retrieval engine (dependency-free)
    └── skill/
        ├── mod.rs       SkillRegistry with skill retrieval + activate_skill
        ├── loader.rs    SKILL.md parser
        └── types.rs     SkillManifest, Skill
```

**Dependency flow:** `app` → `core` ← `backends` — the app layer only knows about core traits, not specific backends.

<!-- afk smoke test -->
