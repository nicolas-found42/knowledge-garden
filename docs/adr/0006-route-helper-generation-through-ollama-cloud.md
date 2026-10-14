---
status: accepted
---

# Route helper generation through Ollama Cloud DeepSeek V4.1 Flash

On 2026-10-03 the owner replaced the OpenCode Go route for generative helper work with "use ollama cloud with deepseek 4.1 flash". This overrides the route named in [ADR 0004](0004-use-jev-throughout-semantic-processing.md): the DeepSeek V4.1 Flash helper is now reached at `https://ollama.com/api` using the exact model name `deepseek-v4.1-flash`. The OpenCode Go DeepSeek route is not used; its credential was rejected with an inactive-subscription error, and the provider's terms are not settled for automatic parsing of model output. GPT-6 Luna is not part of this override and is neither enabled nor replaced.

Nothing else in ADR 0004 changes. Jev stays primary for every choice, score, and yes/no judgment. The Ollama model is only a helper where those primitives cannot express the work, such as describing image content, and its output is never accepted as grounded evidence without code-side validation against the source. Exact copying, calculations, identities, workflow control, and file mutations also stay in code.

The adapter is one shared Rust client used by every generative consumer, so those consumers cannot drift apart:

- The endpoint and model are fixed constants. No other model or provider is substituted automatically. If the response names a different model, the request fails as recoverable rather than being accepted.
- Credentials are looked up in Rust only, lazily at first request: the `OLLAMA_API_KEY` environment variable, then the macOS Keychain service `Knowledge Garden Provider Keys`. The Keychain lookup is bounded to 5 seconds. Credentials never reach the webview, logs, errors, or evidence.
- Each HTTP request is bounded to 45 seconds. Transient failures retry a small bounded number of times with backoff.
- Limit, quota, authorization, outage, offline, and timeout failures are recoverable, so the caller leaves semantic work pending and retries it later. They never block import or erase existing knowledge. Malformed requests are permanent failures.
- Requests are non-streaming with `think` configurable per call. Images travel as base64 in `message.images`.

Evidence for this route is bounded. Authorized synthetic text and image probes against the live service returned HTTP 200 with the exact model, and the image probe separated no image, a red square, and a blue circle. That is transport and basic image discrimination only. It does not establish provenance accuracy, arbitrary scene accuracy, or media types other than images, and it authorizes no uploads of private owner or research material. Consumers still need their own independent quality evaluation. [Specification #1](https://github.com/nicolas-found42/knowledge-garden/issues/1) keeps its agreed text; this ADR is the record of the route change.
