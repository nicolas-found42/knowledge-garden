---
status: accepted
---

# Bundle local audio transcription

Use the pinned whisper.cpp v1.9.3 source commit `371b5a7561823ab2bb32142d2751e35e7534727b`, built as a static Apple Silicon runner with embedded Metal, and bundle the `ggerganov/whisper.cpp` large-v3-turbo model from revision `98aa99a0a9db05ae2342309f5096248665f7cba3` (SHA-256 `1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69`). This route is selected because it correctly recovered the frozen uncertain count correction where the tested Apple SpeechAnalyzer, small.en, and medium.en routes did not. Provision and verify assets at build time; inference performs no model downloads or runtime compilation. Keep the exact audio original authoritative, process decoded audio in at most 30-second resumable segments, label timestamps as estimates, leave speakers unidentified, and qualify transcripts as unverified evidence. The synthetic fixture is a narrow capability check, not a general transcription-accuracy or diarization claim.
