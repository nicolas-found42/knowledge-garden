# Held-out synthetic speech script

Authored before waveform rendering, ASR, or Jev classification on 2026-10-07. This is a synthetic two-voice test fixture; the voice selections identify rendering inputs only and do not establish speaker identities.

1. Voice A: “Question: should we close the north gate before noon?”
2. Voice B: “I think we should leave it open until the inspection is complete.”
3. Quiet interval: 1.6 seconds of digital silence.
4. Voice A: “Decision: postpone closing the north gate until after the inspection.”
5. Voice B: “Reasoning: the west approach may still be icy.”
6. Voice A: “I might have said the west approach was clear, but I'm not sure; it could still be icy.”

The expected facts are not that the gate was actually left open, that the approach was icy, or that any person spoke. Expected speech acts are an explicit question, a tentative proposal, an explicitly labeled decision with its stated action, a rationale retaining “may,” and an uncertain correction retaining “might,” “not sure,” and “could.” Voice A and Voice B remain unidentified.
