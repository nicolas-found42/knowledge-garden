# Local text import: implementation and validation

[Issue #2](https://github.com/nicolas-found42/knowledge-garden/issues/2) establishes the first complete collection workflow. The [source contract](source-contract.md) defines what stays on disk and which parts are derived. This slice preserves source text; it does not extract facts or relationships.

## What the application does

- **Add source** uses the native macOS file picker. A file dropped on the app window enters the same import operation with `drop` as its acquisition method.
- The Rust application streams the supplied file into a staging bundle, hashes its bytes, writes a Markdown source page, then publishes the bundle. Reimporting the same bytes with the same normalized extension reopens the same source and page. Another acquisition context gets its own Markdown record without rewriting the source page.
- The reader shows the source page, opens its retained original with macOS file handling, and reveals processing state and acquisition context on demand. It restores the last opened page after restart. The lookup index can be rebuilt from committed Markdown.
- Unsupported formats, invalid UTF-8, binary control characters, and text beyond the 2 MiB preview limit keep their complete originals and show explicit coverage states.

## Checks performed on the 16 GB Apple Silicon Mac

On October 2, 2026, macOS 26.4.1:

- `npm run typecheck`, `npm run format:check`, the reader test file, the application test file, and the Rust collection workflow test file passed.
- `npm run tauri build -- --bundles app` produced `Knowledge Garden.app` and launched from its bundled executable with a disposable collection.
- The native picker imported a 58-byte UTF-8 text file. The visible page showed its title, source text, line location, source information, and acquisition method. A 4-byte `.bin` file showed an unsupported state and an original link.
- The retained originals matched both supplied files byte for byte. After stopping and relaunching the bundled app, the unsupported page reopened and **Sources** listed both pages.
- The reader test drives a drop event through the public `GardenApi.onDrop` boundary and verifies that it imports with the `drop` method. A direct Finder-to-app drag was not completed during the packaged-app check, so native drop delivery remains unverified by that manual check.

The application test uses a real Rust application driver with a temporary collection and exercises page display, original bytes, restart, duplicate import, unsupported coverage, and lookup-index deletion. Its file picker and drop event are test doubles. The Rust workflow tests cover atomic bundle publication, duplicate identities, external page edits, coverage states, bounded previews, source-safe Markdown, collection locking, and index reconstruction. These checks do not establish later semantic extraction, full collection-scale performance, signed installation, or notarization.
