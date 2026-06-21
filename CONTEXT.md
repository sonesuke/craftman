# craftman

**craftman is a reference implementation** of a local-LLM CLI assistant that surfaces **skills** relevant to each user turn (a retrieval pattern called ToolRAG) and exposes executable **tools** as function calls. Skills conform to the [Agent Skills](https://agentskills.io/specification) standard. This file pins down the project's domain language; it is a glossary only, free of implementation detail.

## Language

**Skill**:
A unit of knowledge and instructions, written as a directory with a `SKILL.md` following the [Agent Skills](https://agentskills.io/specification) standard — YAML frontmatter (the Manifest) plus a freeform Markdown body (the Instructions). A skill is *read and followed*, never executed. Executable capabilities live in Tools, not Skills.
_Avoid_: tool, function, capability, plugin

**Tool**:
A function the model invokes by name with JSON arguments; its result is returned to the model as text. Capability tools (e.g. `calculator`) are exposed only through the skills that declare them; `load_skill` is the one always-present tool. A Skill is *read*; a Tool is *called and executed*.
_Avoid_: don't use "tool" to mean a Skill

**Harness**:
The craftman runtime that orchestrates a turn — runs ToolRAG retrieval, activates skills, loads the tools their `allowed-tools` declare, dispatches tool calls, and feeds results back to the model.
_Avoid_: runtime, loop, orchestrator

**allowed-tools**:
An Agent Skills frontmatter field listing the tools a skill may use. In craftman it is a **directive to the harness**: when the skill is active, load exactly these tools — not documentation for the model.
_Avoid_: dependencies, imports

**Manifest**:
The YAML frontmatter of a skill — at minimum a `name` and `description`. The skill's identity for discovery and retrieval.
_Avoid_: metadata, header

**Instructions**:
The freeform Markdown body of a skill, after the frontmatter. What the model follows once the skill is active. Format is unrestricted per the standard.
_Avoid_: body, content

**Progressive disclosure**:
The standard's loading discipline: a skill enters context in stages — Manifest first, Instructions on activation, bundled resources only when needed — so only what the task requires is read.
_Avoid_: lazy loading

## Naming convention

A capability may have a Skill (knowledge) and a Tool (execution), but the two never share a name. **Skills are named for the domain** they address (`arithmetic`); **Tools for the action** they perform (`calculator`). As a reference implementation, craftman keeps both for capabilities where the split is illustrative, even when one would suffice in production.
