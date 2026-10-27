# Background lifecycle

Closing the reader hides its existing window and leaves one collection engine and
maintenance worker running. The menu-bar **Open reader** action shows that same
window. **Quit Knowledge Garden** stops the worker and exits. Opening the app again
reconciles retained Markdown, watched sources, and pending work.

An installed app in `/Applications` or `~/Applications` registers a macOS user
LaunchAgent on its first run. Login starts the installed executable with
`--background`; the reader stays hidden. **Launch at login** in the menu bar
persists opt-out. Development builds and explicit collection overrides do not
register login startup. The registration is scoped to the application identifier.
Moving or deleting the installed app requires launching the replacement to update
the registration. An unreadable or future-version preference is preserved and its
error appears in the menu rather than silently overwriting it.

The worker waits on a condition variable between bounded maintenance turns. Its
status distinguishes active work, waiting/retry, stopping, and stopped, with the
last observed issue. Waiting is not a claim that all sources have been processed.
Remote semantic calls run outside the collection mutex. Existing local extraction
still holds that mutex; responsiveness during large imports belongs to #33.

Explicit Quit records a synced receipt before exit without waiting for extraction.
Quit also terminates and reaps app-owned helpers (including Whisper), rejecting
late helper launches. A cancelled media result leaves its durable window leased
until recovery rather than recording a transcription failure. The next exclusive
collection opener consumes the receipt after recovery, returning
interrupted semantic, media, and URL jobs to pending without exhausting their
crash budget. An unmarked process interruption keeps the existing bounded crash
recovery behavior. Stable source/version IDs, published knowledge, retained
originals, and media time ranges remain authoritative. A missing watched volume
continues to be recorded as disconnected, rather than confirmed deletion.

## Verification

`npm run check` covers formatting, lint, shell/workflow validation, frontend build,
frontend tests, Clippy, and native tests. `background_lifecycle.rs` exercises the
public application and actual worker: hidden-window state does not stop watching;
volume absence preserves sources; stop halts new work; reopening resumes without
duplicating identities. A regression repeats four controlled Quits with leased
semantic/audio jobs and then verifies an unmarked crash still consumes its budget.
The preference test preserves opt-out and unknown future data. An isolated
subprocess regression cancels a live helper during a public media operation,
verifies the helper was reaped and late launches rejected, then reopens the
collection and verifies the same window is pending with no crash-budget charge.

The native qualification build uses the separate identifier
`com.nicolasfound.knowledgegarden.lifecycleqa` and product name
`Knowledge Garden Lifecycle QA`, so its collection and login service cannot
replace the normal application's state. Current qualification host: Apple Silicon,
48 GB RAM, macOS 27.0.1. This is not the final 16 GB release qualification.

Actual installed-app close/reopen/Quit, login, sleep/wake, and crash outcomes must
be recorded before #28 is accepted. Unit tests and generic event-loop resume
notifications do not establish physical sleep or an actual login.

On the 48 GB qualification host, the installed QA build imported three watched
fixtures, survived a forced process crash and LaunchAgent bootstrap, and retained
all prior source identities while importing a fourth. Closing the reader left the
same process running; a fifth watched file appeared in the collection before the
reader reopened. After the app exited, a sixth file remained outside the collection
until the next launch, when it appeared in the reader. These observations do not
substitute for an actual login, physical sleep/wake, or menu-bar interaction.

The controlled-shutdown subprocess regressions also interrupt active helper,
replacement extraction, and final URL acquisition operations. They verify that
helpers are reaped and pending leases retain their retry budgets on reopening.
On this host, PDF and photo native regressions timed out waiting for Apple's Vision
neural-engine compilation service; the sampled process was waiting for that
service, and these checks remain unverified locally.
