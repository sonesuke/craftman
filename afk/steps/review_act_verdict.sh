# afk/steps/review_act_verdict.sh — Step 3 of the AFK review iteration.
#
# Act on the review agent's verdict (VERDICT) for PR #PR_NUM. The decision is
# the pure helper review_verdict_action (tested in afk/tests/test_pure.sh); the
# side effects — label transitions and `gh pr merge` — live HERE in the control
# layer, never in the agent (ADR-0006: agents only judge). The two-strike guard
# is inside review_verdict_action: the 2nd consecutive needs-rework force-
# transitions to ready-for-human instead of ping-ponging. Reads globals PR_NUM,
# N, ROUND, VERDICT. Side-effecting: calls gh.
#
#   mergeable    -> merge after CI is green (or ready-for-human if merge fails)
#   needs-human  -> ready-for-human (round resets)
#   needs-rework -> 1st time: ready-for-agent + afk-review-1 (send back);
#                   2nd time: ready-for-human (two-strike, round resets)
# A content-approved PR whose CI is not green is re-routed as needs-rework, so
# the two-strike guard can escalate a persistently-broken build instead of
# looping forever.

# True (return 0) if PR #1's CI is green, false otherwise. A PR with no checks
# at all (e.g. a sub-issue PR targeting prd-<parent>, where ci.yml does not run)
# is treated as green — there is nothing to gate on. Polls for a bounded window
# (CI_WAIT_ATTEMPTS x CI_WAIT_INTERVAL seconds, defaults 40 x 30 = 20 min) so a
# stuck build escalates instead of hanging the loop.
review_wait_ci_green() {
  local pr="$1" attempt checks
  checks=$(gh pr checks "$pr" --json name 2>/dev/null | jq 'length' 2>/dev/null || echo 0)
  # No checks reported -> nothing to gate on; treat as green.
  if [ "$checks" = "0" ]; then
    return 0
  fi
  for ((attempt = 1; attempt <= ${CI_WAIT_ATTEMPTS:-40}; attempt++)); do
    if gh pr checks "$pr" >/dev/null 2>&1; then
      return 0
    fi
    sleep "${CI_WAIT_INTERVAL:-30}"
  done
  return 1
}

# Merge PR #1 (squash + delete branch). On a merge failure (e.g. squash
# disabled, conflict) escalate to ready-for-human rather than retrying.
review_do_merge() {
  local pr="$1" issue="$2"
  if gh pr merge "$pr" --squash --delete-branch >/dev/null 2>&1; then
    echo "Review #$issue: merged PR #$pr (CI green). Issue closes via its Closes #$issue."
  else
    gh pr edit "$pr" --remove-label needs-review >/dev/null 2>&1 || true
    gh pr edit "$pr" --add-label ready-for-human >/dev/null 2>&1 || true
    echo "Review #$issue: merge command failed for PR #$pr; -> ready-for-human."
  fi
}

# Send the PR to a human: ready-for-human, round counter cleared.
review_do_to_human() {
  local pr="$1" issue="$2"
  gh pr edit "$pr" --remove-label needs-review >/dev/null 2>&1 || true
  gh pr edit "$pr" --remove-label afk-review-1 >/dev/null 2>&1 || true
  gh pr edit "$pr" --add-label ready-for-human >/dev/null 2>&1 || true
  echo "Review #$issue: PR #$pr -> ready-for-human (round reset)."
}

# Send the PR back to the producer: ready-for-agent, mark round 1 (afk-review-1).
# Only reached on the 1st consecutive send-back — the 2nd is diverted to
# review_do_to_human by the two-strike guard.
review_do_send_back() {
  local pr="$1" issue="$2"
  gh pr edit "$pr" --remove-label needs-review >/dev/null 2>&1 || true
  gh pr edit "$pr" --add-label ready-for-agent >/dev/null 2>&1 || true
  gh pr edit "$pr" --add-label afk-review-1 >/dev/null 2>&1 || true
  echo "Review #$issue: PR #$pr sent back to producer (ready-for-agent). One more send-back escalates to a human."
}

review_act_verdict() {
  local verdict="${VERDICT:-needs-human}" round="${ROUND:-0}" dispatch action
  # Content-approved but CI not green: the producer must fix the build. Re-route
  # as needs-rework so the two-strike guard escalates a persistently-broken CI.
  if [ "$verdict" = "mergeable" ] && ! review_wait_ci_green "$PR_NUM"; then
    echo "Review #$N: CI not green on PR #$PR_NUM; treating as needs-rework."
    verdict="needs-rework"
  fi

  dispatch=$(review_verdict_action "$round" "$verdict")
  action="${dispatch%% *}"

  case "$action" in
    merge)
      review_do_merge "$PR_NUM" "$N"
      ;;
    to-human)
      review_do_to_human "$PR_NUM" "$N"
      ;;
    to-agent)
      review_do_send_back "$PR_NUM" "$N"
      ;;
    *)
      echo "Review #$N: unrecognized dispatch '$dispatch' for PR #$PR_NUM; leaving at needs-review."
      ;;
  esac
  return 0
}
