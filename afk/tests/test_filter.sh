#!/usr/bin/env bash
# Offline tests for filter_grabbable(), the grabbable-issue filter in run.sh.
#
# Feeds canned `gh issue list` JSON (no network) and asserts the filtered
# survivors. A candidate is grabbable iff:
#   - it has no sub-issues of its own (subIssues.totalCount == 0), AND
#   - every blocked-by blocker is closed (its number is in CLOSED_JSON).
#
# Run: bash afk/tests/test_filter.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$SCRIPT_DIR/../run.sh"

pass=0
fail=0

check() {
  local name="$1" input="$2" closed="$3" expected="$4" got
  got=$(printf '%s' "$input" | CLOSED_JSON="$closed" filter_grabbable)
  if [[ "$got" == "$expected" ]]; then
    echo "ok   - $name"
    pass=$((pass + 1))
  else
    echo "FAIL - $name"
    echo "  closed:   $closed"
    echo "  expected: $expected"
    echo "  got:      $got"
    fail=$((fail + 1))
  fi
}

# --- Fixture 1: mixed bag -----------------------------------------------
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

# --- Fixture 2: empty input ---------------------------------------------
check "empty input -> empty output" '[]' '[7]' '[]'

# --- Fixture 3: multiple blockers, all closed -> grabbable --------------
read -r -d '' MULTI <<'JSON' || true
[
  {"number":8,"title":"multi-closed","subIssues":{"totalCount":0,"nodes":[]},"blockedBy":{"totalCount":2,"nodes":[{"number":7},{"number":9}]}}
]
JSON

check "multiple blockers all closed -> grabbable" \
  "$MULTI" '[7,9]' \
  '[{"number":8,"title":"multi-closed"}]'

# --- Fixture 4: multiple blockers, one open -> skip ---------------------
check "multiple blockers one open -> skip" \
  "$MULTI" '[7]' \
  '[]'

# --- Fixture 5: blocker present but closed set empty -> skip ------------
check "blocker present, closed set empty -> skip" \
  '[{"number":1,"title":"x","subIssues":{"totalCount":0,"nodes":[]},"blockedBy":{"totalCount":1,"nodes":[{"number":7}]}}]' \
  '[]' \
  '[]'

echo
echo "passed=$pass failed=$fail"
[[ "$fail" -eq 0 ]]
