#!/usr/bin/env bash
# afk/run.sh — AFK execution loop: one GitHub issue per worktree, via worktrunk.
#
# Consumes issues from the upstream planning flow:
#   grill-with-docs -> to-prd -> [decompose-prd] -> this script
# Each sub-issue (or single-shot PRD) carries the `ready-for-agent` label.
#
# Each iteration:
#   0. PARENT-COMPLETION SCAN: for every open parent whose sub-issues are ALL
#      closed, open the final `prd-<parent>` -> `main` PR (`Closes #<parent>`,
#      `ready-for-human`). This is how a parent reaches done.
#   1. WHICH issue to grab is Claude's call: grabbable candidates are filtered
#      mechanically (ready-for-agent, no sub-issues, all blocked-by blockers
#      closed, no open PR), then Claude picks the single highest-priority one.
#   2. run.sh ensures the parent branch exists (sub-issues target `prd-<parent>`;
#      single-shot PRDs target `main`. It opens a worktree on `issue-<N>` via
#      `wt switch`, and Claude runs headless: implement, `mise run pre-commit`,
#      commit, push. Claude does NOT open a PR — the loop does (Step 3).
#   3. run.sh opens the PR with the correct base (`prd-<parent>` or `main`),
#      labels it `ready-for-human`, and removes `ready-for-agent` from the issue.
# The issue is NOT closed here — it closes at PR merge (`Closes #N`). run.sh
# never closes an issue itself. Worktrees are left in place; clean them up with
# `wt remove pr:<N>` after merge.
#
# Branch hierarchy mirrors the issue hierarchy (ADR-0004):
#   single-shot PRD:  issue-<N>  -> main
#   decomposed PRD:   issue-<child> -> prd-<parent> -> main
#
# Usage: ./afk/run.sh <iterations>
# Run from the repo root (the main worktree). Requires `gh`, `wt`, `claude`,
# and `mise` on PATH. A non-zero Claude exit in step 2 does NOT abort the loop:
# the worktree stays for inspection and the next iteration runs.
#
# filter_grabbable() below is unit-tested by afk/tests/test_filter.sh, which
# sources this file. The main loop runs only when this file is executed
# directly (not sourced).

# --- Functions (sourceable; tested offline) ---

# Read a `gh issue list --json number,title,subIssues,blockedBy` blob on stdin;
# emit a JSON array of {number, title} for the grabbable issues only.
# An issue is grabbable iff it has no sub-issues of its own AND every one of its
# blocked-by blockers is closed (its number is in CLOSED_JSON, a JSON array).
filter_grabbable() {
  local closed_json="${CLOSED_JSON:-[]}"
  jq -c --argjson closed "$closed_json" '
    map(
      select(
        ((.subIssues.totalCount // 0) == 0)
        and
        ([.blockedBy.nodes[].number] as $blockers
         | ($blockers | length) == 0
           or ($blockers | map(. as $b | $closed | index($b) != null) | all))
      ) | {number, title}
    )
  '
}

# --- Main (only when executed, not sourced) ----------------------------

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then

set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <iterations>"
  exit 1
fi

for ((i=1; i<=$1; i++)); do
  echo "Iteration $i"
  echo "--------------------------------"

  # --- Step 0: open final parent PRs for parents whose sub-issues all closed.
  parents=$(gh issue list --state open --json number,subIssues \
    --jq '[.[] | select(.subIssues.totalCount > 0) | .number][]')
  for P in $parents; do
    all_closed=$(gh issue view "$P" --json subIssues \
      --jq '[.subIssues.nodes[].state] | (length == 0) or all(. == "CLOSED")')
    if [ "$all_closed" = "true" ] \
       && ! gh pr list --state open --head "prd-$P" --json number | grep -q '"number"'; then
      git show-ref --verify --quiet "refs/heads/prd-$P" || git branch "prd-$P" main
      git push origin "prd-$P" >/dev/null 2>&1 || true
      pr_url=$(gh pr create --base main --head "prd-$P" \
        --title "PRD #$P: all sub-issues merged" \
        --body "Closes #$P" 2>/dev/null || true)
      if [ -n "$pr_url" ]; then
        pr_num=$(printf '%s' "$pr_url" | grep -oE '[0-9]+' | tail -1)
        gh pr edit "$pr_num" --add-label ready-for-human >/dev/null 2>&1 \
          && echo "Parent #$P: opened final PR #$pr_num (prd-$P -> main), labeled ready-for-human."
      fi
    fi
  done

  # --- Step 1: filter grabbable candidates, then Claude picks one. --------
  raw=$(gh issue list --state open --label ready-for-agent \
    --json number,title,subIssues,blockedBy)
  closed_json=$(gh issue list --state closed --json number --jq '[.[].number]')
  candidates=$(printf '%s' "$raw" | CLOSED_JSON="$closed_json" filter_grabbable)

  if [ "$candidates" = "[]" ]; then
    echo "No grabbable ready-for-agent issues left, exiting."
    if command -v tt >/dev/null 2>&1; then
      tt notify "craftman: no grabbable issues left after $((i-1)) iterations"
    fi
    exit 0
  fi

  # Drop candidates whose branch already has an open PR.
  candidates=$(printf '%s' "$candidates" | \
    jq --argjson openprs "$(gh pr list --state open --json headRefName --jq '[.[].headRefName]')" '
      map(select(. as $c | $openprs | index("issue-\($c.number)") | not))')
  if [ "$candidates" = "[]" ]; then
    echo "All grabbable candidates already have open PRs, exiting."
    if command -v tt >/dev/null 2>&1; then
      tt notify "craftman: all candidates have open PRs after $((i-1)) iterations"
    fi
    exit 0
  fi

  pick=$(claude --dangerously-skip-permissions -p \
"You are choosing which issue to work on next in the craftman repo.
Grabbable ready-for-agent issues (already filtered: no sub-issues, all blockers closed, no open PR):
$candidates
Pick the single highest-priority issue you would grab next.
Reply with ONLY the issue number (e.g. 42). If nothing is grabbable, reply NONE.")

  N=$(printf '%s' "$pick" | grep -oE '[0-9]+' | head -1 || true)

  if [ -z "$N" ]; then
    echo "Claude reported no grabbable issue, exiting."
    if command -v tt >/dev/null 2>&1; then
      tt notify "craftman: no grabbable issue after $((i-1)) iterations"
    fi
    exit 0
  fi

  echo "Claude picked issue #$N -> worktree branch issue-$N"

  # --- Step 2: determine base, ensure parent branch, run headless Claude. --
  parent=$(gh issue view "$N" --json parent --jq '.parent.number // empty')
  if [ -n "$parent" ]; then
    BASE="prd-$parent"
    git show-ref --verify --quiet "refs/heads/$BASE" || git branch "$BASE" main
    # The PR base must exist on the remote, or gh pr create fails
    # ("Base ref must be a branch"). Push is idempotent.
    git push origin "$BASE" >/dev/null 2>&1 || true
    echo "Issue #$N is a sub-issue of #$parent -> PR base $BASE"
  else
    BASE="main"
    echo "Issue #$N is a single-shot PRD -> PR base $BASE"
  fi

  # Claude implements and pushes, but does NOT open a PR — the loop does.
  PROMPT="You are an AFK agent in the craftman repo. You are inside a worktree on branch issue-$N, \
dedicated to GitHub issue #$N. Work on ONLY this issue. \
1. Read issue #$N fully: gh issue view $N --comments. The issue body is the contract; if an '## Agent Brief' comment is present, treat it as authoritative. \
2. Implement the spec end to end. Use the project domain glossary and respect relevant ADRs. \
3. Verify it holds together: mise run pre-commit. \
4. Commit and push the branch issue-$N. Do NOT open a PR — the AFK loop opens it for you. \
Do NOT close the issue."

  set +e
  if git show-ref --verify --quiet "refs/heads/issue-$N"; then
    wt switch --yes -x claude "issue-$N" -- --dangerously-skip-permissions -p "$PROMPT"
  else
    wt switch --create --yes -x claude "issue-$N" -- --dangerously-skip-permissions -p "$PROMPT"
  fi
  rc=$?
  set -e
  if [ $rc -ne 0 ]; then
    echo "Iteration $i: Claude exited non-zero (rc=$rc); worktree issue-$N left for inspection. Continuing."
    continue
  fi

  # --- Step 3: open the PR with the correct base, then drop ready-for-agent.
  if ! gh pr list --state open --head "issue-$N" --json number | grep -q '"number"'; then
    pr_url=$(gh pr create --base "$BASE" --head "issue-$N" \
      --title "Closes #$N" --body "Closes #$N" 2>/dev/null || true)
    if [ -n "$pr_url" ]; then
      pr_num=$(printf '%s' "$pr_url" | grep -oE '[0-9]+' | tail -1)
      gh pr edit "$pr_num" --add-label ready-for-human >/dev/null 2>&1
      echo "Issue #$N: opened PR #$pr_num (base=$BASE), labeled ready-for-human."
    fi
  else
    echo "Issue #$N already has an open PR."
  fi
  if gh pr list --state open --head "issue-$N" --json number | grep -q '"number"'; then
    gh issue edit "$N" --remove-label ready-for-agent >/dev/null 2>&1 \
      && echo "Issue #$N: removed ready-for-agent (open PR exists)."
  else
    echo "Iteration $i: no open PR for issue-$N; leaving ready-for-agent."
  fi
done

echo "Completed $1 iterations."

fi  # end main guard
