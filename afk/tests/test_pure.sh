#!/usr/bin/env bash
# Offline tests for the pure helpers in afk/lib/pure.sh — the AFK loop's
# unit-tested seam (no gh/wt/git/claude I/O, no network).
#
#   filter_grabbable:   a candidate is grabbable iff it has no sub-issues of its
#                       own AND every blocked-by blocker is closed (in CLOSED_JSON).
#   drop_with_open_pr:  drop candidates whose issue-<N> branch already has an
#                       open PR (in OPEN_PRS).
#   pr_url_to_number:   extract the trailing numeric PR number from a PR URL.
#   worktree_path_for_branch:  resolve a branch name to its worktree path from
#                              `git worktree list --porcelain` output.
#
# To add a pure helper: source afk/lib/pure.sh, feed it canned input, and assert
# the survivors via expect_eq (the single comparison primitive below).
#
# Run: bash afk/tests/test_pure.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/../lib/pure.sh"

pass=0
fail=0

# Single comparison primitive: every test below funnels through this.
expect_eq() {
  local name="$1" expected="$2" got="$3"
  if [[ "$got" == "$expected" ]]; then
    echo "ok   - $name"
    pass=$((pass + 1))
  else
    echo "FAIL - $name"
    echo "  expected: $expected"
    echo "  got:      $got"
    fail=$((fail + 1))
  fi
}

# filter_grabbable: `gh issue list` JSON on stdin, CLOSED_JSON in env.
check() {
  local name="$1" input="$2" closed="$3" expected="$4"
  expect_eq "$name" "$expected" \
    "$(printf '%s' "$input" | CLOSED_JSON="$closed" filter_grabbable)"
}

# drop_with_open_pr: candidate array on stdin, OPEN_PRS in env. Asserted by the
# surviving numbers — the contract is WHICH candidates survive, not jq's output
# formatting — so the assertion is formatting-agnostic.
check_prune() {
  local name="$1" input="$2" open_prs="$3" expected="$4"
  expect_eq "$name" "$expected" \
    "$(printf '%s' "$input" | OPEN_PRS="$open_prs" drop_with_open_pr | jq -c '[.[].number]')"
}

# pr_url_to_number: PR URL on stdin, number on stdout.
check_pr_url() {
  local name="$1" url="$2" expected="$3"
  expect_eq "$name" "$expected" "$(printf '%s' "$url" | pr_url_to_number)"
}

# worktree_path_for_branch: `git worktree list --porcelain` on stdin,
# WORKTREE_BRANCH in env, worktree path on stdout.
check_worktree() {
  local name="$1" branch="$2" input="$3" expected="$4"
  expect_eq "$name" "$expected" \
    "$(printf '%s' "$input" | WORKTREE_BRANCH="$branch" worktree_path_for_branch)"
}

# --- filter_grabbable --------------------------------------------------------

# Fixture 1: mixed bag
# 1 single-shot (no children, no blocker)        -> grabbable
# 2 parent (has sub-issues)                      -> skip
# 3 child blocked by OPEN issue #10              -> skip
# 4 child blocked by CLOSED issue #7             -> grabbable
# 5 child no blocker                             -> grabbable
read -r -d '' MIXED <<'JSON' || true
[
  {"number":1,"title":"single-shot","subIssues":{"totalCount":0,"nodes":[]},"blockedBy":{"totalCount":0,"nodes":[]}},
  {"number":2,"title":"parent","subIssues":{"totalCount":3,"nodes":[{"number":11},{"number":12},{"number":13}]},"blockedBy":{"totalCount":0,"nodes":[]}},
  {"number":3,"title":"child-open-blocker","subIssues":{"totalCount":0,"nodes":[]},"blockedBy":{"totalCount":1,"nodes":[{"number":10}]}},
  {"number":4,"title":"child-closed-blocker","subIssues":{"totalCount":0,"nodes":[]},"blockedBy":{"totalCount":1,"nodes":[{"number":7}]}},
  {"number":5,"title":"child-no-blocker","subIssues":{"totalCount":0,"nodes":[]},"blockedBy":{"totalCount":0,"nodes":[]}}
]
JSON

check "mixed: keeps single-shot + closed-blocker + no-blocker, drops parent + open-blocker" \
  "$MIXED" '[7]' \
  '[{"number":1,"title":"single-shot"},{"number":4,"title":"child-closed-blocker"},{"number":5,"title":"child-no-blocker"}]'

check "empty input -> empty output" '[]' '[7]' '[]'

# Fixture: multiple blockers
read -r -d '' MULTI <<'JSON' || true
[
  {"number":8,"title":"multi-closed","subIssues":{"totalCount":0,"nodes":[]},"blockedBy":{"totalCount":2,"nodes":[{"number":7},{"number":9}]}}
]
JSON

check "multiple blockers all closed -> grabbable" \
  "$MULTI" '[7,9]' \
  '[{"number":8,"title":"multi-closed"}]'

check "multiple blockers one open -> skip" \
  "$MULTI" '[7]' \
  '[]'

check "blocker present, closed set empty -> skip" \
  '[{"number":1,"title":"x","subIssues":{"totalCount":0,"nodes":[]},"blockedBy":{"totalCount":1,"nodes":[{"number":7}]}}]' \
  '[]' \
  '[]'

# --- drop_with_open_pr -------------------------------------------------------
# Candidates in the {number, title} shape that filter_grabbable emits.
read -r -d '' CANDS <<'JSON' || true
[
  {"number":1,"title":"a"},
  {"number":4,"title":"d"},
  {"number":5,"title":"e"}
]
JSON

check_prune "drops the candidate whose issue-<N> branch has an open PR" \
  "$CANDS" '["issue-4","issue-9"]' '[1,5]'

check_prune "no open PRs -> all candidates survive" \
  "$CANDS" '[]' '[1,4,5]'

check_prune "every candidate has an open PR -> none survive" \
  "$CANDS" '["issue-1","issue-4","issue-5"]' '[]'

check_prune "OPEN_PRS empty -> treated as no open PRs, all survive" \
  '[{"number":7,"title":"g"}]' '' '[7]'

# --- pr_url_to_number --------------------------------------------------------

check_pr_url "extracts the number from a standard pull URL" \
  "https://github.com/sonesuke/craftman/pull/42" "42"

check_pr_url "extracts the number from another repo's pull URL" \
  "https://github.com/octocat/Hello-World/pull/7" "7"

check_pr_url "takes the trailing number when earlier segments also have digits" \
  "https://github.com/o1/r2/pull/123" "123"

# --- worktree_path_for_branch -------------------------------------------------
# `git worktree list --porcelain` on stdin; blank-line-separated stanzas, each a
# `worktree <path>` + `branch refs/heads/<name>` pair. Mirrors the real layout
# craftman's worktrunk produces (/workspaces/craftman.issue-<N>).
read -r -d '' WT_PORCELAIN <<'PORCELAIN' || true
worktree /workspaces/craftman
HEAD 68a29995ecd58786ad404584010fe934b3c94bb5
branch refs/heads/main

worktree /workspaces/craftman.issue-16
HEAD da2fc9719589659c3b2fec6444fe01240ae823fa
branch refs/heads/issue-16

worktree /workspaces/craftman.issue-33
HEAD 1a82f9d30fb1447d04a3fcf057aeb604bf5bb973
branch refs/heads/issue-33
PORCELAIN

check_worktree "finds the path for an issue branch" \
  "issue-33" "$WT_PORCELAIN" "/workspaces/craftman.issue-33"

check_worktree "finds the path for main" \
  "main" "$WT_PORCELAIN" "/workspaces/craftman"

check_worktree "returns nothing for a branch with no worktree" \
  "issue-99" "$WT_PORCELAIN" ""

check_worktree "returns nothing when WORKTREE_BRANCH is empty" \
  "" "$WT_PORCELAIN" ""

echo
echo "passed=$pass failed=$fail"
[[ "$fail" -eq 0 ]]
