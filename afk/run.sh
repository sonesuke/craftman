#!/usr/bin/env bash
# afk/run.sh — AFK execution loop: one GitHub issue per worktree, via worktrunk.
#
# One iteration runs four phases in order, each a named function sourced below:
#   Step 0  step_parent_completion  open the final prd-<parent> -> main PR for
#                                   parents whose sub-issues are all closed
#   Step 1  step_pick_issue         filter grabbable ready-for-agent candidates,
#                                   then Claude picks one (sets global N)
#   Step 2  step_run_agent          resolve the PR base, run headless Claude on
#                                   issue-<N>; returns non-zero so the loop can
#                                   skip the PR step on a failed agent run
#   Step 3  step_open_pr            open the PR with the right base and relabel
#
# The entry sources the pure helpers in afk/lib/ (the unit-tested seam) and the
# phase functions in afk/steps/; state crossing phase boundaries (N, BASE) is
# carried in shell globals.
#
# Usage: ./afk/run.sh <iterations>
# Run from the repo root (the main worktree). Requires `gh`, `wt`, `claude`, and
# `mise` on PATH. A non-zero Claude exit in Step 2 does NOT abort the loop: the
# worktree is left for inspection and the next iteration runs.
#
# The pure functions in afk/lib/ are unit-tested by afk/tests/test_pure.sh,
# which sources them directly; this main loop runs only when this file is
# executed (not sourced).
#
# Architecture rationale — why the loop never closes issues, why branches mirror
# the issue hierarchy, the full process vocabulary — lives in
# docs/agents/workflow.md and ADR-0004, not in this header.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/lib/pure.sh"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/steps/step0_parent_completion.sh"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/steps/step1_pick_issue.sh"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/steps/step2_run_agent.sh"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/steps/step3_open_pr.sh"

# --- Main (only when executed, not sourced) ----------------------------

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then

set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <iterations>"
  exit 1
fi

for ((i=1; i<=$1; i++)); do
  echo "Iteration $i"
  echo "--------------------------------"

  # --- Step 0: open final parent PRs for parents whose sub-issues closed.
  step_parent_completion

  # --- Step 1: filter grabbable candidates, then Claude picks one (sets N).
  step_pick_issue

  # --- Step 2: determine base, ensure parent branch, run headless Claude.
  #             A non-zero agent run skips the PR step and leaves the worktree.
  step_run_agent || continue

  # --- Step 3: open the PR with the correct base, then drop ready-for-agent.
  step_open_pr
done

echo "Completed $1 iterations."

fi  # end main guard
