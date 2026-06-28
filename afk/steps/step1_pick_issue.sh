# afk/steps/step1_pick_issue.sh — Step 1 of the AFK iteration.
#
# Pick this iteration's work. Rework takes precedence over new work: a
# ready-for-agent PR (sent back by the review loop) is in flight, so finish it
# before starting fresh — pick the oldest such PR deterministically (sets
# REWORK=1, PR_NUM, N). Otherwise filter grabbable ready-for-agent issues
# mechanically (filter_grabbable, then drop those whose branch already has an
# open PR) and ask Claude to pick the single highest-priority one (REWORK=0,
# sets N). PR_NUM is the open PR number for a rework PR; empty for new work.
#
# Exits the script (exit 0) when nothing is grabbable, matching the original
# early-exit behaviour. Side-effecting: calls gh/claude.
step_pick_issue() {
  local raw closed_json open_prs candidates pick rework_json prnum headref
  REWORK=0
  PR_NUM=""

  # --- Rework first: ready-for-agent PRs sent back by the review loop. ----
  # Smallest PR number = oldest in-flight rework. headRefName maps to the issue
  # the PR implements (issue-<N> or prd-<parent>).
  rework_json=$(gh pr list --state open --label ready-for-agent \
    --json number,headRefName)
  if [ "$rework_json" != "[]" ]; then
    read -r prnum headref <<EOF
$(printf '%s' "$rework_json" | jq -r 'sort_by(.number) | .[0] | "\(.number) \(.headRefName)"')
EOF
    N=$(printf '%s\n' "$headref" | pr_head_to_issue_number)
    if [ -n "$N" ] && [ -n "$prnum" ]; then
      REWORK=1
      PR_NUM="$prnum"
      echo "Rework: picked PR #$PR_NUM (issue #$N) sent back for rework."
      return 0
    fi
    # headRef didn't map to an issue number; fall through to new-work pick.
  fi

  # --- New work: grabbable ready-for-agent issues. -----------------------
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
