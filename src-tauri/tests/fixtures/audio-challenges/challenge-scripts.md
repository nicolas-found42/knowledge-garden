# Held-out native acoustic challenge scripts

These synthetic files extend the original frozen spoken-acts fixture with separately attributable conditions. The installed macOS voices are speech renderers, not real people. Voice labels are rendering channels only.

## Accent rendering (`accent-en-in`)

Renderer: macOS `say`, voice Aman (`en_IN`).

> Question: should we close the library at five? I think we should leave the west entrance unlocked until the inspection is finished.

Source-only expectations: an explicit question and a tentative proposal; “I think” is retained. No action or actual lock state is established.

## Added background noise (`low-noise`)

Renderer: macOS `say`, voice Daniel (`en_GB`), mixed with a fixed low-amplitude white-noise bed.

> The blue binder was moved to the north shelf after lunch.

Source-only expectation: one declarative statement; no speaker identity or source-described event date is supplied. The file is a reproducible synthetic noise condition, not field noise.

## Simultaneous speech (`overlap-two-voices`)

Renderer A: macOS `say`, voice Aman (`en_IN`). Renderer B: macOS `say`, voice Daniel (`en_GB`). Both start together and are mixed without transcript prompts.

A: “The west entrance is open; the delivery arrived before noon.”

B: “Wait, the delivery has not arrived; close the side gate.”

Source-only expectations: two overlapping, conflicting recorded statements, each attributed only to its rendering channel. Do not resolve the actual delivery or gate state and do not infer a real person's identity.

## Truncated recording (`partial-final-sentence`)

Renderer: macOS `say`, voice Daniel (`en_GB`). The generated utterance is cropped at a fixed 2.5-second boundary to create a partial recording.

> The inspection report will arrive tomorrow morning and the gate should stay open until the city confirms the route.

Source-only expectation: the original recording ends during this scripted utterance. No complete final clause, continuation, or fact beyond the actual cropped waveform may be claimed. This is an intentional source truncation, not an ASR transcript.

## Limits

Labels are authored from this script and composition plan only, before generating or decoding the challenge WAVs. They are not human-gold labels, waveform inspection, or a claim about performance on real accented or noisy recordings. Exact speech recognition, timestamps, silence/noise/overlap interpretation, and partial coverage remain empirical outcomes to be recorded after freezing.
