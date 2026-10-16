# Development

Knowledge Garden's native helper uses macOS 26 speech APIs. Use Apple Silicon
macOS 26 or later with Xcode 26 Command Line Tools,
[Rust via rustup](https://rustup.rs/), Node 24 (see `.nvmrc`), and the native and
lint prerequisites:

```sh
brew install cmake actionlint shellcheck
npm ci
npm run provision:assets
npm run tauri dev
```

rustup automatically selects the version, rustfmt and Clippy declared in
`rust-toolchain.toml`. Commit both `package-lock.json` and `src-tauri/Cargo.lock`
when changing dependencies. Use `npm ci` for a clean install; Cargo validation
uses `--locked` so it cannot silently rewrite dependency resolution.

Asset provisioning downloads roughly 2 GiB, verifies pinned SHA-256 hashes, and
builds the pinned Whisper submodule. It requires network access at setup time;
the local inference tests use the bundled files. Provider tests use recorded
transports or local HTTP servers and require no API keys.

## Before opening a pull request

```sh
npm run format
npm run check
```

`check` runs formatting, ESLint, ShellCheck, actionlint, the production frontend
build (including TypeScript checks), Vitest, Clippy and every Rust test target.
Run `npm run package:app` to provision assets and build a release `.app` bundle.
CI also builds an unsigned debug bundle and checks its embedded resources;
signing, notarization and interactive permission prompts need separate release
validation.

On Linux, run `npm run test:ui:unit` for the reader tests alongside the frontend
and static commands. `npm run test:ui:integration` compiles the Rust application
driver and runs on Apple Silicon macOS with the native tests, inference
resources and app packaging. A Linux frontend pass alone does not establish
native correctness.

## CI and merging

Every pull request targeting `main` runs `.github/workflows/ci.yml`. Its Linux
job validates frontend code and tooling, including reader unit tests. Its macOS
ARM job provisions assets, runs native checks and tests plus frontend integration
tests against the real Rust application driver, and builds the app bundle. The
`CI required` job runs after both and fails unless both succeeded, including when a dependency
was skipped or cancelled. There are no path filters that exempt documentation
or tooling changes from the gate. Superseded runs are cancelled automatically.

Native CI runs Rust tests serially on the standard three-core runner. The AAC
inference case runs first; the remaining test invocation excludes only that
already-executed case. The full pinned model, two-minute segment deadline and
transcript assertions remain intact. Verified model downloads and compiled
dependencies are saved even if a later test fails.

The repository's `main` protection requires a pull request, an up-to-date branch,
`CI required` from GitHub Actions, and resolved review conversations, including
for administrators. No additional approving reviewer is required for this
single-owner project. Force pushes and branch deletion are disabled.

The intended protection is versioned in `.github/main-protection.json`. Editing
that file does not change GitHub settings. An administrator can apply it with:

```sh
gh api --method PUT repos/nicolas-found42/knowledge-garden/branches/main/protection \
  --input .github/main-protection.json
gh api repos/nicolas-found42/knowledge-garden/branches/main/protection
```

Keep the required job name and this configuration in sync. A failing check must
be fixed before merging. Workflow runs have read-only repository permissions
and use no signing or provider secrets. Action revisions are pinned to full
commit hashes; weekly Dependabot PRs update npm, Cargo, the Rust toolchain and
actions. Those PRs pass through the same CI gate. Review changes to native model
or Whisper pins and checksums together. Dependabot alerts and security-update
PRs are also enabled in repository settings.

The rationale and upstream references are in
[CI and development tooling research](docs/research/ci-and-dev-tooling.md).
