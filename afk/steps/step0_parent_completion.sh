# afk/steps/step0_parent_completion.sh — Step 0 of the AFK iteration.
#
# For every open parent whose sub-issues are ALL closed and which has no open
# prd-<parent> -> main PR, create the prd-<parent> branch (off main if needed),
# push it, and open the final PR (Closes #<parent>, needs-review). This is how a
# parent reaches review: the final integration PR is not special — it flows
# through needs-review like any other (ADR-0006). Side-effecting: calls gh/git.
# No state crosses into the other steps.
step_parent_completion() {
  local parents P all_closed pr_url pr_num
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
        pr_num=$(printf '%s' "$pr_url" | pr_url_to_number)
        gh pr edit "$pr_num" --add-label needs-review >/dev/null 2>&1 \
          && echo "Parent #$P: opened final PR #$pr_num (prd-$P -> main), labeled needs-review."
      fi
    fi
  done
  return 0
}
