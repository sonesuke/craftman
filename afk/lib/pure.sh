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
