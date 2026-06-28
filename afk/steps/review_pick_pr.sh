# afk/steps/review_pick_pr.sh — Step 1 of the AFK review iteration.
#
# Filter needs-review PRs mechanically (filter_needs_review), then ask Claude to
# pick the single highest-priority one to review. Resolves the picked PR's head
# ref, the issue it implements (N), and the current review round (ROUND = 1 if
# the PR has been sent back once already, else 0 — read from the afk-review-1
# label that powers the two-strike guard).
#
# Exits the script (exit 0) when no PR needs review, matching the producer
# loop's early-exit behaviour. Sets globals PR_NUM, HEAD_REF, N, ROUND.
# Side-effecting: calls gh/claude.
review_pick_pr() {
  local raw candidates pick
  raw=$(gh pr list --state open --label needs-review \
    --json number,headRefName,labels)
  candidates=$(printf '%s' "$raw" | filter_needs_review)

  if [ "$candidates" = "[]" ]; then
    echo "No needs-review PRs, exiting."
    if command -v tt >/dev/null 2>&1; then
      tt notify "craftman: no needs-review PRs after $((i-1)) review iterations"
    fi
    exit 0
  fi

  pick=$(claude --dangerously-skip-permissions -p \
"You are choosing which PR to review next in the craftman repo.
needs-review PRs (already filtered to those awaiting review):
$candidates
Pick the single highest-priority PR you would review next.
Reply with ONLY the PR number (e.g. 42). If nothing is to review, reply NONE.")

  PR_NUM=$(printf '%s' "$pick" | grep -oE '[0-9]+' | head -1 || true)

  if [ -z "$PR_NUM" ]; then
    echo "Claude reported no PR to review, exiting."
    if command -v tt >/dev/null 2>&1; then
      tt notify "craftman: no PR to review after $((i-1)) review iterations"
    fi
    exit 0
  fi

  HEAD_REF=$(gh pr view "$PR_NUM" --json headRefName --jq '.headRefName')
  N=$(printf '%s\n' "$HEAD_REF" | pr_head_to_issue_number)
  if gh pr view "$PR_NUM" --json labels --jq '[.labels[].name]' \
       | grep -qs '"afk-review-1"'; then
    ROUND=1
  else
    ROUND=0
  fi

  echo "Review: picked PR #$PR_NUM (issue #${N:-?}, head $HEAD_REF), round=$ROUND."
}
