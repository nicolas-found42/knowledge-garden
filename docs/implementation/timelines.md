# Three actual date meanings (#25)

The desktop Timeline command reads a bounded derived index of current pages.
Source events use their current date facts. Reasoning events use acquired
reasoning-message dates and recorded owner-decision message dates; ordinary
conversation assertions are not promoted into real events. Collection activity
uses actual retained acquisition records. No absent processing timestamp or
missing event date is inferred from acquisition time.

All modes support words/title and normalized tag filters, ascending/descending
sorting and 50-entry pages. Activity tags refer to current supported source tags.
Exact dates, supplied timestamps, months, years and explicit `date to date` / `..`
intervals preserve their precision. Dates sort by interval start, then end;
unknown/conflicting dates remain last in both orders, with stable identity ties.
Acquisition entries use stable content-derived record identities and refresh on
new duplicate-source acquisition contexts as well as full rebuild.

Selecting an event reads its current page. A separate source-context action
preserves the evidence quote and conversation/message or media locator. Back
restores meaning/order/filters/page/scroll and the exact event or support button;
changing filters preserves keyboard focus. Current pages supply the timeline,
so corrections do not create fictitious new observations or obsolete alternatives.

Eight native timeline scenarios cover out-of-order V17/V18, month/year/interval,
unknown/conflicting values, dated decisions, undated reasoning, filtering,
restart/rebuild, alternate support after representative-source removal, and activity continuation past 50. The existing distinct-event
correction fixture now asserts exactly two timeline events. Reader tests cover
support/Back focus, filter state and page changes. Actual browser inspection uses
the reader connected to the Rust application driver with temporary originals,
checking all three date meanings and their visible date/provenance distinctions.

Earlier-version timeline evidence is quoted separately from current source text
and offers the exact retained original when available. A missing retained version
is disclosed instead of opening another version.
