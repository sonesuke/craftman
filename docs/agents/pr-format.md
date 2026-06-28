# PR Format

The contract between the **producer** (the implementation agent that writes a
PR's title and body into `.afk/pr_title` / `.afk/pr_body`) and the **reviewer**
(the review agent that checks a PR against this spec). Every PR the AFK loop
opens conforms to this; the producer writes it, the reviewer enforces it.

This is a *process* document. The product-domain language lives in
[CONTEXT.md](../../CONTEXT.md), and the AFK workflow lives in
[workflow.md](./workflow.md).

## Why a spec

Before this spec, `afk/run.sh` opened every PR with `--title "Closes #N"
--body "Closes #N"` — a title and body that said nothing. The reviewer (human
or agent) had to reconstruct the change from the diff. [PR #35](https://github.com/sonesuke/craftman/pull/35)
was the symptom; [PR #36](https://github.com/sonesuke/craftman/pull/36) is the
shape we want. The producer knows things no one else does (what it verified,
what it deliberately left as-is), and the PR is where that knowledge is handed
off.

## Required

A PR is non-conforming (the reviewer rejects) if either is missing.

- **Title** — one line, [conventional-commit](https://www.conventionalcommits.org/)
  form: `<type>(<scope>): <subject>`. `type` is `fix`, `feat`, `refactor`,
  `docs`, `chore`, `test`, …; `scope` is the area (`afk`, `skills`, `harness`).
  The subject is imperative and specific — never just `Closes #N`.
- **Intro line** — the first line of the body: a single sentence naming **what
  the PR does**, citing **issue #N** and **any related ADR** by number. A
  reviewer who reads only the title and the intro line should know what the PR
  is and where its rationale lives.

## Recommended

Include these sections when they apply. The reviewer judges whether they are
present *and sufficient* — not merely present.

- **`## Verification`** — how the change was confirmed: test output (`passed=N
  failed=0`), `mise run pre-commit`, CI status, a behaviour-equivalence
  argument. This is the producer's evidence; only the producer can write it.
- **`## Note for reviewers`** — anything a reviewer needs to know that is not
  obvious from the diff: a deliberate coupling left in place, a TODO deferred
  with a reason, an out-of-scope item respected. This is where the producer
  heads off "why did you do it this way?" questions.
- **`## Acceptance Criteria`** — mirror the issue's acceptance/testing notes as
  a checklist when the issue carries them. Omit when it has none.

## Worked example (PR #36, abridged)

```
refactor(afk): split run.sh into entry/steps/lib and shrink the header

Implements #32 — splits the AFK loop into entry / steps / lib and separates the
code header from architecture docs. See the issue for rationale; ADR-0005
records the deliberate docs↔header overlap.

## Acceptance Criteria
- [x] One test seam …

## Verification
- bash afk/tests/test_pure.sh → passed=12 failed=0
- bash -n clean on all 7 scripts.
- CI green: AFK pure-function tests ✓ and build-and-verify ✓.

## Note for reviewers
Step functions read the entry's globals … Kept as-is to stay a pure refactor.
```

## How the loop uses it

- The producer agent is instructed (in `afk/steps/step2_run_agent.sh`) to write
  `.afk/pr_title` and `.afk/pr_body` per this spec; the files are gitignored.
- `afk/steps/step3_open_pr.sh` reads them and opens the PR. If either is
  missing it re-runs the producer — it never falls back to a bare `Closes #N`.
- The review agent (planned — see
  [ADR-0006](../adr/0006-review-afk-two-loop-architecture.md)) checks every PR
  against this spec as one of its two duties; the other is content correctness.
