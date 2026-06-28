---
name: decompose-prd
description: Internal — break a PRD into native GitHub sub-issues (vertical slices). Activated from to-prd, not user-invocable.
user-invocable: false
---

# Decompose PRD

Break a large PRD into native GitHub sub-issues using vertical slices (tracer bullets). This skill is **internal**: it is activated by `to-prd` after it decides the PRD is too large to ship as a single issue. It is not user-invocable.

The parent PRD issue already exists (created by `to-prd`); you receive its number as input.

The issue tracker and triage label vocabulary should have been provided to you — run `/setup-matt-pocock-skills` if not.

## Process

### 1. Gather context

The parent PRD issue number is given. Read its full body and comments: `gh issue view <parent> --comments`. The PRD body is the contract — its Problem Statement, Solution, User Stories, Implementation Decisions, and Out of Scope define what the slices must cover.

### 2. Explore the codebase (optional)

If you have not already explored the codebase, do so to understand the current state. Slice titles and descriptions should use the project's domain glossary vocabulary, and respect ADRs in the area you're touching.

Look for opportunities to prefactor the code to make the implementation easier. "Make the change easy, then make the easy change."

### 3. Draft vertical slices

Break the PRD into **tracer bullet** sub-issues. Each sub-issue is a thin vertical slice that cuts through ALL integration layers end-to-end, NOT a horizontal slice of one layer.

A slice must satisfy BOTH:

- **Completable in one iteration** — a narrow but COMPLETE path through every layer (schema, API, UI, tests), demoable/verifiable on its own.
- **Reviewable in one pass** — small enough that a human can review its PR in a single sitting. If a slice is too big to review at once, split it.

Any prefactoring should be its own slice, done first.

### 4. Quiz the user

Present the proposed breakdown as a numbered list. For each slice, show:

- **Title**: short descriptive name
- **Blocked by**: which other slices (if any) must complete first
- **User stories covered**: which of the parent PRD's user stories this slice addresses

Ask the user:

- Does the granularity feel right? (too coarse / too fine)
- Are the dependency relationships correct?
- Should any slices be merged or split further?

Iterate until the user approves the breakdown. Every user story in the PRD should be covered by at least one slice — flag any gap.

### 5. Publish the sub-issues to the issue tracker

Publish slices in **dependency order** (blockers first) so each successor can reference its predecessor's real issue number.

For each slice:

1. Create the sub-issue as a child of the parent, with the `ready-for-agent` label:
   ```
   gh issue create --title "<title>" --body "<body>" --parent <parent> --add-label ready-for-agent
   ```
2. If it is blocked by a predecessor (already created), add the relationship — the **successor** is `--add-blocked-by <predecessor>`:
   ```
   gh issue edit <successor> --add-blocked-by <predecessor>
   ```

The parent already carries **no** `ready-for-agent` (`to-prd` set it that way); do not add it. Each sub-issue gets `ready-for-agent` so the AFK loop can grab it. A sub-issue whose blockers are not yet closed is still `ready-for-agent` — the AFK loop's grab filter skips it until its blockers close.

Use the issue body template below.

<issue-template>

## What this slice delivers

A concise description of what this one slice delivers end-to-end (the behavior, not layer-by-layer implementation). This slice is a thin vertical cut of the parent PRD; describe only the part of the PRD's Solution that this slice covers.

Avoid specific file paths or code snippets — they go stale fast. Exception: if a prototype produced a snippet that encodes a decision more precisely than prose can (state machine, reducer, schema, type shape), inline it here and note briefly that it came from a prototype. Trim to the decision-rich parts — not a working demo, just the important bits.

## Acceptance criteria

- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

Concrete, testable conditions. The AFK agent implements until these hold.

## Out of scope

What this slice explicitly does NOT do (e.g. "DB migration is deferred to the next slice", "OAuth is excluded this time"). This is slice-specific scope, not the parent PRD's overall out-of-scope.

## Covers user stories

The parent PRD user-story numbers this slice covers (e.g. #2, #5).

</issue-template>

Do NOT close or modify the parent issue.
