# Office application fixtures

These small synthetic OOXML packages exercise the application import and evidence-link paths. Expected content and provenance were labeled before each package was imported by the implementation under test; see [`expected-labels.md`](expected-labels.md). `container-validation.json` records the original package checks.

`field-visit.docx` has a qualified table, body text, a link to an `example.invalid` destination, and an intentionally malformed OLE relationship target. `visit-summary.pptx` has one visible slide with “12 visits,” a separate speaker note about an unconfirmed correction, an uncrawled link, and a malformed OLE relationship target. The links are preserved as references only.

The malformed payloads are not valid CFB/OLE objects and do not model their full shape and preview relationship graphs. These fixtures do not qualify strict OOXML, macro-enabled or legacy files, encryption, complex table layouts, large presentations, broad real-world parser quality, or scale. They test only the labeled cases documented above.

`general-measure.docx` is an additional minimal synthetic OOXML package for the follow-up Office statement path. Its independently frozen expected labels are appended to `expected-labels.md` before its application test. It has no event identifier or links, and the recorded Jev transport test exercises body and row support through the public `Application` import/navigation boundary. This package is intentionally small and does not claim full Word-renderer interoperability.

`multi-measurement.docx` and `multi-measurement-update.docx` are minimal identity fixtures created after their labels were frozen in `expected-labels.md`. The table contains two measurements under the same fact property; the update changes only one reading. These fixtures test source scoping and stable fact keys, not renderer interoperability.

`multi-field.pptx` is a minimal one-slide package with two same-property visible field values and distinct OOXML shape IDs. Its expected identities were frozen before package generation; it tests preservation and identity granularity, not renderer interoperability.
