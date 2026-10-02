---
status: accepted
---

# Reconcile sources automatically into current knowledge

The owner supplies sources and then reads, searches, and navigates; normal operation must not require semantic review or curation. Reconcile affected facts automatically using source-version order: complete replacements add, change, and remove solely supported facts, targeted corrections change only their scope, and separate events remain separate, with one current value per fact and no user-facing replay of obsolete values. Preserve authoritative manual corrections, distinguish incomplete extraction or source unavailability from deletion, and recover failed updates automatically under the rules and concrete examples in [Specification #1](https://github.com/nicolas-found42/knowledge-garden/issues/1).
