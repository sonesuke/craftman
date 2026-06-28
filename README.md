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
      --log <LOG>                Log structured harness events to this file (JSONL format)
  -h, --help                     Print help
  -V, --version                  Print version
```

`--log` writes one JSON object per line for each meaningful step of a turn — skill retrieval, activation, tool calls and results, turn completion (with token usage) — so you can observe what the assistant did.

### Chat Commands

| Command | Description |
|---------|-------------|
| `/quit`, `/exit` | Exit the chat |
| `/clear` | Clear conversation history |
| `/skills` | List loaded skills |

## Skills

A **Skill** is a unit of knowledge: a directory containing a `SKILL.md` file with YAML frontmatter (the manifest) and a Markdown body (the instructions), following the [Agent Skills](https://agentskills.io/specification) format.

### How it works

For each message, craftman surfaces only the skills that matter instead of handing the model the whole catalog:

1. **Skill retrieval** — the skills whose `name` + `description` best match your message are ranked and shown to the model (dependency-free keyword matching — no embedding model required).
2. **Activation** — the model calls `activate_skill("name")` to pull the full instructions for the one it needs into context.
3. **Tool execution** — an active skill declares the tools it may use (`allowed-tools`); those become callable, and the model invokes them to act.

### Skills vs. Tools

| | Skill | Tool |
|---|---|---|
| What it is | Knowledge (instructions) | Execution (a function) |
| How the model uses it | Read | Called with arguments; result returns as text |
| Example | `arithmetic` (when/how to compute) | `calculator` (does the computation) |

A capability may have both: a **Skill** for the knowledge and a **Tool** for the action. They never share a name — **Skills are named for the domain** (`arithmetic`), **Tools for the action** (`calculator`) — so there are no collisions.

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

1. Create a directory under `skills/` named after the domain (e.g. `skills/my-skill/`).
2. Add a `SKILL.md` with YAML frontmatter (`name` + `description` at minimum) and a Markdown body of instructions.
3. If the skill needs a tool, list it under `allowed-tools` — the harness exposes exactly those tools when the skill is active.

```markdown
---
name: my-skill
description: What the skill does and when to use it
---

# Instructions

Step-by-step instructions for the LLM to follow when using this skill.
```

Skill name rules (per spec): lowercase letters, digits, and hyphens only. 1–64 characters.

## Documentation

This README is a conceptual entry point. For deeper detail, the docs split into five roles (the code is the source of truth; these stay subordinate to it):

| Doc | Role | What it covers |
|-----|-------|----------------|
| This README | entry | conceptual introduction for users |
| [`CONTEXT.md`](CONTEXT.md) | glossary | the project's domain language |
| [`docs/architecture.md`](docs/architecture.md) | structure | file tree, layer dependencies, Harness/EventSink, building & testing |
| [`docs/adr/`](docs/adr/) | decisions | architectural decision records |
| [`docs/agents/`](docs/agents/) | ops | agent workflow, issue triage, the AFK development loop |
