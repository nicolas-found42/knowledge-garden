# Domain Docs

How engineering skills consume this project's domain documentation.

## Before exploring, read these

- Read `GLOSSARY-MAP.md` at the repo root, then the project-wide `GLOSSARY.md` and each context glossary relevant to the topic. If the map is absent, read the root `GLOSSARY.md` directly.
- Read ADRs in `docs/adr/` that touch the area being explored. If the map links a context-specific ADR directory, also read relevant decisions there.

If any of these files are absent, proceed silently. Domain-modeling creates glossary entries and ADRs as terms and decisions are resolved.

## File structure

The glossary map currently points to one project-wide context. Add separate contexts only as the project's domain structure requires them.

```text
/
├── GLOSSARY-MAP.md       # Index of domain contexts and their documentation
├── GLOSSARY.md           # Project-wide terms
├── docs/adr/             # Project-wide architecture decisions
└── docs/agents/          # Engineering skill configuration
```

When adding a context, add its glossary and ADR directory to `GLOSSARY-MAP.md`. Keep shared vocabulary in the project-wide glossary and system-wide decisions in `docs/adr/`.

## Use the glossary's vocabulary

When output names a domain concept in an issue title, refactor proposal, hypothesis, or test name, use its defined glossary term. Avoid synonyms the glossary explicitly excludes.

If the concept is absent, reconsider whether it belongs to the project's vocabulary or note the gap for domain-modeling.

## Flag ADR conflicts

When a proposal contradicts an existing ADR, identify the decision and explain why it should be reconsidered before proceeding.
