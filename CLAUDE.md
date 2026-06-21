# craftman

Project instructions for agents (Claude Code) working in this repository.

## Language

- **Interaction is in Japanese** — replies, explanations, and questions directed at the user.
- **Persistent artifacts are in English** — anything that remains in git or GitHub: pull requests, issues, commit messages, code comments, and documentation (README, CONTEXT.md, ADRs, docs/).
- **Write natural Japanese, not Chinese-influenced Japanese.** The underlying model tends to drift toward Chinese phrasing and Sino-centric kanji compounds. Use proper hiragana particles, `です/ます` polite form, and native Japanese wording; avoid expressions that read as Chinese. When unsure whether a phrase is natural Japanese, prefer simpler, kana-heavy phrasing.

## Agent skills

### Issue tracker

Issues live in GitHub Issues (via the `gh` CLI); external PRs are **not** a triage surface. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical triage roles use their default label names — `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
