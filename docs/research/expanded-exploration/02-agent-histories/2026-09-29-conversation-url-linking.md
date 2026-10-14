# Conversation URLs: occurrence capture without destination acquisition

Date: 2026-09-29. This experiment treats a URL found in a conversation as a reference. Its destination is **merely linked**: it has not been acquired and is not a failed retrieval. No destination content is inferred from a hostname, path or title.

## Capture and adjacency

The [capture runner](experiments/url_occurrences.py) recursively scans string fields in the 39 selected Codex/Claude/OMP JSONL histories and the selected extracted message fields for 13 Hermes sessions. It records every HTTP(S) candidate occurrence, including repeated references in messages, metadata, pasted skills and tool outputs. This is not an estimate of every URL in the whole available corpus, and repeated tool/metadata references are not independent human endorsements.

Each private occurrence records thread ID, record/line or message ID, field path, exact substring offsets, exact interpreted original string, full lexical match, conservative identity, exact hostname, acquisition status and `destination_fetched=false`. Lexical matches survive even where a Markdown closing delimiter is removed. Both normalized URL and hostname adjacency maps enumerate occurrence IDs; these are explicit links back to every occurrence rather than only frequency counts. Invalid candidates retain original evidence without a shared null identity edge.

Final capture results are:

| Measure | Count |
| --- | --- |
| Selected histories | 52, comprising 13 per application |
| HTTP(S) lexical candidate occurrences | **5,131** |
| Conservative parseable identities | **1,936** |
| Parseable exact-hostname buckets | **230** |
| Identities appearing in multiple selected threads | **24** |
| Exact-hostname buckets appearing in multiple selected threads | **25** |
| Invalid candidate occurrences retained without identity merging | **46** |
| Destination acquisition attempts | **0** |
| Occurrences marked merely linked | **5,131** |

The scanner favors source retention and conservative under-merging. It has not manually established that all 5,131 lexical candidates are valid intended links. Bare trailing punctuation, template syntax and serialized output can create candidate artifacts. Those remain inspectable; they must not silently become confirmed destination nodes. Other URI schemes, relative links and naked domain names are outside this HTTP(S) experiment.

An initial tokenizer stripped bare trailing punctuation and produced 1,901 valid identities. Audit found that punctuation may be legitimate URL path data, so the final tokenizer removes only a known Markdown `](...)` closing delimiter and preserves bare punctuation. This yields 1,936 identities and 46 invalid occurrences. These changed counts are a conversion diagnostic; no destination requests were made in either variant. Final public aggregates and the private occurrence/adjacency files reflect the conservative version.

## Identity rules and tested boundaries

URI scheme and host are case-insensitive; other components generally need scheme-specific knowledge before case or semantic normalization. [RFC 3986 section 6](https://www.rfc-editor.org/rfc/rfc3986#section-6) describes comparison and normalization. Python's parser provides components but does not validate arbitrary inputs as safe or well-formed; this experiment retains invalid candidates separately. [Python urllib.parse documentation](https://docs.python.org/3/library/urllib.parse.html#url-parsing-security).

The experiment lowercases scheme/host and removes explicit `:443` for HTTPS or `:80` for HTTP. It preserves scheme choice, exact path case, empty path versus slash, non-default and zero ports, userinfo, query ordering, repeated/meaningful parameters, tracking parameters, percent escapes, subdomains, fragments and empty `?`/`#` delimiters. It does not strip tracking automatically or resolve redirects. An exact hostname groups references to one host; it does not establish that different subdomains are the same entity. No public-suffix/registrable-domain inference was implemented.

Synthetic examples use reserved `.test` domains only:

| Pair | Policy |
| --- | --- |
| `HTTPS://EXAMPLE.test:443/a` / `https://example.test/a` | Shared conservative identity; preserve both originals and occurrences. |
| `https://example.test/a?item=1` / `https://example.test/a?item=2` | Separate identities. |
| `https://example.test/a?x=1&y=2` / reversed query ordering | Separate identities; unknown server semantics. |
| A URL with `utm_source=x` / same URL without it | Separate identities; tracking removal would be a separately labeled candidate relation. |
| `#chapter1` / `#chapter2` on the same base | Separate reference identities; fragments can encode meaningful evidence locations. |
| `https://sub.example.test/a` / `https://example.test/a` | Separate URL and exact-hostname identity. |
| `/path!` / `/path` | Separate identities; punctuation survives as possible path data. |

Seven [unit-test methods](experiments/test_url_occurrences.py) pass. They exercise two safe equivalences, fourteen protected distinctions plus punctuation preservation, three occurrence/offset checks, normalized-collision recovery, invalid-port retention and empty delimiter handling. A forced normalized collision retains both exact originals. The first implementation also accidentally combined invalid identities in a null bucket; the corrected version excludes null from adjacency, preserving failures independently. Neither normalization nor invalid parsing causes a URL to be fetched.

## Evidence for the no-fetch boundary

The URL module contains no HTTP client or destination acquisition function. It imports only URL parsing, reads local source records and writes local occurrences/adjacency. Tests use string literals and run without network access. The model runners POST only to the configured OpenRouter System One endpoint; conversation URL substrings are replaced with a capture marker before inference, and no model answer is connected to a browsing callback. Public TypeSafe/OpenRouter and URI/Python documentation were independently selected research sources, rather than destinations followed out of these histories.

`destination_fetch_attempts=0` is a code-path/accounting observation for this experiment, not a machine-wide network forensic claim. Captured thread text and tool evidence are source content available locally; linked destinations remain unknown. “Could not retrieve” is reserved for an actual attempted acquisition that failed, which did not happen to any conversation destination in this wave.

## Product implication to test

An on-demand thread import can create a reference neighborhood immediately: “this URL occurs in these messages” and “these threads reference this exact host.” That provides navigation without generating destination prose or making a large wiki page for every repeated link. A future separately authorized acquisition could attach a captured destination revision to the existing reference node; it should preserve all original conversation occurrences and their different fragment/query context.

Do not interpret domain coincidence as relevance or truth. A shared host may be a package registry, general documentation or boilerplate pasted into hundreds of sessions. The next experiment should judge the relationship of selected occurrences using their message context, while deterministic code remains responsible for exact capture and identity. No destination acquisition is required to test that relation.
