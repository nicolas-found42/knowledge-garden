# Independent source-only labels — frozen before ASR

I authored this file using only `challenge-scripts.md` and the planned audio composition. I did not generate, listen to, visualize, decode, or transcribe any challenge waveform. The expected source is synthetic TTS plus fixed editing, not a human recording. This is an agent-authored evaluation plan, not human signoff or acoustic ground truth.

| Fixture | Expected content / boundary | Required qualification | Prohibited inference |
|---|---|---|---|
| `accent-en-in.wav` | Question about closing a library at five; tentative proposal to leave west entrance unlocked until inspection ends. | Preserve the question and “I think” qualifier; “en_IN” names the installed TTS voice locale only. | Do not claim speech recognition is robust to real Indian-accented speech, identify a real speaker, or claim an action occurred. |
| `low-noise.wav` | The scripted binder-location sentence under an added low-amplitude generated noise bed. | Report any missing words/coverage as incomplete; distinguish recognizer output from the noise ground truth in this synthetic file. | Do not infer that the system detected/classified real-world noise, or promote the transcript to a verified move. |
| `overlap-two-voices.wav` | Two simultaneous scripts: A says the delivery arrived and west entrance is open; B contradicts delivery status and says close side gate. | Preserve conflict and anonymous rendering channels; disclose overlap/no speaker diarization limits. | Do not reconcile the statements, authenticate a voice, attribute words to a person, or infer gate/delivery state. |
| `partial-final-sentence.wav` | A fixed 2.5-second crop from the longer inspection/route sentence. | Coverage ends at the WAV boundary; disclose truncation and do not complete an unrecorded clause. | Do not treat an absent continuation as source omission or assert words after the crop. |

These labels are frozen before waveform generation and all ASR/Jev output. Later results may be added to a separate evaluation file without changing this expectation table.
