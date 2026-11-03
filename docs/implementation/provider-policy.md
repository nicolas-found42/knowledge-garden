# Provider policy

The application uses `typesafe/jev-1.14` through OpenRouter's System One endpoint for semantic judgments. Jev is remote. Requests can contain acquired source text, extracted observations, and, for some reconciliation judgments, a prior source projection. They do not contain provider credentials in application logs. The transport rejects other model identifiers and fallback-routing fields before connecting.

Qwen visual interpretation runs locally through a managed Ollama process. The output is an unverified interpretation, not an authenticated observation. Local Whisper transcription is also machine-generated and its timestamps are estimates. No alternate cloud model is used as a fallback.

The current implementation still needs a complete review of evidence minimization. Speech-act judgments send the candidate sentences but not the full source or prior projection. Other judgments may send a bounded full projection. Do not describe the system as fully offline while Jev is enabled.
