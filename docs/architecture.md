# Architecture

This is craftman's **structure** doc — where things live and how the layers fit together. It is the counterpart to the [README](../README.md) (concept) and [`CONTEXT.md`](../CONTEXT.md) (glossary): the README stays conceptual, while this file carries the structural detail a contributor needs to navigate the code without reverse-engineering it.

For the *why* behind specific choices, see the [ADRs](adr/). For domain language, see [`CONTEXT.md`](../CONTEXT.md).

## File tree

```
skills/                          ← SKILL.md definitions (Agent Skills format)
└── arithmetic/
    └── SKILL.md

src/
├── main.rs              CLI entry point (clap): parses --model/--url/--skills-dir/--log,
│                        calls app::chat::run
├── lib.rs               Declares app/backends/core/tools modules; re-exports key types
│                        (Harness, EventSink, HarnessEvent, ResponseModel, …)
│
├── app/                 Application layer — the CLI front-end and event observers
│   ├── mod.rs
│   ├── chat.rs          Interactive chat REPL (run) + CliSink: renders HarnessEvents
│   │                    to the terminal. Owns only I/O and commands; agent logic is
│   │                    in the Harness.
│   └── logging.rs       JsonlLogger — an EventSink that appends each structured event
│                        as one JSON line (the --log sink; deltas skipped)
│
├── core/                Domain core — knows nothing about specific backends or the CLI
│   ├── mod.rs
│   ├── harness.rs       Harness<L: ResponseModel>, HarnessEvent, EventSink — orchestrates
│   │                    a turn (skill retrieval → activation → tool chain → answer)
│   ├── retriever.rs     Dependency-free BM25 skill-retrieval engine
│   ├── llm/
│   │   ├── mod.rs       ResponseModel / EmbeddingModel traits (the seam backends implement)
│   │   └── types.rs     Domain types: InputItem, OutputItem, StreamEvent, TokenUsage,
│   │                    ToolDefinition, Role, …
│   └── skill/
│       ├── mod.rs       SkillRegistry: indexing, BM25 retrieval, activate_skill
│       ├── loader.rs    SKILL.md parser (YAML frontmatter → manifest, body → instructions)
│       └── types.rs     SkillManifest, Skill
│
├── backends/            Concrete ResponseModel implementations (depend on core traits)
│   ├── mod.rs
│   ├── ollama.rs        Ollama backend (Responses API + streaming)
│   └── mock.rs          Scripted mock backend for deterministic tests
│
└── tools/               Executable tools exposed to the model via skills' allowed-tools
    ├── mod.rs           Tool trait + ToolRegistry
    └── calculator.rs    calculator tool (hand-rolled expression evaluator)
```

## Layer dependencies

```
app ──▶ core ◀── backends
```

The intended layering is `app` → `core` ← `backends`: both the application layer and the backend implementations sit on top of the stable `core` traits, and `core` itself depends on neither.

- **`core`** holds the agent logic. The [`Harness`](#the-harness) is generic over `ResponseModel`, so it depends on **no concrete backend** and can be driven by a mock in tests ([ADR-0003](adr/0003-harness-extraction-eventsink.md)). It does consume `tools` (the Harness owns a `ToolRegistry`).
- **`backends`** implement the `core::llm::ResponseModel` / `EmbeddingModel` traits and depend on `core` only.
- **`app`** is the composition root: `chat.rs` constructs a concrete `OllamaBackend`, the `SkillRegistry`, and the `ToolRegistry`, then injects them into the generic `Harness`. This is the one place a concrete backend is named; the agent logic above it stays backend-agnostic.

Where to add code: new **agent behavior** goes in `core/harness.rs`; a new **backend** implements `core::llm::ResponseModel` and is wired up in `app/chat.rs`; new **CLI or observability** concerns go in `app/` (as an `EventSink`); new **tools** go in `tools/` and are registered in `app/chat.rs`.

## The Harness

`Harness<L: ResponseModel>` (in `core/harness.rs`) is the agent runtime. It owns session state (the message `history` and the set of `active_skills`) and runs one turn per `submit(user_msg)`:

1. **Skill retrieval** — the user's message is scored against every indexed skill's manifest (`name` + `description`); the top results are injected into the request so the model only reasons over relevant skills.
2. **Round-trip loop** — the harness calls the backend, exposing `activate_skill` plus the union of `allowed-tools` across active skills. If the model calls `activate_skill`, that skill's instructions enter context and its tools become exposed; any other tool call is dispatched and its result fed back.
3. **Answer** — when the model replies with no tool calls, the turn ends and a `TurnComplete` event (with token usage) is emitted.

A turn with no retrieved skill behaves as plain Q&A — there is no separate assistant path ([ADR-0003](adr/0003-harness-extraction-eventsink.md)).

## EventSink

Each step of a turn is emitted as a `HarnessEvent` to every registered `EventSink` — the boundary between the harness and the outside world. The CLI implements a display sink (`CliSink` in `app/chat.rs`) that renders reasoning, tool activity, and the answer to the terminal; the logger (`JsonlLogger` in `app/logging.rs`) records the structured events as JSONL via `--log`. High-frequency deltas (`ReasoningDelta`, `AssistantDelta`) are skipped by the logger so the log stays focused on the meaningful steps: retrieval, activation, tool calls and results, and turn completion. See [ADR-0003](adr/0003-harness-extraction-eventsink.md).

The `HarnessEvent` variants: `SkillRetrieved`, `SkillActivated`, `ToolsExposed`, `ToolCalled`, `ToolResult`, `ReasoningDelta`, `AssistantDelta`, `TurnComplete`.

## Skill retrieval (BM25)

Skill retrieval ([ADR-0001](adr/0001-skill-retrieval-bm25-not-embeddings.md)) is dependency-free lexical retrieval over each skill's manifest — no embedding model needs to be pulled into Ollama, and the retriever sits behind a small interface so a dense retriever can replace it later.

Each skill is modelled as two fields — a short `name` and a longer `description` — and a query is scored as:

```
score = W_NAME * bm25(name) + W_DESC * bm25(description)        # name hit outweighs description hit
```

with standard BM25 parameters (`k1 = 1.2`, `b = 0.75`, `W_NAME = 2.0`, `W_DESC = 1.0`) and a Lucene-style IDF that stays positive. Tokenization lowercases, splits on non-alphanumerics, and drops stopwords and tokens shorter than 2 characters. The engine returns up to `top_k` positive-score hits, best first, ties broken by document index. The engine lives in `core/retriever.rs`; the per-turn top-K (currently 3) is set in `core/harness.rs`.

> The `EmbeddingModel` trait and the Ollama `/api/embed` implementation are **deliberately unused** — kept so a dense retriever can drop in later. They are not dead code ([ADR-0001](adr/0001-skill-retrieval-bm25-not-embeddings.md)).

## Building & testing

```bash
# Format check
cargo fmt --all -- --check

# Lint
cargo clippy --all-targets -- -D warnings

# Unit tests
cargo test --lib --all

# E2E tests
cargo test --test e2e_cli

# All checks (fmt + clippy + unit tests)
mise run pre-commit
```
