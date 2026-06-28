# AFK Workflow

The development process that turns a finished plan into merged code while the
maintainer is away. This is a **process** glossary, not a product-domain one; the
product's domain language lives in [CONTEXT.md](../../CONTEXT.md). The upstream flow
is `grill-with-docs` → `to-prd` → (when the PRD is large) `decompose-prd` →
`afk/run.sh`.

## Language

### Planning

**PRD**:
A single issue describing what to build and why, published by `to-prd`. When it is
small enough to implement as one slice it ships with the `ready-for-agent` label; a
large PRD becomes a **parent** and is not itself grabbable.
_Avoid_: spec, plan, epic

**parent**:
A PRD issue that has been decomposed into sub-issues. It is a control issue: it
carries no `ready-for-agent` label and `afk/run.sh` never grabs it.
_Avoid_: master issue, umbrella

**sub-issue**:
A native GitHub child of a parent, created by `decompose-prd` via `--parent`. Each
sub-issue is one slice and is independently grabbable.
_Avoid_: task, child ticket

**slice** (vertical slice, tracer bullet):
The unit of decomposition — a thin vertical cut through every layer (schema → API →
UI → tests) that is demoable on its own. A slice must be completable in one
iteration **and** reviewable in one pass; if either fails, decompose further.
_Avoid_: task, horizontal slice, layer

### Execution

**AFK loop**:
`afk/run.sh`, which consumes `ready-for-agent` issues one at a time, each in its own
worktree, and opens a PR per slice. It never closes issues; closing happens at PR
merge.
_Avoid_: runner, cron

**iteration**:
One pass of the AFK loop: pick one grabbable issue → one worktree → implement → one
PR. The atomic unit of AFK progress.
_Avoid_: cycle, run

**grabbable**:
An issue the AFK loop will pick: it is `ready-for-agent`, has no sub-issues of its
own (`subIssues.totalCount == 0`), and all its `blocked-by` blockers are closed.
Parents are never grabbable.
_Avoid_: ready, available

**blocked-by**:
A native GitHub relationship set via `--add-blocked-by`. An issue is not grabbable
until every issue it is blocked-by is closed. Direction matters: the successor is
`--add-blocked-by <predecessor>`.
_Avoid_: depends-on (use the native field)

**agent brief**:
The issue body (and an authoritative `## Agent Brief` comment, when present) that the
headless agent implements against.
_Avoid_: spec, ticket body

### Labels

See [triage-labels.md](./triage-labels.md) for the canonical set. The two the AFK
loop reads directly:

**ready-for-agent**:
An issue the AFK loop may grab.
**ready-for-human**:
A PR that is implemented and awaits a human to review and merge.

### Branches

**`issue-<N>`**:
The branch and worktree for a single issue — a single-shot PRD, or one sub-issue.
**`prd-<N>`**:
The parent branch for a decomposed PRD `<N>`; sub-issue branches merge into it, and
it merges into `main`.

## Rules

- **Done is closed, and closing is a PR merge.** An issue is done when it is closed.
  A single-shot PRD and a parent close via the standard `Closes #N` when their PR
  merges to `main`. A sub-issue is closed by the `close-sub-issue-on-merge` GitHub
  Action when its PR merges into `prd-<parent>` — the standard `Closes #N` only fires
  on the default branch, so it would not close a sub-issue whose PR targets a parent
  branch. `afk/run.sh` never closes an issue.
- **The branch hierarchy mirrors the issue hierarchy** (see
  [ADR-0004](../adr/0004-issue-branch-isomorphic-hierarchy.md)): single-shot PRDs are
  one layer (`issue-<N>` → `main`); decomposed PRDs are two (`issue-<child>` →
  `prd-<parent>` → `main`). A parent closes when its `prd-<parent>` branch merges to
  `main`.
- **"Which issue to grab" stays a priority judgment, not a mechanical sort.**
  `afk/run.sh` filters grabbable candidates mechanically (label, no sub-issues,
  blockers closed) and hands them to the model to pick the single highest-priority
  one.

## See also

- [CONTEXT.md](../../CONTEXT.md) — product domain language (what is built).
- [issue-tracker.md](./issue-tracker.md) — how issues are tracked via `gh`.
- [triage-labels.md](./triage-labels.md) — the canonical triage labels.
- [ADR-0004](../adr/0004-issue-branch-isomorphic-hierarchy.md) — the stacked
  branch hierarchy.
