# afk/steps/review_act_verdict.sh — Step 3 of the AFK review iteration.
#
# Act on the review agent's verdict (VERDICT) for PR #PR_NUM. The decisions are
# the pure helpers review_mergeability_gate and review_verdict_action (both
# tested in afk/tests/test_pure.sh); the side effects — branch rebase/force-
# push, PR comments, label transitions, and `gh pr merge` — live HERE in the
# control layer, never in the agent (ADR-0006: agents only judge). The two
# pre-merge gates run in source order, each detect -> route:
#
#   1. Mergeability gate (runs FIRST, only for a mergeable verdict).
#        pass          -> fall through to the CI-green gate.
#        auto-update   -> rebase the PR's own head onto its base and re-push,
#                         then leave at needs-review (no strike consumed); the
#                         next iteration re-reviews the rebased branch.
#        needs-rework  -> CONFLICTING/DIRTY: route to the producer (with a
#                         comment telling it to rebase + resolve + re-push).
#        wait          -> mergeability UNKNOWN (GitHub still computing): leave
#                         at needs-review and re-check next iteration.
#   2. CI-green gate. A mergeable, clean PR whose CI is not green is re-routed
#      as needs-rework, so the two-strike guard escalates a persistently-broken
#      build instead of looping forever.
#
# Reads globals PR_NUM, N, ROUND, VERDICT (HEAD_REF from Step 1). Sets the
# step-local globals MERGEABLE, MERGE_STATE, PR_BASE while the gate runs.
# Side-effecting: calls gh/git.
#
#   mergeable    -> merge after the two gates pass (or ready-for-human if merge fails)
#   needs-human  -> ready-for-human (round resets)
#   needs-rework -> 1st time: ready-for-agent + afk-review-1 (send back);
#                   2nd time: ready-for-human (two-strike, round resets)

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

# Read PR #1's mergeability via `gh pr view --json mergeable,mergeStateStatus`,
# polling a bounded window (MERGE_POLL_ATTEMPTS x MERGE_POLL_INTERVAL, defaults
# 5 x 10 = ~1 min) while GitHub reports UNKNOWN (it recomputes after every
# push/merge). Sets the step globals MERGEABLE (MERGEABLE | CONFLICTING |
# UNKNOWN), MERGE_STATE (CLEAN | BEHIND | DIRTY | UNSTABLE | ...), and PR_BASE
# (the PR's baseRefName — main for a single-shot, prd-<parent> for a sub-issue,
# per ADR-0004). Side-effecting: calls gh.
review_read_mergeability() {
  local pr="$1" attempt json
  MERGEABLE="UNKNOWN"; MERGE_STATE=""; PR_BASE="main"
  for ((attempt = 1; attempt <= ${MERGE_POLL_ATTEMPTS:-5}; attempt++)); do
    json=$(gh pr view "$pr" --json mergeable,mergeStateStatus,baseRefName 2>/dev/null || true)
    MERGEABLE=$(printf '%s' "$json" | jq -r '.mergeable // "UNKNOWN"')
    MERGE_STATE=$(printf '%s' "$json" | jq -r '.mergeStateStatus // ""')
    PR_BASE=$(printf '%s' "$json" | jq -r '.baseRefName // "main"')
    [ "$MERGEABLE" != "UNKNOWN" ] && return 0
    sleep "${MERGE_POLL_INTERVAL:-10}"
  done
  # Still UNKNOWN after the window; the gate routes it to `wait`.
}

# Auto-rebase PR #1's head branch onto its base (PR_BASE, read from the PR's
# baseRefName) and force-push the head ONLY. Uses a detached TEMP worktree so it
# neither disturbs the producer's issue-<N> worktree nor needs to move a local
# branch ref that may be checked out elsewhere. On a conflict during rebase it
# aborts, cleans up, and returns non-zero so the caller falls back to
# needs-rework. This is mechanical branch hygiene — judgment-free, so it does
# NOT consume a producer/review round (ADR-0006: an exception to "side effects =
# labels + merge", recorded in ADR-0006). Side-effecting: calls git.
review_do_auto_update() {
  local pr="$1" issue="$2" head="${HEAD_REF:-}" base="${PR_BASE:-main}" tmp
  [ -n "$head" ] || head=$(gh pr view "$pr" --json headRefName --jq '.headRefName' 2>/dev/null || true)
  [ -n "$head" ] && [ -n "$base" ] || return 1
  tmp=$(mktemp -d 2>/dev/null || mktemp -d -t afk-rebase) || return 1
  # Fetch the remote tips so the rebase and the lease both see them.
  git fetch origin "$base" "$head" >/dev/null 2>&1 || git fetch origin >/dev/null 2>&1 || true
  if ! git worktree add --detach "$tmp" "origin/$head" >/dev/null 2>&1; then
    rm -rf "$tmp"; return 1
  fi
  # Rebase the detached head onto the base; on conflict, abort and fall back.
  if git -C "$tmp" rebase "origin/$base" >/dev/null 2>&1; then
    # Clean rebase: force-push the head ONLY, with a lease so a tip that moved
    # since our fetch is never clobbered.
    if git -C "$tmp" push --force-with-lease origin "HEAD:$head" >/dev/null 2>&1; then
      git worktree remove --force "$tmp" >/dev/null 2>&1 || rm -rf "$tmp"
      git worktree prune >/dev/null 2>&1 || true
      return 0
    fi
  else
    git -C "$tmp" rebase --abort >/dev/null 2>&1 || true
  fi
  git worktree remove --force "$tmp" >/dev/null 2>&1 || rm -rf "$tmp"
  git worktree prune >/dev/null 2>&1 || true
  return 1
}

# Post a PR comment with the concrete rework instruction when the mergeability
# gate routes a PR back for a conflict: rebase onto the PR's base, resolve, and
# re-push. Mirrors how the review agent posts actionable comments. Side-effecting:
# calls gh.
review_comment_conflict() {
  local pr="$1" base="${PR_BASE:-main}"
  gh pr comment "$pr" --body "Mergeability gate: this PR conflicts with its base \`$base\` and cannot be merged as-is. Rework: rebase the branch onto \`$base\`, resolve the conflicts, and push again — the review loop will re-review the updated branch." >/dev/null 2>&1 || true
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
  local verdict="${VERDICT:-needs-human}" round="${ROUND:-0}" dispatch action gate
  # --- Mergeability gate (the FIRST pre-merge gate). ----------------------
  # Runs only for a content-approved (mergeable) PR: a PR being sent back or to
  # a human has nothing to gain from a rebase. auto-update and wait leave the PR
  # at needs-review WITHOUT touching the round label, so no strike is consumed;
  # needs-rework falls through to the verdict dispatch below (with the
  # instruction comment posted). The pure decision is unit-tested; the rebase /
  # force-push / comment side effects live here.
  if [ "$verdict" = "mergeable" ]; then
    review_read_mergeability "$PR_NUM"
    gate=$(review_mergeability_gate "$MERGEABLE" "$MERGE_STATE")
    case "$gate" in
      auto-update)
        if review_do_auto_update "$PR_NUM" "$N"; then
          echo "Review #$N: auto-rebased PR #$PR_NUM onto ${PR_BASE:-main} (was BEHIND); left at needs-review for re-review (no strike consumed)."
          return 0
        fi
        echo "Review #$N: auto-rebase of PR #$PR_NUM hit a conflict; routing to producer as needs-rework."
        review_comment_conflict "$PR_NUM"
        verdict="needs-rework"
        ;;
      needs-rework)
        echo "Review #$N: PR #$PR_NUM is ${MERGEABLE}/${MERGE_STATE:-UNKNOWN}; routing to producer as needs-rework."
        review_comment_conflict "$PR_NUM"
        verdict="needs-rework"
        ;;
      wait)
        echo "Review #$N: mergeability of PR #$PR_NUM is UNKNOWN after polling; left at needs-review to re-check next iteration."
        return 0
        ;;
      pass)
        : # mergeable and clean -> fall through to the CI-green gate.
        ;;
    esac
  fi

  # --- CI-green gate (the SECOND pre-merge gate). -------------------------
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
