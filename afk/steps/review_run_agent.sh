# afk/steps/review_run_agent.sh — Step 2 of the AFK review iteration.
#
# Run the review agent on PR #PR_NUM. It has two duties — content correctness
# (does the change implement issue #N's spec) and pr-format conformance (does
# the PR match docs/agents/pr-format.md). It posts its findings as a PR comment
# and prints a verdict line. We parse that line into the global VERDICT
# (mergeable | needs-human | needs-rework), defaulting to needs-human if the
# agent produced no parseable verdict — escalating is safer for an unattended
# loop than silently retrying.
#
# Returns non-zero when Claude exits non-zero, so the caller can skip the act
# step (|| continue) and leave the PR at needs-review. Reads globals PR_NUM, N,
# HEAD_REF; sets VERDICT. Side-effecting: calls gh/claude.
review_run_agent() {
  local PROMPT rc verdict_raw
  PROMPT="You are the AFK review agent for the craftman repo, reviewing PR #${PR_NUM} \
(implements issue #${N:-unknown}, head branch ${HEAD_REF}). You have two duties:
1. Content correctness: does the change correctly and completely implement issue #${N}'s spec? \
Read the spec: gh issue view ${N} --comments. Inspect the change: gh pr diff ${PR_NUM}. \
Read the PR description: gh pr view ${PR_NUM} --json title,body.
2. PR-format conformance: does the PR conform to docs/agents/pr-format.md? \
(conventional-commit title; a one-line intro citing issue #${N} and any related ADR; \
## Verification and ## Note for reviewers where they apply.)
If you find problems, post a specific, actionable PR comment FIRST so the producer can rework: \
gh pr comment ${PR_NUM} --body \"...\".
Then decide a verdict and print it as your FINAL line, EXACTLY one of:
  VERDICT: mergeable      — correct and conformant; safe to merge.
  VERDICT: needs-human    — needs a human (integration judgment too heavy, or a risk you cannot resolve).
  VERDICT: needs-rework   — fixable problems; send back to the producer.
Do NOT merge, relabel, close, or push anything — the loop acts on your verdict."
  set +e
  verdict_raw=$(claude --dangerously-skip-permissions -p "$PROMPT")
  rc=$?
  set -e
  if [ $rc -ne 0 ]; then
    echo "Iteration $i: review agent exited non-zero (rc=$rc); leaving PR #$PR_NUM at needs-review. Continuing."
    VERDICT=""
    return 1
  fi

  VERDICT=$(printf '%s\n' "$verdict_raw" \
    | grep -oiE 'VERDICT:[[:space:]]*(mergeable|needs-human|needs-rework)' \
    | tail -1 \
    | sed -E 's/.*:[[:space:]]*//' \
    | tr 'A-Z' 'a-z')

  if [ -z "$VERDICT" ]; then
    echo "Iteration $i: review agent gave no parseable verdict for PR #$PR_NUM; escalating to needs-human."
    VERDICT="needs-human"
  fi
  echo "Review agent verdict for PR #$PR_NUM: $VERDICT"
  return 0
}
