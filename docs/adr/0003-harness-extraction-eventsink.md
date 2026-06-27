# Extract Harness as a module; separate CLI and observability via EventSink

## Context

The agent logic — skill retrieval, tool exposure/dispatch, LLM round-trips, and session state — lived inside `chat::run()` and `chat::send_turn()`, tangled with the CLI (REPL, commands, display) and coupled to the concrete `OllamaBackend`. This caused:

- `send_turn` depended on `OllamaBackend` directly, so it could not be driven by a mock — the skill-retrieval → `activate_skill` → `calculator` chain could not be tested deterministically. The model's probabilistic choice to call `activate_skill` is not the test subject; the harness logic is.
- Two agent paths existed: `AssistantService` (plain Q&A, no skills/tools) and `send_turn` (full chain), splitting responsibility.
- Observability was split: `--log` dumped raw HTTP on `OllamaBackend` only, and `chat.rs` emitted ad-hoc `eprint!` strings — neither surfaced the harness's meaningful events (retrieval, activation, tool exposure/result) as structured data, and no metrics (rounds, tokens, latency) were available.

## Decision

Extract a single `Harness<L: ResponseModel>` module (`src/core/harness.rs`) that owns the agent responsibility, and use an `EventSink` trait as the narrow boundary to the CLI and observability:

- **Harness owns session state** (`history`, `active_skills`) and exposes `new`, `submit`, `clear`, `active_skills`.
- **`submit(user_msg, sink)`** runs one turn (retrieval → activation → tool chain → answer) and emits `HarnessEvent`s to the sink.
- **`EventSink` trait** is the boundary; the CLI implements a display sink, a logger implements a structured-log sink. Harness fans out to registered sinks.
- **`ResponseModel` trait gains `stream_create_response`**, so `Harness<L: ResponseModel>` is generic and mock-drivable.
- **`AssistantService` is removed**; Harness subsumes it (turns with no retrieved skill behave as plain Q&A).
- **Logger sink records structured events** (`SkillRetrieved`, `SkillActivated`, `ToolsExposed`, `ToolCalled`, `ToolResult`, `TurnComplete`) as JSONL; deltas are not recorded. `OllamaBackend`'s raw HTTP dump is removed — observability becomes Harness-derived and single-source.

## Considered options

- **Keep agent logic in `chat.rs`**: rejected — CLI/agent coupling blocks mock testing and blocks reusing the agent behind other front-ends.
- **async `Stream<Item=HarnessEvent>` as the boundary**: rejected — `Pin`/self-referential complexity; `EventSink` callback extends the existing `stream_create_response(req, |event| ...)` pattern and is idiomatic in Rust.
- **channel (mpsc) boundary**: rejected — adds task-splitting and complicates state ownership.
- **Keep `AssistantService` as a lightweight path**: rejected — Harness with no retrieved skill already behaves as plain Q&A, so the second path is redundant and splits responsibility.
- **Keep raw HTTP dump as the log**: rejected — it is backend-specific and unstructured; structured `HarnessEvent`s subsume it for observation (a raw debug dump can return as a separate `--dump-raw` option if needed later).
