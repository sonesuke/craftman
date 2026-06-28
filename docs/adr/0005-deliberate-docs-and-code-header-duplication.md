# Deliberate duplication between process docs and the implement.sh code header

## Context

`afk/implement.sh` and the process documentation (`docs/agents/workflow.md`, ADR-0004)
describe overlapping facts about the AFK loop: the Step 0–3 sequence, the branch
hierarchy (`issue-<N>` → `prd-<parent>` → `main`), and the rule that the loop
never closes an issue. Before the entry/steps/lib split this content was
**triplicated** — the 37-line `implement.sh` header restated the full architecture
rationale already in docs, and the three copies had begun to drift.

The split (issue #32) removed the bulk of that duplication by shrinking the
header to a one-paragraph summary, a per-step table of contents, usage, and a
pointer to `docs/agents/workflow.md`. But a **small, irreducible overlap
remains by design**: the header still restates, in one line each, what the four
steps do and that closing happens at PR merge. Eliminating even that would leave
the header as a bare pointer, which is hostile to a maintainer who opens the
script and needs to know what it does at a glance.

This is the one surprising decision in the refactor worth recording: most
projects pick a single source of truth (DRY), and a future reader could
reasonably "fix" the residual duplication by deleting one view.

## Decision

Keep **two deliberately overlapping views** of the same facts, scoped to their
audience:

- **Docs (`docs/agents/workflow.md`, ADR-0004) are authoritative for the
  *why/architecture***: the process vocabulary, why the loop never closes
  issues, why branches mirror the issue hierarchy. This is the single place to
  update reasoning and the only place rationale lives.
- **The `implement.sh` header is authoritative for the *what/usage* of this one
  file**: a one-paragraph statement that one iteration runs Step 0→3 in order, a
  one-line table of contents per step, how to run it, prerequisites, the
  sourceable-test note, and a single pointer into the docs. It states only what
  *this file* does, not why the workflow is shaped that way.

The overlap (a brief restatement of the step sequence and the close-at-merge
rule) is **intentional, not a DRY violation to eliminate**. Each view is
optimized for a different reader (a maintainer reading code vs. a reader of
process docs) and neither is derived from the other.

## Considered options

- **Single source of truth — header is a bare pointer, no summary:** rejected.
  A maintainer who opens `implement.sh` needs a one-glance picture of the iteration
  without context-switching to docs; a pointer alone makes the script
  unreadable in isolation.
- **Single source of truth — keep all rationale in the header, delete the
  docs:** rejected. The architecture rationale and the process vocabulary
  belong with the other agent/process docs and ADRs, read by a different
  audience; a code header is the wrong home for "why the workflow is shaped this
  way", and a header is a poor glossary.
- **Auto-generate the header from the docs (true single source):** rejected as
  over-engineering. The overlap is small and changes only when the file's own
  behavior changes; a generation pipeline costs more than the drift it prevents.

## Consequences

- Two places describe the few facts that overlap (the step sequence, the
  close-at-merge rule). The drift surface is minimized — the header carries only
  a summary plus a pointer, and docs remain the sole home of rationale — but it
  is not zero. When the loop's *behavior* changes, both the header summary and
  the docs are updated together; when only the *reasoning* changes, only docs
  are touched.
- A future "DRY cleanup" that deletes one of the two views should be **resisted**:
  this ADR is the stop-sign. Deleting the header summary strands code readers;
  deleting the docs removes the only authoritative home of the process
  vocabulary and rationale.
- This decision is scoped to `implement.sh` ↔ process docs. It is not a license to
  duplicate freely elsewhere; it records a specific, bounded trade-off made for
  reader ergonomics at the code/docs boundary.
