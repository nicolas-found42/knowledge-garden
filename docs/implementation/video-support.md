# Saved video evidence

Issue #15 composes the existing retained-original, photo and local-audio paths for
saved MP4, MOV and M4V containers. AVFoundation supplies container and track facts,
decoded presentation times, and bounded JPEG frames. Speech uses the pinned local
Whisper processor. Neither channel authenticates a speaker, depicted identity or
capture date.

Processing advances through durable windows of at most 30 seconds. One frame is
sampled every three seconds; intervening images are unexamined. OCR regions and
uncertain classifier predictions are retained separately from transcript passages.
A failed track/window does not discard evidence from another track. After the
bounded speech retry budget, a damaged video interval is recorded as a coverage
gap and subsequent windows can continue. Restart retries the durable unfinished
window; stable version/window identities prevent duplicate passages or frames.

The readable Markdown includes sampled times, OCR, qualifications, track status,
and failed intervals. Semantic formation is bounded to 24 visual evidence lines;
additional observations remain readable on the source page. Jev represents frame
text and classifier output in their own visual channels. Frame dates and Vision
revision annotations cannot order source versions. Pasted projection text cannot
establish acquired video provenance.

Frame links request one preview from an explicitly retained source version and
asset. Previews are bounded to 2048 pixels and disclose requested and actual
decoded times, including the decoder's 250 ms tolerance. The reader retains one
on-demand image. Playback opens the retained original in QuickTime Player; precise
playback seeking is unavailable, so evidence links disclose the timestamp for
manual seeking. Missing previews retain that fallback. Source/version containment
checks also apply to preview requests.

## Verification and limits

`tests/video_workflow.rs` exercises public import, processing, semantic publication,
restart and original/preview access. Native fixtures cover silent MP4, truncated
MP4 and a MOV with irregular presentation timestamps. The latter can be rebuilt
with `tests/fixtures/video/generate-variable-rate.swift` into a new output path.
The illustrated river/title-card clip deliberately uses unrelated narration and a
silent section; its title-card date is not an authenticated capture date.

Deterministic ASR and Jev transports test origin discipline, publication, recovery,
and damaged-window continuation. The long-window test deliberately repeats the
short fixture's visual content through a recorded processor to isolate scheduling;
it is not a long-video native-decoding benchmark. Native frame/OCR and preview
checks run separately. Existing pinned Whisper acoustic tests remain in the audio
suite. None of these fixtures establishes general OCR, classification, transcription
accuracy, broad codec coverage, arbitrary-corpus scale or named-person recognition.

The reader frame link is also checked by keyboard in Vitest. A local browser check
uses the real Rust application driver for silent-video import, processing and
version-specific native frame decoding; it does not substitute mocked image data.
