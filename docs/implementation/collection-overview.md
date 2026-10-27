# Collection overview

Issue #22 adds a collection overview built from current knowledge subjects and
supported tags. These are the initial automatic topics: Jev's existing source
processing supplies their identities, evidence and qualifications. Group membership
indicates shared source support or an accepted tag; it does not infer an additional
identity, causal relationship or spatial connection.

Person, place, source-event date and source-format axes organize the same pages.
Sources without published knowledge remain in an explicit unclassified/processing
group. An authoritative owner page with unavailable source support remains reachable
as its own knowledge subject. Unknown event dates stay unknown.

Group and member queries return at most 50 results plus continuation. SQL computes
counts and filters on disk. A source neighborhood expands to its current supported
pages. Page content and original evidence load only when selected. The canvas draws
the current group window and one expanded member window, with a structured keyboard
alternative exposing the same entries and continuation controls. Reading, research
and canvas presentations share filters, expansion, member position and selection;
Back restores that state after reading a page.

`collection_source_pages` is derived from each source page's published knowledge
references. Updates compare a source's membership set and change only affected rows;
unchanged sources are not rewritten. The organization revision hashes current page,
tag and membership inputs and remains stable across a cache rebuild. A continuation
against a changed revision restarts its member window. Existing general index rebuild
costs remain part of the larger import/scale work in #33 and #35; this overview does
not establish target-scale performance.

`collection_groups.rs` verifies overlapping subjects/tags, unclassified access,
replacement/rebuild behavior, stable revisions, more than 50 members, literal
filters and affected-source-only membership writes. Reader/component tests verify
presentations, keyboard reading, Back, focus and late responses. A native application
driver test crosses the real Rust/frontend boundary; a Jev-browser check also opened
an actual retained source from a canvas group and returned to its selected context.
