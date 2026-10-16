# Repository ingestion

Scope: [issue #16](https://github.com/nicolas-found42/knowledge-garden/issues/16).

## Acquisition and identity

**Add repository** supplies one local folder to the common source pipeline. Every readable regular member gets a source page and an exact retained original, including unsupported, binary, generated and oversized members. Symlinks, special files, unreadable members and `.git` directories are reported as exclusions. No repository scripts, hooks, dependency installation, Git commands, cloning or linked-destination acquisition run during import. The scanner currently collects all paths before importing; progressive import remains #33.

Repository identity is derived from the canonical supplied folder path. File source identity is scoped to that repository, relative path and format; equal names or bytes in another repository do not establish semantic identity. Moving a repository to another canonical path currently gives it a different identity.

A file's **acquired revision** is the SHA-256 of its exact retained bytes. Evidence records retain source identity, content-version identity, relative file location, quote, byte range and line range. This is working-folder acquisition, not Git-history ingestion: no commit identity, clean/dirty status, repository-wide atomic snapshot or revision chronology is inferred. Files may change during the scan; each retained file's hash identifies what was actually acquired. Changed bytes enter the shared pending-version/reconciliation path, including its uncertainty and last-successful-version behavior. An unchanged repeat import reuses the source, content version and knowledge pages.

The reader's **Source information** exposes repository path/identity, relative file, full acquired-file hash, acquisition method and supported-code limits. Search results show the supporting content revision and file locator for knowledge spans. The source page and retained-original links remain usable from concept, decision and question pages. Original files open at their beginning; displayed lines/byte offsets identify the supporting passage.

## Supported content and semantic limits

Readable UTF-8 text is preserved up to the common 2 MiB text limit; larger originals are retained completely. Repository code/config extensions are enumerated by `is_text_format` in `src-tauri/src/application.rs`. Structured document/media members continue through their existing adapters and coverage contracts. Unsupported formats, decoding failures and failed semantic processing remain visible with original access.

Code concept discovery is a bounded textual candidate recognizer, not a language parser or general code comprehension system. It currently recognizes adjacent Rust `///` doc-comment blocks followed immediately by a plain or `pub` `struct`, `enum` or `trait` declaration with an ASCII uppercase initial symbol name. Comments can contain multiple lines and LF or CRLF endings; all adjacent lines, including scope qualifications, form one exact evidence span and description. Attributes, restricted visibility, functions, methods, implementations, block doc comments and other languages are not covered by this recognizer. Their acquired text remains readable; no arbitrary-language semantic capability is claimed.

Explicit Markdown `# ADR-<id>: <title>` documents can yield decision pages from their `## Decision` section, preserving the whole section's scope. Literal question bullets under `## Questions` or `## Open Questions` can yield unanswered question pages. Supplied links can yield reference pages recording **not acquired** destination content. Jev judges candidates; semantic completion describes supported spans, not exhaustive understanding of the file.

## Verification

The public Application boundary is covered by `src-tauri/tests/repository_ingestion.rs`: retained members, exclusions, no execution, separate repository/file/concept identities, changed and unchanged imports, exact evidence, ADR/question navigation and multiline qualifications. The new Rust regression first failed because only the final doc-comment line survived, then passed with both lines at source lines 1–2. It also checks revision-bound search evidence after reopening. The identity regression now checks repository and current/pending-version provenance after reopening. `src/reader.test.ts` covers the repository picker command and visible identity, revision, scope and exclusion information; its enhanced regression first failed on missing repository identity, then passed.

| Issue #16 criterion | Evidence at the public boundary |
| --- | --- |
| Common source acquisition and retained originals | `owner_supplied_repository_enters_the_common_source_pipeline`; native folder acquisition and retained-hash check |
| Navigable supported code/document knowledge and precise evidence | Code, ADR and question navigation regressions; CRLF multiline/restart/search regression; native code-concept evidence walkthrough |
| Distinct repository, file, content-version and semantic identities | `repository_identity_file_source_and_content_versions_stay_distinct_across_reimport`; `repeated_repository_symbol_names_in_different_files_keep_separate_concepts`; live distinct-ID and repeat-import checks |
| Record references without acquiring their destinations | Common-pipeline regression and live README reference marked not acquired |
| Visible binary, generated, large and unreadable coverage | Binary/oversized/generated, unreadable-file, symlink, special-file and metadata-directory regressions; native binary page and exclusion notice |
| Imported content never executes | `repository_scripts_are_retained_as_text_without_execution`; native execution marker absent |
| Synthetic repository with connected navigation and repeat import | Frozen four-file labels, real-provider import/reopen/repeat, and native synthetic-folder walkthrough below |

### Live provider and native walkthrough — October 16, 2026

Environment: Apple Silicon arm64, macOS 26.6 (25G72), Node 24.21.0, Rust 1.99.0. Built a debug Tauri application with a distinct `com.nicolasfound.knowledgegarden.issue16` identifier and **Knowledge Garden Issue16** title. Validation used only synthetic collections. Native automation used the separate identifier's Application Support collection; the normal application's collection was not the target.

The labels in `src-tauri/tests/fixtures/repositories/expected.md` were prepared before the live run. Importing only its `garden/` sibling through the real `JevSemanticProvider` produced:

| Acquired member | Observed result |
| --- | --- |
| `src/query/cache.rs` | Cache concept with both description and test-only qualification, lines 1–2 |
| `src/http/cache.rs` | A distinct Cache page describing one-request HTTP reuse, line 1 |
| `docs/adr/0001-local-plans.md` | Qualified decision, lines 4–5 |
| `README.md` | Unanswered LRU question and a reference whose destination content is marked not acquired |

The recorded provider model was `typesafe/jev-1.13-20260917`. Unchanged repeat import in a new process preserved every source/content-version/knowledge-page identity and one version per file. Exact page-ID comparison confirmed the two Cache identities differ. Jev comparison agreed overall with the labels but flagged the identity aspect for review (same-fact probability 0.82); that aspect was resolved by the deterministic distinct-ID check, not by treating the judgment as proof. Jev audit of the four grounded values passed with no flagged records. These are narrow fixed-corpus observations, not a general accuracy estimate.

For the native walkthrough, copied that fixture to a disposable folder and added a four-byte binary, a shell script that would create an execution marker, and a symlink. Through the native folder picker, imported six regular members and observed the symlink exclusion. Opened the generated query Cache concept: the full qualification, file locator, source lines 1–2, bytes 0–128 and source/original links were present. Source information showed the full acquired-file hash and repository acquisition method. Native keyword search for `temporary` returned the qualified Cache and decision with supporting file/revision locations. The binary page displayed unsupported coverage and original access. The script remained readable but semantic processing failed without candidates; the execution marker was absent. Recomputed hashes of all six retained originals matched their recorded revisions. The retained-original link was invoked; external-editor presentation was not separately qualified.

To repeat the live application-level check with configured provider credentials:

```sh
KNOWLEDGE_GARDEN_LIVE_SEMANTICS=1 cargo run --manifest-path src-tauri/Cargo.toml \
  --example application_driver -- /tmp/garden-repository-evaluation \
  repository-live-import src-tauri/tests/fixtures/repositories/garden
```

`repository-import` performs the same acquisition without requesting semantic processing. Provider credentials stay in the existing Rust environment/keychain path. Collection-scale throughput, broad semantic quality and the complete installed release remain the separate #33/#34/#35/#36 gates.

### Final automated checks and review

Code commit `22f3470` passed `npm test`: **40 UI/application tests and 163 Rust tests**, with zero failures. `npm run build` (including TypeScript checking), `npm run format:check`, and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` passed. The subsequent documentation-only commit adds the criterion map and this check record. Independent standards and specification inspections each reported zero actionable findings.

Two earlier full-suite attempts timed out: the application test's Cargo-build step waited behind the native build, and a later application-driver invocation exceeded its 10-second limit. The isolated six application tests and final complete suite passed without changing timeouts. A strict-Clippy unused-`mut` finding in the new restart assertion was fixed before the final successful checks. Native/live observations above remain bounded separately from these automated checks.
