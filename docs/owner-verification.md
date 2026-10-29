# Owner-run verification procedures

These procedures close the acceptance criteria that need hardware, third-party
software, provider spend, or physical timing that an agent cannot supply. Each
names the exact prerequisite, the commands, and the fields to record. Run them at
an approved time and paste the recorded values into the ticket comment.

Write evidence to the ticket, not into the repository, unless a procedure says
otherwise. Do not run any of these against the owner's personal collection.

Related decisions:
[ADR 0008](adr/0008-rebuild-derived-stores-through-a-resumable-stage.md),
[background lifecycle](background-lifecycle.md),
[offline processing](offline-processing.md),
[cold rebuild](cold-rebuild.md).

## Common preconditions

- Apple Silicon Mac with the release target's 16 GB of memory for anything labeled
  target-scale. The 48 GB host used for lifecycle work is not a substitute.
- A disposable collection directory outside any owner collection, created fresh
  for the run and deleted after the evidence is preserved.
- A build made from the commit under test, recorded by full SHA.
- `macOS` version, hardware identifier (`sysctl -n hw.model`), and the exact build
  command, recorded before the first action.

## #80 and #28: installed lifecycle across login and sleep

Prerequisite: physical logout or restart, and sleep/wake, at a time the owner
approves. No automation can supply these.

1. Build and install an isolated QA app:
   `npm run provision:assets`, then
   `npm run tauri -- build --bundles app -- --locked` with the identifier
   `com.nicolasfound.knowledgegarden.lifecycleqa` and product name
   `Knowledge Garden Lifecycle QA`.
2. Install to `~/Applications`, launch once with an explicit disposable collection,
   and confirm with `ps -o pid,command -p $(pgrep -f lifecycleqa)` that one process
   holds only that collection.
3. Configure a disposable watched directory and leave at least one queued item.
   Close the reader, add a source, and confirm the same process imports it. Reopen
   through the menu-bar item and confirm the same collection appears.
4. Enable launch at login. Log out and back in, or restart. Confirm the app starts
   without a terminal, opens the same collection, and resumes queued work without
   duplicating sources.
5. Sleep and wake the Mac. Add or change a watched source. Confirm processing
   resumes, retained knowledge stays readable, and a temporarily unavailable volume
   is recorded as disconnected rather than deleted.
6. Choose Quit from the menu-bar item. Confirm the process and its helpers stop
   (`pgrep -fl 'whisper-cli|media-helper|pdf-helper|photo-helper'` empty). Add a
   source while stopped, reopen, and confirm reconciliation imports it exactly once.

Record per step: timestamps, process IDs before and after, the count of sources in
the collection, and any failure. Then remove only the QA registration and the
disposable data. Leave #28 and #80 open until every step above has a recorded
outcome; the repository's automated lifecycle suite covers close, watching, stop,
and restart recovery, but not login startup or physical sleep.

## #31: real plugin-free Obsidian round trip

Prerequisite: a real ordinary Obsidian installation with no custom plugin.

1. Produce a disposable mixed-format collection through the application: a text
   report, a PDF, an Office document, a photo, an audio recording, and a video.
2. Open that collection as a vault in Obsidian. Read a page, inspect its properties,
   follow a link and a backlink, and open a PDF and a media original.
3. Edit one fact in Obsidian. In the application, update an unrelated fact of the
   same source. Confirm the edited fact keeps its value and the others refresh.
4. Rename a linked page inside Obsidian. Then move another page on disk with `mv`.
   Return to the application and confirm both pages keep their identity and the
   resolvable links are repaired.
5. Confirm saved views and trails are readable as ordinary Markdown, and that
   application search, graph, and media behavior is unchanged by the round trip.

Record: Obsidian version, macOS version, and the observed outcome of each of the
five steps. Do not publish private source material in the evidence.

## #32: keyboard-only and VoiceOver task runs

Prerequisite: VoiceOver, and a human usability observation. Automated DOM checks
and scripted focus assertions are not acceptances for the assistive-technology
criterion.

1. With a background import running, complete by keyboard only: import a file,
   search, build a relationship query, expand a group, inspect evidence and a gap,
   compare a passage, use a timeline, and edit and follow a saved view and a trail.
   Confirm focus never escapes into a trap and record the tab order used.
2. Repeat the same tasks with VoiceOver on. Record what is announced for names,
   roles, reading order, state changes, and the nonspatial route through the canvas
   content. Note every announcement that is missing or wrong.
3. Record human observations separately from the automated results and fix the
   failures before the ticket closes.

## #34: independent labels and authorized live provider runs

Prerequisite: provider budget authorization, and labels authored by a person
before the run.

1. Freeze a corpus and its expected labels for retrieval, support, qualifications,
   identity, update scope, and provenance. Commit the labels and the corpus hash
   before invoking any provider, so the model cannot grade its own output.
2. Run the deterministic replay first and record it as deterministic. Then run the
   authorized live provider pass, recording model and service versions.
3. Report misses and inappropriate results with denominators, plus representative
   failures. Record quota, capability, and unavailability behavior observed in the
   same run.

## #35: target-scale qualification

Prerequisite: the 16 GB Apple Silicon Mac, a corpus of at least 250,000 distinct
source files, and several hours of uninterrupted machine time.

1. Build the corpus into a disposable collection through the real watched-import
   path, and disclose how much of it is synthetic.
2. Measure separately: first import, incremental update, cold rebuild, normal
   startup, title and meaning search, optional online refinement, and graph
   interaction. Record latency, throughput where meaningful, peak memory, disk
   footprint, and hardware.
3. During an active import, exercise search, group expansion, media, page and
   evidence navigation, and access to distant items. Then interrupt a cold rebuild
   and restart it, confirming the same current knowledge and navigation outcomes.
4. Compare against the regression budgets recorded in the repository. Investigate
   every miss and fix before rerunning. Do not shrink the corpus to obtain a pass.

## #36: release assembly

Prerequisite: everything above, plus an ordinary installation on the target Mac.

1. Install the candidate as a normal Mac app and run the fresh-collection smoke
   journey: direct file import and drop, watched directory, a controlled URL,
   every required media class, evidence inspection, a V17 update, one external
   correction, offline search, save and reopen a view and a trail, menu-bar
   close/reopen, Quit, and cache rebuild.
2. Link the #31, #32, #34, and #35 evidence for the same candidate build.
3. Complete the requirement-to-evidence checklist across all 70 parent user
   stories. Unresolved failures keep the release ticket open.
