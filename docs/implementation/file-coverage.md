# Source processing coverage

Knowledge Garden retains accepted original bytes even when it cannot interpret them. An extension identifies a processing route; it is not proof that a file is valid or that every part was understood. Failed, unsupported, encrypted and partial processing must remain visible. References inside a source do not authorize fetching their destinations, and imported code, formulas and macros are never executed.

| Family | Current processing | Important limits |
| --- | --- | --- |
| Plain text, Markdown, code, CSV, JSON, XML and other configured text extensions | UTF-8 text is retained and projected as text. Code and structured data are not executed. | The current primary checkout has a 2 MiB text limit; larger-source processing is under development. Structured formats are not necessarily semantically parsed. |
| PDF | macOS PDFKit text extraction and Vision OCR provide page-located text. | OCR is fallible. Missing page coverage must not be interpreted as absence of content. |
| DOCX and PPTX | OOXML paragraphs, tables, slide text and available notes are projected with locations. | Embedded content and unsupported package parts may be omitted and reported as gaps. |
| XLSX | Supported cell values and cached formula results are projected with sheet and cell locations. | Formulas are not run and cached values may be stale. Workbook, part, sheet and cell limits apply. Supported Agile encryption is identified but not decrypted. |
| JPEG, PNG, HEIC and other supported image formats | Local ImageIO and Vision provide metadata, OCR and bounded image analysis; local Qwen can add an attributed visual interpretation. | Model interpretations and OCR can be wrong. Photo regions and whole-image fallbacks have different precision. |
| Audio and supported video containers | Local Whisper transcribes audio in bounded intervals. Video processing extracts frames for local visual interpretation and keeps audio and visual evidence separate. | Transcript timestamps are estimates; speakers and non-speech sounds are not reliably identified. Model outputs are unverified. |
| Unknown, corrupt, unsupported or encrypted material | Original bytes are retained with a processing outcome and reason where recognized. | Retention does not mean the contents were understood. |

Email and general archive processing are being tested in an isolated implementation and are not yet integrated into the primary checkout. The limits above describe the current implementation and are not a broad format compatibility guarantee. See [Office support](office-support.md), [photo support](photo-support.md), [audio recordings](audio-recordings.md), and [video support](video-support.md) for detailed coverage and historical qualification evidence.
