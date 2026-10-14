# Knowledge Garden spec #1 — handoff #2 (continue WITHOUT subagents)

Saved 2026-10-03. Supersedes nothing: read the earlier checkpoint first, then apply this delta. Operational checkpoint, not a spec.

- Earlier checkpoint (authoritative for paths, authorizations, evidence pointers, constraints): `/var/folders/vj/z460ltg56p122lsl4_s8pd140000gp/T/knowledge-garden-spec1-handoff-20261003.md`. If it is gone, `R/shared-context.md` plus GitHub #1 and child issues hold the rest.
- `R` = `$HOME/Documents/agent-runs/knowledge-garden-spec1-20261003`; `I` = `$HOME/.codex/worktrees/spec1-integration-20261003/knowledge-garden`; main = `$HOME/Documents/github/knowledge-garden`.

## Instruction change for the next agent

**Do not spawn subagents.** The user cancelled all three implementers and asked for a continuation without delegation. Do the implement, merge, and verify work yourself, one ticket at a time, in the preserved worktrees. This overrides the implement-spec rule about implementer/merger subagents. Keep the structure otherwise: each ticket on its own branch/worktree, merged into the integration branch with a deliberate no-ff merge, and don't merge a branch you just implemented without re-running the union gates first. Everything else persists: the whole pipeline runs without check-ins, TDD seams already approved, Ollama Cloud `deepseek-v4.1-flash` override, fixed synthetic live data only, no private uploads, no PR, no main merge, parent #1 body unchanged, user's main-checkout changes untouched (modified AGENTS.md, untracked `.scratch/` and `docs/research/` — this file lives in `.scratch/` at the user's request).

## What happened this session

1. Verified state matched the earlier checkpoint. Integration local HEAD `da2db8c86a4da8a775bc6471e2924e86de4c195e` (clean), remote `0dfb4f99…`, base/main `5acda689…`.
2. **Wrong merge subject on `da2db8c`** ("integrate Ollama cloud provider adapter", actually the #18 followup): decided to keep history, not rewrite, because photo and Ollama branches already contain it. Correction is recorded in the pin's `build-identity.json`. Mention it in the next integration merge body.
3. Ran the missing `npm run build` at `da2db8c`: passed. Rebuilt the debug driver: SHA256 `572fb4c7c5b41b281ba663c402301c0df5950bb2c5d88f51758d9c8c244bc133` (matches merger's report). Pinned read-only at `R/evaluation-binaries/da2db8c86a4da8a775bc6471e2924e86de4c195e/{application_driver,build-identity.json}`.
4. Fresh live reconciliation (`R/verify-reconciliation-live.py`, env `KG_APPLICATION_DRIVER` = pinned copy, `KG_EVALUATION_COMMIT` = full SHA) → `R/reconciliation-live-da2db8c8/verification.json`: **passed=true, all 16 cases**, including obsolete 12-visit value absent from ordinary search, four same-path adversarial controls, and independent-support provenance. This resolves the earlier superseded-search failure for #4 at the integration commit.
5. Dispatched three implementers, then **stopped them (killed) before any reported**. Their output files held no final reports. No live processes remained afterward (checked with `ps`). No commits were made by them; all their work is uncommitted and unverified.

## Uncommitted in-progress state to inspect (all unverified, nothing compiled/passing claimed)

- `R/ticket-12-role-followup` (branch `codex/spec1-ticket12-role-followup-20261003`, HEAD `0dfb4f99`): dirty `src-tauri/src/{application,desktop,providers,semantic}.rs`, `src-tauri/tests/collection_workflow.rs` (5 files, ~456+/26-), plus untracked `src-tauri/tests/fixtures/reconciliation/` (copies of the frozen same-path inputs: baseline-revision-1, conditional/fact-only/negated/quoted replacement). Agent had begun the semantic.rs role-context types when stopped. Pending work is unchanged from the earlier checkpoint: whole-source role context via Application, source_update OOXML locator/origin, and a separate commit for test infra (`CARGO_BUILD_JOBS=2` + longer cold-build timeout in `application.test.ts`). Design: `R/ticket12-source-role-scope-investigation.md`. Inspect `git diff` before trusting any of it.
- `R/ticket-13` (branch `codex/spec1-ticket13-20261003`, HEAD `dc9c51e`): same dirty set as the earlier checkpoint (application.rs, lib.rs, semantic.rs; untracked fixture-context.md, fixture-manifest.json, photo.rs, photo_native.swift). Agent made no visible progress before the stop. Still has the suspected 3-vs-4 tuple compile error in the changed-source photo branch and an uncompiled Swift preview path.
- `R/run/ollama-cloud-provider` (branch `codex/spec1-ollama-cloud-provider-20261003`, HEAD `a8c924f`): now dirty — `src-tauri/Cargo.toml` + `Cargo.lock` (+2 lines, a dependency was added), untracked `src-tauri/src/ollama.rs` (140 lines), `src-tauri/tests/ollama_cloud.rs` (207 lines), `docs/adr/0006-route-helper-generation-through-ollama-cloud.md` (19 lines). Not compiled or run; `lib.rs` module wiring not confirmed. The #10 durable acquisition retry has not been started.
- Retained clean worktrees unchanged: R/ticket-4 (d03e8b7), R/ticket-10 (52490ad), R/ticket-12 (d72d466), R/ticket-18 (bc0d9a7), R/run/ticket-18-followup (780b5da).

Recommended order (smaller and blocking first): Ollama adapter (photo depends on it) → Office role followup → photo → #10 retry. After each, merge integration tip into the branch, run the full union gates, then merge no-ff into integration.

## Native qualification limit

This session has **no desktop-automation (computer-use) tool**, only the in-app browser pane and iOS simulator tools. Native packaged-app proof (#18 search/history/offline under WKWebView, #32 keyboard/VoiceOver, Keychain prompt, Obsidian) could not be driven here. Do not substitute another automation path; if the next runtime also lacks one, record those items as unproven gaps on the tickets rather than claiming them. Earlier CUA/SecurityAgent denial was a tool denial: never bypass.

## Issue graph (refreshed via gh this session)

Open: #1 and #4–#36. #2, #3 closed. Native blockers (live): #5←#4, #6←#4, #7←#5, #8←#7, #9←#7, #11←#6, #15←#14,#13, #19←#18, #20←#18, #21←#20, #22←#20, #23←#20, #24←#6,#20, #25←#17,#5, #26←#22,#21,#25, #27←#26, #28←#9, #29←#19, #30←#8,#19,#27, #31←#27,#8,#15,#11,#12,#16, #32←#24,#23,#27, #33←#9,#18, #34←#16,#15,#27,#12,#29,#10,#24,#11, #35←#30,#33,#28,#29, #36←#35,#34,#31,#32. Unblocked and unassigned now: #14, #16, #17 (fixtures at `R/ticket-14-fixture`, `ticket-16-fixture`, `ticket-17-fixture`). Assigned to the user: #4, #10, #12, #13, #18. Close only with acceptance evidence; closures were deliberately held until the Office fix merges and live suites rerun (it touches the same source-role code). #4 and #18 have recorded-check evidence plus the new live pass above; their native gates remain.

## Practical constraints (still apply)

16 GiB M5 Air: `CARGO_BUILD_JOBS=2`, set before `npm run test:ui`; private `node_modules` and Cargo target per worktree; with no subagents you run one suite at a time, so a single shared target per worktree is fine. No `timeout` binary on macOS; use `run_in_background` for long runs, and keep logs in the session scratchpad. Never print credentials (`OPENROUTER_API_KEY`, `OLLAMA_API_KEY`, `OPENCODE_GO_KEY` are in the environment). Conventional commits, `doc` not `docs`. Don't edit the main checkout's tracked files.

## Suggested skills

- `tdd` (approved seams: Collection/Application APIs, user-visible reader interactions, provider adapters, packaged-app workflows)
- `conventional-commits`, `conventional-branch`
- `code-review` after all tickets, from fixed base `5acda689f9716549190a8b175bec321ddb683713` (fix all findings yourself, no subagent)
- `domain-modeling` for ADR/glossary edits; `diagnosing-bugs` for failing gates
- `jgrep` for behavior searches, `rg` for identifiers
- `pr`/`pr-merge-verification`-style union verification before declaring integration green; repo-branch-worktree-cleanup at the end
- implement-spec (`$HOME/Documents/github/mattpocock-skills/skills/engineering/implement-spec/SKILL.md`) is a reference only; it is `disable-model-invocation`, read it rather than invoking it

## First actions

1. Read the earlier checkpoint, this file, `gh issue view` for any ticket you touch, `I/docs/agents/*`, `GLOSSARY.md`, `docs/adr/`.
2. `git status`/`diff` each preserved worktree above; confirm no stray processes (`ps`), and recheck ports (14335, 14338–14341).
3. Finish the Ollama adapter → Office role followup → photo → #10 retry, as above, then reverify live (`verify-office-granularity-live.py`, `verify-office-fidelity-live.py`, reconciliation, semantic regression) on a newly pinned driver, then close proven tickets, push the verified integration ref, and continue the DAG (#14/#16/#17, then #5/#6/#19/#20…).
4. Final code review, release/scale qualification, worktree cleanup remain required.
