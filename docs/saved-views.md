# Saved views

Existing saved views can be opened for reading and navigation. Creating, renaming, removing, or saving persistent view definitions is no longer offered; ordinary navigation and last-session presentation remain temporary. This follows [ADR 0009](adr/0009-sources-are-the-content-authority.md).

Each `views/*.md` file has a stable UUID identity, a meaningful name, ordinary links to referenced material, and authoritative YAML criteria. The definition covers applied search filters, typed Boolean relationship clauses, collection grouping/presentation/expanded scopes, timeline meaning and sorting, relationship navigation, and explicit comparison selections. Unsaved form drafts, result sets, cursor tokens, signatures, indexes, and session position are excluded. Opening starts at the first result page and evaluates current data. Selecting Search, Timeline, or Collection then uses the restored arrangement's applied criteria.

Malformed or unsupported definitions remain visible as diagnostics with an option to open their Markdown; duplicate identities are never resolved by arbitrarily choosing a file. Missing references retain their scope and produce warnings. Current names are resolved by stable identity; a missing start never becomes an unscoped query or an unrelated same-named item.

Records are bounded to 256 KiB, catalog result pages to 50 entries, and criteria use the existing query work/page budgets. Historical installed-app checks used a disposable collection and frozen provider results; they do not establish final pivot acceptance.
