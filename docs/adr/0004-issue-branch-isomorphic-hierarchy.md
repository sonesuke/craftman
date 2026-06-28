# Mirror the issue hierarchy in the branch hierarchy (stacked parent branch)

## Context

The AFK workflow decomposes a large PRD into native GitHub sub-issues. The parent
PRD is a **control issue**: `afk/implement.sh` never grabs it (it carries no
`ready-for-agent` label and has sub-issues), so no branch, worktree, or PR is ever
created for it. That left the parent's "done" undefined — the single rule the
workflow follows ("done = PR merge = issue close, via `Closes #N`") had no PR to hang
the parent's close on. Closing the parent through an exceptional path (a GitHub
Action, a manual step, or never closing it) would either break that rule or leave
"is this PRD finished?" unqueryable.

## Decision

Make the branch hierarchy **isomorphic to the issue hierarchy**, so every issue
closes on the merge of its own PR. The depth of the branch stack always equals the
depth of the issue tree:

- **Single-shot PRD (1 layer):** branch `issue-<N>` → PR to `main` (`Closes #N`).
- **Decomposed PRD (2 layers):** each sub-issue is `issue-<child>` → PR to
  `prd-<parent>` (`Closes #<child>`). Once all sub-issues are closed, `prd-<parent>`
  → PR to `main` (`Closes #<parent>`).

`afk/implement.sh` creates `prd-<parent>` lazily on the first sub-issue it grabs, and opens
the final parent PR when it detects that all of a parent's sub-issues are closed.
Closing is always a PR merge; `afk/implement.sh` never closes an issue itself.

Because GitHub's `Closes #N` only fires on a merge to the default branch, a
sub-issue's PR (which targets `prd-<parent>`, not `main`) does NOT auto-close its
issue. The `close-sub-issue-on-merge` GitHub Action closes the sub-issues named in
the PR body when a PR merges into a `prd-*` branch. The parent, by contrast, closes
via the standard `Closes #<parent>` when its `prd-<parent>` → `main` PR merges.

## Considered options

- **Sub-issue PRs straight to `main` (single layer):** rejected — the parent has no
  PR, so its close needs an exceptional path (a GitHub Action or a manual step),
  splitting "done" into two mechanisms and breaking "close = PR merge".
- **GitHub Action that auto-closes the parent when all sub-issues close:** rejected —
  the parent closes via its own `prd-<parent>` → `main` PR merge (standard
  `Closes #<parent>`), so no Action is needed for the parent. (An Action IS used to
  close *sub-issues* — see Decision — because their PRs target `prd-<parent>`, where
  the standard mechanism does not fire.)
- **Never close the parent (leave it open as a progress tracker):** rejected — "is
  this PRD done?" must be answerable from the parent's `closed` state, queryably.

## Consequences

- `afk/implement.sh` gains parent-branch management and final-parent-PR automation
  (detect all-sub-issues-closed → open the `prd-<parent>` → `main` PR with
  `needs-review`).
- Sub-issue PRs target `prd-<parent>`, which advances as sub-issues merge; later
  sub-issues may need to rebase onto the updated parent branch.
- Review is two-tier: each sub-issue PR is reviewed, then the final parent PR is a
  full code review (not merely an integration gate).
- Single-shot PRDs are unaffected — they remain a single layer straight to `main`.
- A GitHub Action (`close-sub-issue-on-merge`) closes sub-issues when their PR merges
  into a `prd-*` branch, because the standard `Closes #N` does not fire for
  non-default-branch merges. The parent still closes via the standard mechanism on
  its `prd-<parent>` → `main` merge.
