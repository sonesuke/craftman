#!/usr/bin/env bash
# afk/review.sh — AFK review (consumer) loop: one needs-review PR per iteration.
#
# The symmetric counterpart to afk/implement.sh. One iteration runs three phases
# in order, each a named function sourced below:
#   Step 1  review_pick_pr       filter needs-review PRs (filter_needs_review),
#                                 then Claude picks one. Sets globals PR_NUM,
#                                 HEAD_REF, N (the issue it implements), ROUND.
#   Step 2  review_run_agent     run the review agent (duties: content
#                                 correctness + pr-format conformance); sets
#                                 VERDICT (mergeable | needs-human | needs-rework).
#                                 Returns non-zero so the loop can skip the act
#                                 step on a failed agent run.
#   Step 3  review_act_verdict   act on the verdict via review_verdict_action:
#                                 merge after CI green, ready-for-human, or send
#                                 back to ready-for-agent (two-strike guarded).
#
# Side effects — label transitions and `gh pr merge` — live in Step 3's control
# layer; the review agent only judges (ADR-0006). The entry sources afk/lib/
# (shared with afk/implement.sh) and the review phase functions in afk/steps/;
# state crossing phase boundaries (PR_NUM, HEAD_REF, N, ROUND, VERDICT) is
# carried in shell globals.
#
# Usage: ./afk/review.sh <iterations>
# Run from the repo root (the main worktree). Requires `gh` and `claude` on PATH.
# A non-zero agent exit in Step 2 does NOT abort the loop: the PR stays at
# needs-review and the next iteration runs.
#
# The pure functions in afk/lib/ are unit-tested by afk/tests/test_pure.sh,
# which sources them directly; this main loop runs only when this file is
# executed (not sourced).
#
# This is the consumer half of the two-loop AFK architecture (ADR-0006); the
# producer half is afk/implement.sh. The full process vocabulary and rationale
# (why agents only judge, the two-strike guard) live in docs/agents/workflow.md
# and ADR-0006, not here.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/lib/pure.sh"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/steps/review_pick_pr.sh"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/steps/review_run_agent.sh"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/steps/review_act_verdict.sh"

# --- Main (only when executed, not sourced) ----------------------------

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then

set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <iterations>"
  exit 1
fi

for ((i=1; i<=$1; i++)); do
  echo "Review iteration $i"
  echo "--------------------------------"

  # --- Step 1: filter needs-review PRs, then Claude picks one (sets PR_NUM).
  review_pick_pr

  # --- Step 2: run the review agent -> verdict. A non-zero agent run skips
  #             the act step and leaves the PR at needs-review.
  review_run_agent || continue

  # --- Step 3: act on the verdict (merge / to-human / send-back).
  review_act_verdict
done

echo "Completed $1 review iterations."

fi  # end main guard
