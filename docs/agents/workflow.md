# AFK Workflow

The development process that turns a finished plan into merged code while the
maintainer is away. This is a **process** glossary, not a product-domain one; the
product's domain language lives in [CONTEXT.md](../../CONTEXT.md). The upstream flow
is `grill-with-docs` → `to-prd` → (when the PRD is large) `decompose-prd` → the
AFK pipeline.

**AFK** is the umbrella concept for this whole away-mode pipeline. It runs as two
symmetric loops (see [ADR-0006](../adr/0006-review-afk-two-loop-architecture.md)):

- the **implementation loop** (`afk/implement.sh`, the producer) consumes
  `ready-for-agent` work — a new issue, or a PR sent back for rework — and opens a
  PR per slice;
- the **review loop** (`afk/review.sh`, the consumer) consumes `needs-review` PRs,
  reviews them, and acts on the verdict — merge, send to a human, or send back.

## Language

### Planning

**PRD**:
A single issue describing what to build and why, published by `to-prd`. When it is
small enough to implement as one slice it ships with the `ready-for-agent` label; a
large PRD becomes a **parent** and is not itself grabbable.
_Avoid_: spec, plan, epic

**parent**:
A PRD issue that has been decomposed into sub-issues. It is a control issue: it
carries no `ready-for-agent` label and the implementation loop never grabs it.
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

**implementation loop** (producer):
`afk/implement.sh`, which consumes `ready-for-agent` work one item at a time, each
in its own worktree, and opens (or reopens) a PR per slice. It never closes issues;
closing happens at PR merge. One iteration runs Step 0–3: parent completion, pick,
run agent, open/relabel PR.
_Avoid_: runner, cron

**review loop** (consumer):
`afk/review.sh`, which consumes `needs-review` PRs one at a time, runs the review
agent on each, and acts on its verdict. One iteration runs Step 1–3: pick PR, run
review agent, act on the verdict.
_Avoid_: reviewer cron

**iteration**:
One pass of a loop. An implementation iteration is: pick one grabbable issue (or
rework PR) → one worktree → implement/rework → one PR. A review iteration is: pick
one `needs-review` PR → review → act on the verdict. The atomic unit of AFK
progress.
_Avoid_: cycle, run

**grabbable**:
An issue the implementation loop will pick: it is `ready-for-agent`, has no
sub-issues of its own (`subIssues.totalCount == 0`), and all its `blocked-by`
blockers are closed. Parents are never grabbable.
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

### Review

**review agent**:
The headless agent the review loop runs on a `needs-review` PR. It has two duties —
**content correctness** (does the change implement the issue's spec?) and
**pr-format conformance** (does the PR match [pr-format.md](./pr-format.md)?). It
posts its findings as a PR comment and returns a verdict; it never merges, relabels,
or closes — the loop acts.
_Avoid_: reviewer bot

**verdict**:
The review agent's judgement, one of `mergeable`, `needs-human`, or `needs-rework`.
The pure dispatch (`review_verdict_action`) maps a verdict plus the PR's review
round to an action; the side effects (merge, label transitions) live in the loop's
control layer.
_Avoid_: decision, ruling

**send back**:
The review loop's `needs-rework` action: the PR returns to `ready-for-agent` and the
implementation loop picks it up again as **rework**, reading the review comment to
guide the fix.
_Avoid_: reject, bounce

**review round** (two-strike guard):
A PR sent back twice *in a row* is force-transitioned to `ready-for-human` rather
than ping-ponging between the producer and reviewer forever. The round is tracked on
the PR by the `afk-review-1` label after the first send-back; the second consecutive
send-back escalates. A human's touch resets the counter.
_Avoid_: attempt, retry count

### Labels

See [triage-labels.md](./triage-labels.md) for the canonical set. The ones the loops
read directly:

**ready-for-agent**:
Ready for an AFK agent — a new issue the implementation loop may grab, or a PR the
review loop has sent back for rework.
**needs-review**:
A PR that is implemented and awaits the review loop.
**ready-for-human**:
A PR that is reviewed and awaits a human's merge or override.
**afk-review-1**:
Marker set by the review loop after the first send-back; powers the two-strike
guard.

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
  branch. Neither loop ever closes an issue.
- **The branch hierarchy mirrors the issue hierarchy** (see
  [ADR-0004](../adr/0004-issue-branch-isomorphic-hierarchy.md)): single-shot PRDs are
  one layer (`issue-<N>` → `main`); decomposed PRDs are two (`issue-<child>` →
  `prd-<parent>` → `main`). A parent closes when its `prd-<parent>` branch merges to
  `main`.
- **Agents only judge; the loops act.** Side effects — opening a PR, transitioning
  labels, merging — live in the `*.sh` control layer, never in an agent prompt. The
  review loop (never an agent) performs `gh pr merge`, after CI is green.
- **Parent PRs are not special.** The final `prd-<parent>` → `main` PR flows through
  `needs-review` like any other; if its integration review is too heavy for the
  agent, it falls to `ready-for-human` via the normal verdict path.
- **Two-strike, no ping-pong.** A PR sent back twice in a row escalates to
  `ready-for-human` (see **review round**), so a stuck producer↔reviewer cycle
  always surfaces to a human.
- **"Which item to pick" stays a priority judgment, not a mechanical sort.** Each
  loop filters candidates mechanically (label, no sub-issues, blockers closed / the
  `needs-review` label) and hands them to the model to pick the single
  highest-priority one. The implementation loop picks rework PRs before new issues.

## See also

- [CONTEXT.md](../../CONTEXT.md) — product domain language (what is built).
- [issue-tracker.md](./issue-tracker.md) — how issues are tracked via `gh`.
- [triage-labels.md](./triage-labels.md) — the canonical triage labels.
- [pr-format.md](./pr-format.md) — the producer/reviewer PR contract.
- [ADR-0004](../adr/0004-issue-branch-isomorphic-hierarchy.md) — the stacked
  branch hierarchy.
- [ADR-0006](../adr/0006-review-afk-two-loop-architecture.md) — the two-loop AFK
  architecture (producer + consumer), the label state machine, and the two-strike
  guard.
