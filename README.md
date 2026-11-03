# Knowledge Garden

A Mac application for retaining sources, finding their contents and reading source-backed knowledge. Original files are preserved in a durable collection. Derived pages are maintained by the application; adding and removing sources are the content changes available to the user.

Choose **Add source**, or drop a file anywhere in the window. **Sources** opens the collection's page list. **Open original** resolves to the copied asset, and **Source information** reveals acquisition context, identities, and processing coverage. The last opened page is restored on restart. The reader has no authoring editor or chat.

**Add repository** imports a supplied local folder with retained originals and file-scoped evidence. Supported Rust doc comments, explicit Markdown decisions and questions can become navigable knowledge pages. Source information shows the repository, relative file, exact acquired-content revision and coverage limits. See [repository ingestion and validation](docs/implementation/repository-ingestion.md) for the supported forms and native/live checks.

Files are accepted even when their format cannot be interpreted. [Source processing coverage](docs/implementation/file-coverage.md) describes the supported document and media routes, their limits, and the distinction between retaining an original and understanding it. Extraction and model output can be incomplete or mistaken; inspect the linked evidence.

**Pivot in progress:** migration and removal safety, large-source processing, and final application acceptance are still being completed. Do not use this development checkout to migrate or remove sources from a valuable collection until those gates pass. See [the acceptance matrix](docs/pivot-acceptance.md).

## Run and build

On Apple Silicon macOS 26 or later, install Xcode 26 Command Line Tools, Rust via rustup, Node.js 24 (see `.nvmrc`), CMake, and the [Tauri macOS prerequisites](https://v2.tauri.app/start/prerequisites/#macos). The native helper uses macOS 26 speech APIs. rustup selects the pinned toolchain in `rust-toolchain.toml`. The first macOS build initializes the pinned Whisper source submodule, downloads the pinned large-v3-turbo model, verifies its SHA-256, and packages the model with a static Metal-enabled runner. Runtime transcription is local and makes no model download; the model and runner license details are bundled with the app.

Install Ollama for macOS and download `qwen2.5vl:7b` with `ollama pull qwen2.5vl:7b` while Ollama is running. The visual worker currently expects `/usr/local/bin/ollama` and starts its own local service for inference; it does not require an open terminal session. Qwen interprets images locally, while bundled Whisper transcribes audio locally. See [the model experiment](docs/research/pivot-model-experiment.md) for measured limits and unresolved qualification.

Semantic judgments use `typesafe/jev-1.14` remotely through OpenRouter. Supply `OPENROUTER_API_KEY` in the app's environment or store a generic password in macOS Keychain with service `Knowledge Garden Provider Keys` and account `openrouter`. Do not put the key in the collection or source files. [Provider policy](docs/implementation/provider-policy.md) describes what evidence may be sent and the current minimization limitation.

**Processing setup** offers separate on-demand checks for the local model and Jev connection. Both checks use built-in test content; a successful check confirms a response, not the accuracy of future interpretations. The local check loads the model and can take time and memory.

```sh
npm ci
npm run tauri dev
```

Build the application bundle:

```sh
npm run package:app
```

The bundle appears in `src-tauri/target/release/bundle/macos/Knowledge Garden.app`. This development foundation does not configure Developer ID signing or notarization.

The default collection lives under the application's data directory, normally `~/Library/Application Support/com.nicolasfound.knowledgegarden/collection`. For development or synthetic UI checks, launch with `KNOWLEDGE_GARDEN_COLLECTION` set to an empty temporary directory. The app opens one collection per process and rejects a second concurrent writer.

```sh
KNOWLEDGE_GARDEN_COLLECTION=/tmp/knowledge-garden-demo npm run tauri dev
```

## Verify

```sh
npm run provision:assets
npm run check
```

Application tests supply temporary collections and inspect public import, page navigation, retained originals, Markdown, and restart/rebuild outcomes. Reader tests use deterministic responses at the same desktop command boundary. They do not establish native file-dialog, drag delivery, or live model behavior; see [implementation and validation notes](docs/implementation/local-text-import.md) for the checks actually performed.

See [CONTRIBUTING.md](CONTRIBUTING.md) for lint prerequisites, individual development checks, CI, and required pull-request checks on `main`.

See [the durable source contract](docs/implementation/source-contract.md) for storage, identities, recovery, and this slice's limits.

For recording import, local transcription, version-bound timestamp evidence, playback fallback, and measured limits, see [audio recordings](docs/implementation/audio-recordings.md).
