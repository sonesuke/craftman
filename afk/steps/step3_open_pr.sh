# afk/steps/step3_open_pr.sh — Step 3 of the AFK iteration.
#
# Open the PR for issue-<N> against the correct base (BASE, resolved in Step 2)
# if none is open yet, label it ready-for-human, and drop ready-for-agent from
# the issue once an open PR exists. The issue is NOT closed here — it closes at
# PR merge (Closes #N). Reads the globals N and BASE. Side-effecting: calls gh.
step_open_pr() {
  local pr_url pr_num
  if ! gh pr list --state open --head "issue-$N" --json number | grep -q '"number"'; then
    pr_url=$(gh pr create --base "$BASE" --head "issue-$N" \
      --title "Closes #$N" --body "Closes #$N" 2>/dev/null || true)
    if [ -n "$pr_url" ]; then
      pr_num=$(printf '%s' "$pr_url" | pr_url_to_number)
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
  return 0
}
