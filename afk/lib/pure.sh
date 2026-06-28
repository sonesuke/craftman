# afk/lib/pure.sh — pure helpers for the AFK loop (no gh/wt/git/claude I/O).
#
# These functions are the testable seam: they take data on stdin or via
# environment variables and emit data on stdout, making no network or shell
# calls. Unit-tested by afk/tests/test_pure.sh, which sources this file
# directly — see that file for the fixture pattern to copy when adding a new
# pure helper.

# Read a `gh issue list --json number,title,subIssues,blockedBy` blob on stdin;
# emit a compact JSON array of {number, title} for the grabbable issues only.
# An issue is grabbable iff it has no sub-issues of its own AND every one of its
# blocked-by blockers is closed (its number is in CLOSED_JSON, a JSON array).
filter_grabbable() {
  local closed_json="${CLOSED_JSON:-[]}"
  jq -c --argjson closed "$closed_json" '
    map(
      select(
        ((.subIssues.totalCount // 0) == 0)
        and
        ([.blockedBy.nodes[].number] as $blockers
         | ($blockers | length) == 0
           or ($blockers | map(. as $b | $closed | index($b) != null) | all))
      ) | {number, title}
    )
  '
}

# Read the {number, title} candidate array (output of filter_grabbable) on stdin;
# drop candidates whose branch already has an open PR. OPEN_PRS is a JSON array
# of open-PR head-ref names (e.g. ["issue-7","issue-9"]). A candidate survives
# iff no open PR targets "issue-<number>". Output keeps jq's default (pretty)
# formatting, matching the original inlined prune step.
drop_with_open_pr() {
  local open_prs="${OPEN_PRS:-[]}"
  jq --argjson openprs "$open_prs" '
    map(select(. as $c | $openprs | index("issue-\($c.number)") | not))
  '
}

# Read a `gh pr create` PR URL (e.g. https://github.com/o/r/pull/42) on stdin;
# emit the trailing numeric PR number.
pr_url_to_number() {
  grep -oE '[0-9]+' | tail -1
}

# Read `git worktree list --porcelain` output on stdin; emit the filesystem
# path of the worktree checked out on the branch named in WORKTREE_BRANCH
# (e.g. "issue-33"), or nothing if no worktree matches. Porcelain output is a
# sequence of blank-line-separated stanzas, each holding a `worktree <path>`
# line and a `branch refs/heads/<name>` line. Pure (no git I/O inside) — the
# caller pipes `git worktree list --porcelain` in, so this stays in the
# unit-tested seam. Used by step3 to find where the producer agent wrote
# .afk/pr_title and .afk/pr_body.
worktree_path_for_branch() {
  local branch="${WORKTREE_BRANCH:-}" wt="" br="" line
  # `|| [ -n "$line" ]` so the final line is processed even when the input has
  # no trailing newline (read returns non-zero at EOF but still sets $line).
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      "worktree "*) wt="${line#worktree }" ;;
      "branch "*)
        br="${line#branch }"
        br="${br#refs/heads/}"
        ;;
      "")
        if [ "$br" = "$branch" ] && [ -n "$wt" ]; then
          printf '%s\n' "$wt"; return 0
        fi
        wt=""; br=""
        ;;
    esac
  done
  # Last stanza (no trailing blank line) — flush it too.
  if [ "$br" = "$branch" ] && [ -n "$wt" ]; then
    printf '%s\n' "$wt"
  fi
}

# Read a `gh pr list --json number,headRefName,labels` blob on stdin; emit a
# compact JSON array of {number, headRefName} for the PRs carrying the
# needs-review label only. The review loop's pick source — the PR-side mirror of
# the implementation loop's filter_grabbable over issues.
filter_needs_review() {
  jq -c 'map(select(.labels | map(.name) | index("needs-review")) | {number, headRefName})'
}

# Pure two-strike verdict dispatch (D7). Given the PR's current review round
# (0 = never sent back by the review agent, 1 = sent back once already) and the
# review agent's verdict (mergeable | needs-human | needs-rework), print
# "<action> <store_round>" on stdout, where:
#   merge      — merge after CI is green (round resets; the PR is done)
#   to-human   — label ready-for-human (round resets; a human's touch)
#   to-agent   — send back to ready-for-agent for rework (round becomes 1)
# The two-strike guard fires on the SECOND consecutive needs-rework (round 1):
# it force-transitions to-human instead of to-agent, ending producer<->reviewer
# ping-pong. store_round is the round the caller should leave on the PR
# (0 = drop the afk-review-1 counter label, 1 = set it). Pure — the caller does
# all gh/label/merge I/O around it.
review_verdict_action() {
  local round="${1:-0}" verdict="${2:-}"
  case "$verdict" in
    mergeable)
      printf 'merge 0\n' ;;
    needs-human)
      printf 'to-human 0\n' ;;
    needs-rework)
      if [ "$round" -ge 1 ]; then
        printf 'to-human 0\n'   # two-strike: 2nd consecutive send-back -> human
      else
        printf 'to-agent 1\n'   # 1st send-back -> producer reworks
      fi
      ;;
    *)
      printf 'unknown 0\n' ;;
  esac
}

# Pure mergeability-gate decision for the review loop's first pre-merge gate.
# Given a PR's GitHub `mergeable` (MERGEABLE | CONFLICTING | UNKNOWN) and
# `mergeStateStatus` (CLEAN | BEHIND | DIRTY | UNSTABLE | ...), print exactly one
# of:
#   pass          — no mergeability objection; hand to the CI-green gate.
#   auto-update   — mergeable but stale (BEHIND): the control layer rebases the
#                   PR's own head onto its base and re-pushes, then re-reviews.
#                   No producer/review round (strike) is consumed.
#   needs-rework  — CONFLICTING or DIRTY: route to the producer to resolve.
#   wait          — UNKNOWN (GitHub still computing) or unrecognized: don't
#                   guess; leave at needs-review and re-check next iteration.
# Decision table (the whole contract): MERGEABLE+CLEAN -> pass,
# MERGEABLE+BEHIND -> auto-update, MERGEABLE+DIRTY -> needs-rework,
# MERGEABLE+UNSTABLE -> pass (a CI concern, owned by the CI gate),
# CONFLICTING -> needs-rework, UNKNOWN -> wait. All gh/git I/O lives in the
# control layer (review_act_verdict) around this pure function.
review_mergeability_gate() {
  local mergeable="${1:-UNKNOWN}" state="${2:-}"
  case "$mergeable" in
    MERGEABLE)
      case "$state" in
        BEHIND) printf 'auto-update\n' ;;
        DIRTY) printf 'needs-rework\n' ;;
        CLEAN|UNSTABLE) printf 'pass\n' ;;
        *) printf 'pass\n' ;;  # mergeable with an unrecognized refinement: no objection
      esac
      ;;
    CONFLICTING) printf 'needs-rework\n' ;;
    *) printf 'wait\n' ;;      # UNKNOWN (still computing) or unrecognized: don't misroute
  esac
}

# Read a PR head-ref name on stdin (e.g. "issue-42", "prd-7") and emit the
# trailing issue/parent number it was branched for, or nothing for an
# unrecognized or non-numeric shape. issue-<N> sub-issue PRs and prd-<parent>
# parent PRs each carry exactly one number; the review loop uses it to fetch the
# spec issue the PR implements.
pr_head_to_issue_number() {
  local ref
  IFS= read -r ref || true
  case "$ref" in
    issue-*|prd-*)
      printf '%s\n' "${ref##*-}" | grep -oE '^[0-9]+$' || true
      ;;
  esac
}
