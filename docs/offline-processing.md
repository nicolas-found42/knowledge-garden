# Offline processing and service recovery

Source originals and available local extraction are retained and searchable before
remote knowledge formation finishes. A new source can display its local projection
while processing remains pending; an update with a published version keeps the
last successful version visible. Previously published pages and the local meaning
index remain usable after an offline restart. Derived indexes can be rebuilt from
retained Markdown without deleting pending jobs.

Known provider availability problems remain pending with bounded exponential
backoff instead of spending the finite invalid-response or extraction-failure
budgets. Network failures, authentication, quota/credits, selected-model
availability, and capability/permission failures are disclosed separately. For
HTTP 429 and 503 (and any other response carrying it), a valid `Retry-After`
sets a minimum next-attempt delay; that deadline is stored with the source. The
app continues to use its selected Jev model and does not silently switch providers.
Provider response bodies and credentials are not copied into error messages.

Deterministic tests exercise real HTTP status/envelope handling, a durable
`Retry-After` deadline across restart and disposable-index loss, five network
outages, local keyword search, current-version publication, and protection against
stale updates. A focused reader test checks first-processing wording without
claiming a previous successful version.

On the 48 GB Apple Silicon development host, a synthetic report was imported
through the public Rust application interface with an unreachable per-process
proxy. It retained its original, remained pending with a network cause, and was
found by keyword. Restoring connectivity completed the same source and version
through the real Jev integration, producing three knowledge pages without a
provider-failure budget charge. A subsequent process restarted with that proxy and
returned four results from the local meaning index. This is an integration smoke,
not an independent model-quality evaluation or the final 16 GB qualification.
Installed-app connectivity behavior remains to be checked without interrupting the
owner's current session.
