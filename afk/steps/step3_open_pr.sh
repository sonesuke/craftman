# afk/steps/step3_open_pr.sh — Step 3 of the AFK iteration.
#
# For NEW work: open the PR for issue-<N> against the correct base (BASE,
# resolved in Step 2) if none is open yet, using the title and body the producer
# agent authored in .afk/pr_title and .afk/pr_body inside the issue-<N> worktree.
# If either file is missing, re-run the agent (step_run_agent) to author them —
# never fall back to a bare "Closes #N" (that is the #35 failure mode). For
# REWORK: the PR #PR_NUM already exists — refresh its title/body from the
# re-authored files and send it back to review.
#
# Either way the PR lands on needs-review (implemented, awaiting review) and
# ready-for-agent is dropped from the issue once a PR exists. The issue is NOT
# closed here — it closes at PR merge (Closes #N). Reads the globals N, BASE,
# REWORK, PR_NUM. Side-effecting: calls gh/git/claude.
step_open_pr() {
  local pr_url pr_num title body wt

  # --- Rework: refresh the existing PR and send it back to review. --------
  if [ "${REWORK:-0}" = "1" ]; then
    wt=$(git worktree list --porcelain | WORKTREE_BRANCH="issue-$N" worktree_path_for_branch)
    if [ -n "$wt" ] && [ -f "$wt/.afk/pr_title" ] && [ -f "$wt/.afk/pr_body" ]; then
      title=$(cat "$wt/.afk/pr_title")
      body=$(cat "$wt/.afk/pr_body")
      gh pr edit "$PR_NUM" --title "$title" --body "$body" >/dev/null 2>&1 \
        && echo "Rework #$N: refreshed PR #$PR_NUM title/body."
    else
      echo "Iteration $i: rework PR #$PR_NUM title/body missing; leaving PR as-is."
    fi
    # Back to review. Keep afk-review-1 so the two-strike guard keeps tracking.
    gh pr edit "$PR_NUM" --remove-label ready-for-agent >/dev/null 2>&1 || true
    gh pr edit "$PR_NUM" --add-label needs-review >/dev/null 2>&1 \
      && echo "Rework #$N: PR #$PR_NUM sent back to review (needs-review)."
    return 0
  fi

  # --- New work: open the PR from the producer-authored title/body. -------
  if gh pr list --state open --head "issue-$N" --json number | grep -q '"number"'; then
    echo "Issue #$N already has an open PR."
  else
    wt=$(git worktree list --porcelain | WORKTREE_BRANCH="issue-$N" worktree_path_for_branch)
    if [ -z "$wt" ] || [ ! -f "$wt/.afk/pr_title" ] || [ ! -f "$wt/.afk/pr_body" ]; then
      echo "Issue #$N: PR title/body not authored in worktree; re-running agent to write them."
      if step_run_agent; then
        wt=$(git worktree list --porcelain | WORKTREE_BRANCH="issue-$N" worktree_path_for_branch)
      else
        echo "Iteration $i: agent re-run failed; leaving worktree for inspection."
      fi
    fi
    if [ -n "$wt" ] && [ -f "$wt/.afk/pr_title" ] && [ -f "$wt/.afk/pr_body" ]; then
      title=$(cat "$wt/.afk/pr_title")
      body=$(cat "$wt/.afk/pr_body")
      pr_url=$(gh pr create --base "$BASE" --head "issue-$N" \
        --title "$title" --body "$body" 2>/dev/null || true)
      if [ -n "$pr_url" ]; then
        pr_num=$(printf '%s' "$pr_url" | pr_url_to_number)
        gh pr edit "$pr_num" --add-label needs-review >/dev/null 2>&1
        echo "Issue #$N: opened PR #$pr_num (base=$BASE), labeled needs-review."
      fi
    else
      echo "Iteration $i: PR title/body still missing; no PR opened (leaving ready-for-agent)."
    fi
  fi
  if gh pr list --state open --head "issue-$N" --json number | grep -q '"number"'; then
    gh issue edit "$N" --remove-label ready-for-agent >/dev/null 2>&1 \
      && echo "Issue #$N: removed ready-for-agent (open PR exists)."
  else
    echo "Iteration $i: no open PR for issue-$N; leaving ready-for-agent."
  fi
  return 0
}
