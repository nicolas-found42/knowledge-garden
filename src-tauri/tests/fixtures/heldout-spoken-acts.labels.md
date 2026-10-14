# Frozen held-out synthetic speech labels

Authored independently from decoder output on 2026-10-07, before any ASR or Jev call. Source: `heldout-spoken-acts.script.md`. The audio is rendered from the literal script with the macOS stock voices Samantha and Daniel, used only as synthetic voice A and voice B. No person identities are assigned. These labels and the source script are frozen before decoding; intervals are mechanical sample boundaries from the rendered utterance clips and concatenation plan.

Fixture: `heldout-spoken-acts.wav`; SHA-256 `12f9d38ec9c9915f3de96d04d6595ca90e126944e2dabde639abde6c555a6a7a`; mono, 22050 Hz, 16-bit PCM, 19.49746 seconds. The single 1.600-second digital-silence interval is sample-exact.

| Expected interval | Synthetic voice | Literal utterance and expected treatment |
| --- | --- | --- |
| 0.000–2.923 s | A | “Question: should we close the north gate before noon?” — explicit question, not a decision. |
| 2.923–6.318 s | B | “I think we should leave it open until the inspection is complete.” — tentative proposal, not an accomplished event. |
| 6.318–7.918 s | None | 1.600 seconds digital silence; should be disclosed as an interval without an utterance, not assigned words or a speaker. |
| 7.918–11.964 s | A | “Decision: postpone closing the north gate until after the inspection.” — records a stated decision; do not infer authorization or execution. |
| 11.964–14.811 s | B | “Reasoning: the west approach may still be icy.” — rationale that retains the qualification “may.” |
| 14.811–19.497 s | A | “I might have said the west approach was clear, but I'm not sure; it could still be icy.” — uncertain correction; preserve “might,” “not sure,” and “could.” Neither the earlier clear state nor current ice state is settled. |

Expected speaker handling: every segment has unknown speaker identity. The synthetic rendering voices are not names or known people. The expected source is synthetic TTS, not a human conversation or evidence that the described gate/road conditions are true.

Qualification limits: this fixture tests speech-act extraction, uncertainty retention, quiet coverage, estimated time-located passage retrieval, and unidentified-speaker handling on a short synthetic recording. It does not establish diarization quality, real-world speech recognition accuracy, or broad acoustic robustness.
