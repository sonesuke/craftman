# afk/steps/step2_run_agent.sh — Step 2 of the AFK iteration.
#
# Resolve the PR base (prd-<parent> for a sub-issue, else main), ensure the
# parent branch exists on the remote, then run headless Claude on the issue-<N>
# worktree to implement, pre-commit, commit, and push. Claude does NOT open a PR
# — the loop does (Step 3).
#
# Returns non-zero when Claude exits non-zero, so the caller can skip the PR
# step (|| continue) and leave the worktree for inspection. Reads the global N;
# sets the global BASE. Side-effecting: calls gh/git/wt/claude.
step_run_agent() {
  local parent PROMPT rc
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
    return 1
  fi
  return 0
}
