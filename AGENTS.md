## Subagent model

Use either `gpt-6.1-sol` or `gpt-6-luna` with `low` (light) or `medium` reasoning for all subagents, including nested subagents. Explicitly set an allowed model and reasoning effort when spawning a subagent. Custom agents must use these same allowed settings. If no allowed combination is available, do the work in the main agent.

## Agent skills

### Issue tracker

Track issues and specs in GitHub Issues for `nicolas-found42/knowledge-garden`. Before issue operations, read `docs/agents/issue-tracker.md`.

### Triage labels

Use the five default triage labels. Before triaging or applying triage labels, read `docs/agents/triage-labels.md`.

### Domain docs

Use `GLOSSARY-MAP.md` to locate domain glossaries, starting with the project-wide `GLOSSARY.md`; record project-wide decisions in `docs/adr/`. Before exploring a domain or recording a decision, read `docs/agents/domain.md`.
