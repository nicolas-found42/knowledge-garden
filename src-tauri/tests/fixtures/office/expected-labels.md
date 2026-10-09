# Independent expected labels for ticket 12 fixture

Labels frozen before DOCX/PPTX construction and before any extraction implementation runs. These fixtures are synthetic; all names, counts, and destinations below are invented.

## DOCX: qualified table entry

- File: `source/field-visit.docx` (modern Word OOXML package, `.docx`).
- Main document title: `Riverside Field Visit`.
- Heading 1 `Observed results` precedes a two-column table headed `Measure` / `Result`.
- Row 2 measure `Transect visits`; result `12 visits, excluding two unverified reports.` This qualification is part of the cell value. Expected extraction must keep measure/value together and preserve the exclusion qualification; extracting bare `12 visits` as the whole fact is incomplete.
- Row 3 measure `Observation period`; result `May 4, 2026, 09:00–11:30 local time.`
- Body paragraph: `Counts include only visits confirmed by two observers.`
- An external hyperlink to `https://example.invalid/unacquired-methods` appears in the document. Expected treatment: retain link text and URL occurrence; do not fetch destination content.
- The package contains a relationship-referenced embedded OLE object whose payload is intentionally not a readable/valid Office object. Expected treatment: retain DOCX; disclose unreadable embedded-object coverage, do not discard document text or pretend object contents are known.

## PPTX: visible slide versus note

- File: `source/visit-summary.pptx` (modern PowerPoint OOXML package, `.pptx`).
- Exactly one slide (slide 1 in deck order); title `Field visit summary`.
- Visible slide shape text: `12 visits`. This is the slide's visible reported value; it is not itself confirmation of any later correction.
- Speaker notes for slide 1: `An unconfirmed correction suggests the count may be 14 visits. Do not replace the slide's visible count until the correction is verified.` Expected extraction must record this as speaker-note content with note origin, keep it distinct from visible slide text, and preserve its unconfirmed qualification.
- A visible hyperlink to `https://example.invalid/linked-method` is a linked occurrence only. Do not fetch the target.
- The package contains a relationship-referenced embedded OLE object with intentionally unreadable payload. Expected treatment: retain the presentation and visible slide/notes; disclose unsupported/unreadable embedded object coverage.

## Shared integrity expectations

- Preserve each original byte-for-byte through a future importer; these fixture hashes can be recorded before/after that importer.
- Preserve source order and evidence locations: DOCX paragraph/table cell; PPTX slide index/order and distinguishable speaker-notes location. Slide-visible text and notes must not be collapsed into one unqualified stream.
- A parser must not run, acquire, or dereference either `.invalid` link. They are reserved example-domain strings and do not designate actual resources.
- Extraction omissions caused by the malformed embedded object must not be interpreted as evidence that the rest of the document/presentation has no content.
# Frozen follow-up labels for general Office proposition formation

Frozen before constructing this added synthetic package and before its application extraction/semantic tests.

- File: `general-measure.docx`, a modern Word OOXML DOCX with no event identifier and no external link.
- Body statement: `Sensor drift left the reading provisional until the second instrument check.` It is a source statement, with provisional qualification preserved and body paragraph provenance.
- Table columns: `Measurement`, `Result`, `Review`.
- Table row: `Dissolved oxygen` / `7.4 mg/L` / `Provisional pending a second-instrument check.` The extracted knowledge must retain the header/value pairing and exact row qualification, not assert a bare unqualified measurement.
- A source-document knowledge page should exist from the admitted statements even though this sample contains no Vxx identifier, no named visit/event, and no URL.
- All facts must quote exact extracted text and identify the DOCX paragraph or table row projection locator. The original must remain byte-identical.

This synthetic fixture tests generic Office-channel processing and is not representative owner data, a quality claim, or a full Office-format qualification.

## Multi-measurement source-scoped identity fixture (frozen before package generation)

- File: `multi-measurement.docx`, generated only after freezing these labels.
- One Word document contains two table measurements under the same `Measurement` property: `Water temperature` / `12 °C` and `Dissolved oxygen` / `7.4 mg/L`.
- The source-level knowledge pages must stay scoped to the distinct imported document source, and both table-row facts must be retained with distinct stable record keys grounded in the `Measure` field/context, not the reading value or row ordinal.
- A later source version changes only water temperature from `12 °C` to `15 °C`; its stable record key and fact identity must remain the same while the supported value updates.
- The original package bytes must remain unchanged after import.

This fixture tests Office statement granularity and update identity through the public Application API; it is synthetic and does not qualify semantic extraction generally.

## Multi-field presentation identity fixture (frozen before package generation)

- File: multi-field.pptx, one slide with two visible labeled values: Water temperature: 12 °C and River stage: 2.4 m.
- Both visible passages must remain separate facts with distinct stable record keys grounded in their OOXML shape IDs and field names.
- The source page must identify this specific presentation source, retain both facts, and keep their origin as visible slide text.
- The original presentation bytes must remain unchanged.

This fixture tests two same-property visible slide facts and shape-identity preservation through the public Application boundary; it is synthetic and not a semantic quality claim.
