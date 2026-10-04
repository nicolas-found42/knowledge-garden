# Office application fixtures

These two tiny synthetic OOXML packages exercise the application import and evidence-link paths. Expected content and provenance were labeled before either package was imported by the implementation under test; see [`expected-labels.md`](expected-labels.md). `container-validation.json` records the pre-import hashes and confirms the package XML was well formed.

`field-visit.docx` has a qualified table, body text, a link to an `example.invalid` destination, and an intentionally malformed OLE relationship target. `visit-summary.pptx` has one visible slide with “12 visits,” a separate speaker note about an unconfirmed correction, an uncrawled link, and a malformed OLE relationship target. The links are preserved as references only.

The malformed payloads are not valid CFB/OLE objects and do not model their full shape and preview relationship graphs. These fixtures do not qualify strict OOXML, macro-enabled or legacy files, encryption, complex table layouts, large presentations, broad real-world parser quality, or scale. They test only the labeled cases documented above.

`general-measure.docx` is an additional minimal synthetic OOXML package for the follow-up Office statement path. Its independently frozen expected labels are appended to `expected-labels.md` before its application test. It has no event identifier or links, and the recorded Jev transport test exercises body and row support through the public `Application` import/navigation boundary. This package is intentionally small and does not claim full Word-renderer interoperability.
