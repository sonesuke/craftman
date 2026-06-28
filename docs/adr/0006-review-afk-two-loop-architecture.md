# AFK as two loops — implementation (producer) and review (consumer)

## Context

The AFK loop (`afk/run.sh`) turned `ready-for-agent` issues into PRs while the
maintainer was away, but every PR it opened was a bare `Closes #N` (PR #35): no
title, no body, no handoff to whoever reviews it. Two gaps followed. (1) The
producer's knowledge — what it verified, what it deliberately left as-is — was
lost; a reviewer had to reconstruct the change from the diff. (2) Review was a
human-only step, so the loop stalled at `ready-for-human` whenever the
maintainer was away. We want both the PR *and* its review to progress while the
maintainer is away.

## Decision

Split AFK into **two symmetric loops** and give PRs a real handoff.

- **`afk/implement.sh`** (producer, the renamed `run.sh`) consumes
  `ready-for-agent` issues/PRs, runs the implementation agent, and opens a PR.
  The implementation agent authors the PR title and body (`.afk/pr_title`,
  `.afk/pr_body`, gitignored) per
  [pr-format.md](../../docs/agents/pr-format.md); Step 3 reads them and opens
  the PR — never a bare `Closes #N`.
- **`afk/review.sh`** (consumer) consumes `needs-review` PRs, runs the review
  agent, and acts on its verdict. The review agent has two duties: **content
  correctness** and **pr-format conformance**.

**Label state machine.** `needs-review` (new) = implemented, awaiting review.
`ready-for-human` is redefined to = reviewed, awaiting a human's merge or
override. `ready-for-agent` is shared by issues (new work) and PRs (sent-back
rework) — rework returns here, and the implementation loop picks it up again.

**Verdict → action** (in the consumer loop):

- mergeable → the control layer (`review.sh`) merges after CI is green;
- needs a human → `ready-for-human`;
- needs rework → `ready-for-agent` (sent back to the producer).

**Side effects stay with the control layer; agents only judge.** Both loops
follow the rule already established for PR opening: the agent decides, the
`*.sh` loop acts (open PR, transition labels, merge). No agent holds merge or
PR-open authority.

**Two-strike loop guard.** A PR sent back twice in a row is force-transitioned
to `ready-for-human` rather than ping-ponging forever. A human's touch resets
the counter.

**Parent PRs are not special.** The final `prd-<parent>` → `main` PR goes
through `needs-review` like any other; if the integration review is too much
for the agent, it falls to `ready-for-human` via the normal verdict path.

**Naming.** "AFK" is promoted to the umbrella concept (the whole away-mode
pipeline); the two loops are the **implementation loop** and the **review
loop**.

## Considered options

- **One unified loop** that scans both `ready-for-agent` and `needs-review` and
  dispatches by work type: rejected. The two loops have different pick sources,
  agent roles, and side effects; unifying adds a dispatch layer with no payoff
  and reshapes the existing `run.sh` (Step 0–3, entry/steps/lib) for no gain.
- **Agent opens/merges the PR directly** (side effects inside the agent):
  rejected. It scatters authority into prompts, breaks the symmetry with the
  existing "loop opens the PR" rule, and puts merge authority in an agent.
- **Keep `ready-for-human` = "implemented, awaits review"** (no `needs-review`):
  rejected. There is no state between implemented and human-reviewed, so review
  cannot be handed to an agent without relabelling — the two-tier label is the
  whole point.
- **Bare `Closes #N` fallback** when the producer didn't write the files:
  rejected. That *is* the #35 bug. If the files are missing, re-run the
  producer.

## Consequences

- Renames `afk/run.sh` → `afk/implement.sh` and adds `afk/review.sh`; the
  existing entry/steps/lib split (ADR-0005, issue #32) carries to the review
  loop, sharing `afk/lib/`.
- The label set grows by one (`needs-review`) and a round-marker (`afk-review-1`);
  `ready-for-human`'s meaning narrows. `docs/agents/workflow.md` and
  `triage-labels.md` move with it.
- The implementation loop picks up rework: a PR the review loop sends back to
  `ready-for-agent` is reworked and returned to `needs-review`, so the
  two-strike guard can actually fire.

## Status

**accepted** — the producer-side handoff (`.afk/pr_title`, `.afk/pr_body`,
step2/step3) landed in PR #37 (issue #38); the review loop (`afk/review.sh`),
the rename to `implement.sh`, the `needs-review` label, and the two-strike guard
landed in PR #39 (issue #39).
