# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the actual label strings used in this repo's issue tracker, and adds the AFK review state.

| Label in mattpocock/skills | Label in our tracker | Meaning                                                        |
| -------------------------- | -------------------- | -------------------------------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue                        |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information                       |
| `ready-for-agent`          | `ready-for-agent`    | Ready for an AFK agent: a new issue to implement, or a PR sent back for rework |
| `needs-review`             | `needs-review`       | Implemented, awaiting AFK review (the review loop picks this)  |
| `ready-for-human`          | `ready-for-human`    | Reviewed, awaits a human's merge or override                   |
| `wontfix`                  | `wontfix`            | Will not be actioned                                           |

`ready-for-agent` is shared across the two AFK loops: an issue lands here as new
work, and a PR lands here when the review loop sends it back for rework. A PR
that is implemented moves to `needs-review`; once reviewed it is either merged,
sent back to `ready-for-agent` (rework), or sent to `ready-for-human`. There is
also a `afk-review-1` marker label — set by the review loop after the first
send-back — that powers the two-strike guard (a second consecutive send-back
escalates the PR to `ready-for-human`).

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label string from this table.

Edit the right-hand column to match whatever vocabulary you actually use.
