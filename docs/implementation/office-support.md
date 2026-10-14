# Office import support

The Office importer reads modern, unencrypted OOXML Word (`.docx`) and PowerPoint (`.pptx`) packages directly. It retains each package byte-for-byte and does not convert or rewrite the original. Links found in package relationships are recorded as references; the importer does not request their destinations.

| Format | Projected content | Provenance shown by the reader | Known coverage limits |
| --- | --- | --- | --- |
| `.docx` | Main-document headings and paragraphs; tables in row and cell order with header/value pairing | WordprocessingML part and paragraph, heading, table, row, and cell locator | Embedded objects and media are retained but not decoded; unsupported package parts are reported as gaps. |
| `.pptx` | Slides in presentation order; visible shape text and DrawingML table rows; linked speaker notes | Slide number, slide part and shape where available; notes have a separate speaker-notes channel | Embedded objects and media are retained but not decoded; missing slide or notes relationships are reported as gaps. |

All other extensions, including legacy `.doc` and `.ppt`, macro-enabled packages, encrypted files, and standalone spreadsheet packages, are outside the supported matrix. Malformed or unsupported containers remain in the collection with their original and an explicit extraction state. Partial coverage is not treated as evidence that omitted content was absent.

The labeled application corpus in [`src-tauri/tests/fixtures/office/README.md`](../../src-tauri/tests/fixtures/office/README.md) exercises qualified table rows, source ordering, slide-versus-note origin, malformed embedded-object references, uncrawled links, multiple fact identities, and one same-source revision. It consists of synthetic OOXML packages and qualifies those controlled behaviors only. Local owner-format checks recorded separately exercised a 236-run DOCX, a 19-slide/332-run PPTX, and an invalid `.docx` container without transmitting private files. They establish package readability, text projection, coverage disclosure, and exact original retention only; they do not establish semantic accuracy or broad format compatibility.
