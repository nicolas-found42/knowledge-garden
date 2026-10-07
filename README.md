# Knowledge Garden

A local Mac application for importing text sources into a durable Markdown collection. This first slice implements [issue #2](https://github.com/nicolas-found42/knowledge-garden/issues/2): choose or drop a file, read its source page, and inspect its retained original.

Choose **Add source**, or drop a file anywhere in the window. **Sources** opens the collection's page list. **Open original** resolves to the copied asset, and **Source information** reveals acquisition context, identities, and processing coverage. The last opened page is restored on restart. The reader has no authoring editor or chat.

UTF-8 `.txt`, `.text`, `.md`, `.markdown`, and extensionless files are supported, including empty files, BOMs, and CRLF line endings. Text is preserved verbatim in fenced Markdown with original line locations; Markdown source syntax is shown as source text in this slice. Unsupported extensions, invalid UTF-8, and binary control characters produce readable source pages with explicit extraction states. Previews are limited to 2 MiB, but originals are copied completely with bounded memory. Semantic extraction and facts belong to subsequent work.

## Run and build

On macOS, install Rust, Node.js 22.12 or later, CMake, and the [Tauri macOS prerequisites](https://v2.tauri.app/start/prerequisites/#macos). The first macOS build initializes the pinned Whisper source submodule, downloads the pinned large-v3-turbo model, verifies its SHA-256, and packages the model with a static Metal-enabled runner. Runtime transcription is local and makes no model download; the model and runner license details are bundled with the app.

```sh
npm ci
npm run tauri dev
```

Build the application bundle:

```sh
npm run tauri build -- --bundles app
```

The bundle appears in `src-tauri/target/release/bundle/macos/Knowledge Garden.app`. This development foundation does not configure Developer ID signing or notarization.

The default collection lives under the application's data directory, normally `~/Library/Application Support/com.nicolasfound.knowledgegarden/collection`. For development or synthetic UI checks, launch with `KNOWLEDGE_GARDEN_COLLECTION` set to an empty temporary directory. The app opens one collection per process and rejects a second concurrent writer.

```sh
KNOWLEDGE_GARDEN_COLLECTION=/tmp/knowledge-garden-demo npm run tauri dev
```

## Verify

```sh
npm run typecheck
npm run format:check
npm run check:rust
npm test
```

Application tests supply temporary collections and inspect public import, page navigation, retained originals, Markdown, and restart/rebuild outcomes. Reader tests use deterministic responses at the same desktop command boundary. They do not establish native file-dialog, drag delivery, or live model behavior; see [implementation and validation notes](docs/implementation/local-text-import.md) for the checks actually performed.

See [the durable source contract](docs/implementation/source-contract.md) for storage, identities, recovery, and this slice's limits.
