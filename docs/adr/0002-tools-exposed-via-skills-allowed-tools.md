# Tools are exposed only via skills, driven by `allowed-tools`

Tools are never globally present to the model. Skill retrieval retrieves **skills** only; a skill declares the tools it needs with the Agent Skills `allowed-tools` field, and the **harness** exposes exactly those tools when the skill is activated via `activate_skill`. The exposed tool set is the union of the `allowed-tools` of all active skills, and only `activate_skill` itself is always present. This keeps skill retrieval a single, skill-centric mechanism, treats `allowed-tools` as the harness directive it was designed to be, and ensures a tool is callable only once the skill whose knowledge guides its use is in context.

## Considered options

- **Always-present tools** (every registered tool in every request): rejected — decouples tools from skills and is incoherent with skill-centric retrieval.
- **Skill retrieval retrieves tools directly** (TinyAgent-faithful): rejected — it would then retrieve two kinds of things; the intent is that it retrieves skills only.
- **Expose at retrieval** (surface a skill's tools before it is activated): rejected — exposes a tool before the skill's guiding knowledge is in context. Exposure happens at **activation** instead.
