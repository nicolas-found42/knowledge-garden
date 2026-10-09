---
status: accepted
---

# Bundle a local English embedder and rebuildable HNSW index

Use the pinned BAAI/bge-base-en-v1.5 ONNX model with Rust ONNX Runtime, the local Hugging Face tokenizer, and a local USearch HNSW index for the first English meaning-search slice. Search itself performs no network requests. The Tauri app bundles its native Apple Silicon inference library, model, tokenizer, and notices as resources; `scripts/provision-meaning-assets.sh` verifies pinned SHA-256 digests and stages them before packaging. The application checks required files and digests at startup and reports unavailable or incomplete meaning search while keeping ordinary keyword search available.

The local index and its SQLite key lookup are derived from current page-search documents. Stable page identifiers are returned to the shared page-result flow. The index identity includes the model revision, tokenizer digest, pooling, and chunking so a mismatch triggers a rebuild from current Markdown-backed search records. Current knowledge and retained source assets remain authoritative.

This release slice supports English and must not be presented as a multilingual or collection-scale retrieval qualification. The installed app carries roughly 459 MiB of meaning-search resources on the validated Apple Silicon build. A small independently labeled synthetic set chooses a cosine cutoff of 0.45 for no-result behavior; that cutoff is fixture-calibrated, not a calibrated probability or general accuracy guarantee. Optional online ranking is not required for offline search.
