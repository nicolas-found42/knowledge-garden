# Frozen synthetic audio labels

Authored before any native transcription call on 2026-10-06. The 11.393-second WAV contains only system text-to-speech voices, not private recordings. The labels were created from the authored TTS inputs and timing plan; they are not decoder output and are never passed to ASR.

- 0.000–4.300 seconds, synthetic voice A: “The field report says twelve visits on May seventeenth, for about ten minutes.”
- 5.440–11.393 seconds, synthetic voice B: “I may have said twelve, but actually the count could be fifteen.”
- 4.300–5.440 seconds: quiet interval, 1.140 seconds.
- A and B describe fixture utterance origin only. The application must keep speaker identities unknown.

Qualification checks: recover the date, count, duration, and correction with source-located passages; preserve “may have” and “could be”; keep the inter-utterance gap visible; never turn fixture voice labels into known people. Transcript time boundaries are estimates and are checked within 250 ms. This fixture does not establish diarization quality, real-world recording accuracy, or acoustic intelligibility beyond the measured decoder result.

The `.m4a` companion is AAC transcoded from this same labeled WAV without changing the intended utterances or silence interval. It checks another representative container/codec path; it is not a separately authored ASR label set.
