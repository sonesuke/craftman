# afk/steps/step1_pick_issue.sh — Step 1 of the AFK iteration.
#
# Filter grabbable ready-for-agent candidates mechanically (filter_grabbable,
# then drop those whose branch already has an open PR), then ask Claude to pick
# the single highest-priority one. Sets the global N to the picked issue number.
#
# Exits the script (exit 0) when nothing is grabbable, matching the original
# early-exit behaviour. Side-effecting: calls gh/claude.
step_pick_issue() {
  local raw closed_json open_prs candidates pick
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
  open_prs=$(gh pr list --state open --json headRefName --jq '[.[].headRefName]')
  candidates=$(printf '%s' "$candidates" | OPEN_PRS="$open_prs" drop_with_open_pr)
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
}
