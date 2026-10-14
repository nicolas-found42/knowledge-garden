# Linked Jev repositories for a local Markdown Knowledge Garden

Research date: 2026-09-29. Scope: the five requested collections and every one-hop GitHub repository link found in their Markdown documentation. Primary evidence is the linked repositories' own READMEs and, for the strongest matches, implementation files. These are research observations and design possibilities, not agreed product decisions.

## Coverage and evidence limits

The collection tarballs were inspected as data, without running their code. Repository links were extracted from all `.md` files, normalized from clone `.git` URLs, and deduplicated. The placeholder `owner/repo` was excluded. Badges, dependency references, and collection cross-links are retained when they resolve to a repository; nested repositories' links are not recursively traversed. Catalog claims such as “verified” are not treated as independent verification.

All linked repositories received an attempt to retrieve root `README.md`, then `readme.md`, using `raw.githubusercontent.com/<owner>/<repo>/HEAD/...`. A successful read is a retrieved document, not proof of a working tool. The entire retrieved corpus was mechanically screened for relevant words and project descriptions; selected matches were read in detail. The inventory distinguishes that screening from source inspection. “No strong match” is a prioritization result, not a claim that no useful code exists. “Unavailable README” does not establish that the repository itself is deleted: an unusual filename, permissions, renamed repository or absent README can cause the same result.

Detailed source reading covered Hafıza OS, Jev the Janitor, Cheshi, Hippo Memory, Jev Retrieval, Neo4jev and Nexibeo's cookbook. Additional relevant projects below were evaluated at README level only. Eleven archives were downloaded for static inspection, including two retrieval evaluation repositories. No package installs, tests, live APIs or benchmark reproduction were performed in this research branch. Source URLs use repository branches, so they can change after this snapshot; raw material is temporarily retained under `/tmp/kg-linked-research` on the research machine.

| Seed | Collection | Distinct one-hop repos in Markdown docs |
| --- | --- | ---: |
| A | [v-modal/awesome-jev-tools](https://github.com/v-modal/awesome-jev-tools) | 176 |
| B | [nexibeo/jev-cookbook](https://github.com/nexibeo/jev-cookbook) | 7 |
| C | [walidboulanouar/awesome-jev-use-cases](https://github.com/walidboulanouar/awesome-jev-use-cases) | 196 |
| D | [cobanov/awesome-jev](https://github.com/cobanov/awesome-jev) | 231 |
| E | [heyjunpenn/awesome-jev](https://github.com/heyjunpenn/awesome-jev) | 949 |

The union contains **1,077 distinct linked repositories**: **1,060 retrieved READMEs** and **17 unavailable conventional root READMEs**. Counts include the seed collections when linked by another seed. Their membership is shown in the inventory.

## Patterns with the strongest fit

### Local Markdown as the canonical source, with derived retrieval views

[Hafıza OS](https://github.com/fornhere/hafiza-os) describes a local Markdown vault shared by Claude Code, Codex and Antigravity, with optional Obsidian, Mem0 and Jev. It distinguishes an agent instruction, a captured session, reviewed memory and actual useful recall. Its [retrieval implementation](https://github.com/fornhere/hafiza-os/blob/main/araclar/jev_retrieval.py) begins with catalog eligibility and scope checks before semantic selection; its docstring expressly makes the optional semantic selector a non-writer. This suggests a useful Knowledge Garden boundary: raw imports, reviewed notes and rebuildable indexes have different roles. The repository also contains hand-written domain aliases and clause splitting: this is a concrete opportunity to test bounded semantic topic/facet selection instead of accumulating language-specific parsing rules. The surrounding provenance/scope gates remain code.

[Memsearch](https://github.com/zilliztech/memsearch) is another especially direct lead: its README advertises coding-agent capture, Markdown memory, background durable project/user note maintenance and optional Jev reranking. It deserves follow-up source investigation before selecting an ingestion adapter. [Total Agent Memory](https://github.com/vbcherepanov/total-agent-memory) instead treats SQLite as its central store, with multiple retrieval methods, validity intervals and explicit supersession. It supplies lifecycle ideas, but adopting its storage architecture would need a separate decision about the user's Markdown requirement. These two descriptions were checked against their respective READMEs only.

### Preserve the original statement, then judge its meaning

[Cheshi's passage construction](https://github.com/CheshiAI/Cheshi/blob/main/desktop/lib/chat-history-recall.mts) preserves thread/turn/item IDs and character offsets, excludes tool activity from authored-dialogue recall, and adds bounded neighboring context. Its [question definitions](https://github.com/CheshiAI/Cheshi/blob/main/desktop/lib/chat-history-recall-rules.mts) independently ask whether a passage contains an answer, records a related decision/correction/outcome, and is a direct record rather than a later retelling. Its [API evaluator](https://github.com/CheshiAI/Cheshi/blob/main/desktop/lib/chat-history-recall-model.mts) validates Noul probabilities and batches by request size.

The implication is unusually relevant to coding-agent ingestion: a chat includes proposals, plans, promises, decisions, implementation claims, corrections and tool evidence. A typed semantic judgment can distinguish these with the exact source attached, rather than a fragile list of keywords such as “decided” or “done”. The source adapter must still parse the host's actual JSON or event structure. Copying selected source spans, IDs and offsets remains deterministic. Their thresholds are explicitly provisional and are not transferable calibration for this project.

### Vault triage as editable questions and reproducible votes

[Jev the Janitor's taxonomy](https://github.com/kylehovance-ai/jev-the-janitor/blob/main/janitor/taxonomies/vault_memory.yaml) asks a bucket Choice, a persistence Score, and separate Nouls for decisions, actionability, possible duplication and sensitive content. The buckets include durable memory, project decision, code note, reference, routine log, ephemeral, junk and review. Its [policy](https://github.com/kylehovance-ai/jev-the-janitor/blob/main/janitor/policy.py) makes low confidence visible as review, uses separate deterministic redaction hits, and does not turn low persistence into deletion. The [README](https://github.com/kylehovance-ai/jev-the-janitor) describes dry runs, frontmatter stamps, resumable journals and cache keys including state and question wording.

This is a strong example of replacing semantic filing heuristics with editable, typed criteria while keeping the mutation policy in code. It also documents a failure: a missing “routine log” bucket caused ambiguous votes; adding the bucket improved its observed fit. Candidate and vocabulary coverage are architectural requirements. Its title-based near-duplicate question is expressly limited; it cannot establish that two differently worded bodies mean the same thing. Exact duplicates are detected locally by content hash. Its redaction is best effort, so a semantic private-content judgment after redaction cannot substitute for controlling which content may be sent.

### Entity matching needs both code facts and contextual judgment

[Nexibeo's duplicate recipe implementation](https://github.com/nexibeo/jev-cookbook/blob/main/recipes/06-dedupe-records/dedupe.mjs) compares organizations using a Score and an explicit review band, with exact normalized domain/phone comparisons supplied as code-checked facts. It distinguishes branches and subsidiaries from the same organization. Its [search recipe](https://github.com/nexibeo/jev-cookbook/tree/main/recipes/10-search-rerank), [multi-label recipe](https://github.com/nexibeo/jev-cookbook/tree/main/recipes/04-multi-label-tagging), [indexing recipe](https://github.com/nexibeo/jev-cookbook/tree/main/recipes/02-database-indexing), and [pick-from-candidates extraction recipe](https://github.com/nexibeo/jev-cookbook/tree/main/recipes/09-extract-by-picking) are directly applicable.

For Knowledge Garden, analogous judgments are whether two mentions refer to the same project/concept, whether a statement updates an earlier one, and which existing note or tag best fits an excerpt. Exact source IDs, hashes, timestamps, known aliases and path resolution belong in code. Semantic merging should initially propose a link or update with evidence: one mistaken merge can contaminate many future answers. The cookbook's README labels its hand-made datasets as demonstrations, not general benchmarks. Its provider-specific API examples must be checked against current TypeSafe/OpenRouter docs before reuse.

### Relevance judgments can simplify retrieval, but cannot restore omitted evidence

[Hippo's reranker implementation](https://github.com/kitfunso/hippo-memory/blob/master/src/rerankers/jev.ts) sends a redacted query plus bounded candidate memories in one request, asks one Noul per candidate, validates all answers, and falls back to a local reranker on failure. [Jev Retrieval's questions](https://github.com/romeromarcelo/jev-retrieval/blob/main/src/jev/questions.rs) distinguish prose that covers a concept from code that implements it, including dedicated supporting material rather than incidental mentions. These demonstrate semantic relevance as a focused judgment after deterministic candidate retrieval.

[Jev Recall](https://github.com/samdotmak/jev-recall) describes sending an entire small memory store, with one Noul per memory and concurrent sharding for larger stores. Its fictional evaluation emphasizes indirect relevance: the memory that constrains a task may share no vocabulary with it. [Jevsearch](https://github.com/kylemclaren/jevsearch) gives immediate lexical results, then judges the candidate pool with individual Nouls, a best-page Choice and an answerability Noul. Its own README states that the remaining misses were candidate recall failures. [Hotchpotch's reranker](https://github.com/hotchpotch/jev-reranker) distinguishes relative ordering from relevance filtering; [Shinpr's CLI](https://github.com/shinpr/jev-reranker) preserves IDs/paths and selects source sentences for evidence compression. These four were read at README level only.

For an initial wiki, comparing lexical retrieval alone, lexical plus Jev, and an all-notes or broader-candidate baseline would expose whether candidate coverage or judgment is the limiting step. Independent Noul judgments make per-item retention meaningful; a Choice is a competition over a fixed set and its probability shares should not be mistaken for globally comparable relevance. A relevance answer also does not certify a note's truth or freshness.

### Check source support before promoting a synthesized claim

[Paper Trellis Citation Verifier](https://github.com/MarissaFamularo/citation-verifier) describes a two-stage evidence check: a generation model proposes a verbatim supporting passage, code verifies that the passage exists in the fetched source, and Jev judges whether the sentence is supported, contradicted or unaddressed. Its README explicitly distinguishes an unavailable full source from an abstract that is silent, and calls unvalidated model verdicts a triage aid. For a wiki, the equivalent is verifying that every proposed claim cites an existing exact source span before asking a semantic entailment question. Missing surrounding context must be a reason to abstain or fetch more evidence. This project was read at README level; its citation parser and live service were not independently validated.

### Graph traversal can choose among observed links

[Neo4jev's navigator](https://github.com/jexp/neo4jev/blob/main/src/neo4jev/navigator.py) uses one Choice over actual outgoing relationships and a goal-reached Noul per hop. Code enforces depth/call/beam budgets and assembles paths; candidate options have a hard bound. This transfers to following Markdown links toward a user's question without generating arbitrary graph queries. It is an exploration possibility after links and useful retrieval exist, not evidence that a graph database is required. [Laya GraphRAG](https://github.com/bodepudimuneendra-netizen/laya-jev-GraphRAG) is a related README-level lead, with less direct relevance than Markdown-vault and chat-recall implementations.

[Byeori](https://github.com/joonan-lab/byeori) describes separate original documents, evidence notes, concept pages, overviews and question pages, plus derived search indexes and optional Jev nominations of reusable knowledge for human review. Its AWS/lab architecture is outside this personal local scope, but the distinction between source evidence and revisable concept synthesis is useful. Its README was inspected; the live cloud workflow was not exercised.

## Findings that should constrain the brainstorm

1. Treat automatic session capture and trustworthy knowledge promotion as separate outcomes. A complete imported transcript is valuable evidence even when no durable note is created.
2. Preserve source references through every judgment. Semantic status (proposal, decision, correction, outcome) is a candidate for Jev; source identity and exact copying are code responsibilities.
3. Test judgments on false friends: a recommendation versus an accepted choice, an agent completion claim versus verified tool evidence, a reversed decision, quoted old history, and two projects with the same technology/name.
4. A predefined Choice cannot pick an omitted note/tag/entity. Include no-match/review options and measure candidate coverage separately from selected-option accuracy.
5. Use derived indexes and graph views so experiments can change without rewriting the originals. Record model version, source fingerprint, question version and raw probabilities to make re-evaluation interpretable.
6. Evaluate update and contradiction behavior before adding automatic merge, forgetting or consolidation. [Hippo's README](https://github.com/kitfunso/hippo-memory) explicitly reports a mechanism audit where decay had no measurable effect and sleep lowered retrieval; it also separates retrieval evidence from evidence that agents do better work. Those are project-specific author reports, not a universal verdict, but they directly undermine assuming that more “memory lifecycle” complexity is automatically beneficial.
7. Compare retrieval methods on the same source pool and labeled queries. [Jev search rerank eval](https://github.com/zhuyansen/jev-search-rerank-eval) and [Jev rerank bench](https://github.com/anessbelbati/jev-rerank-bench) are evaluation leads; their published headline numbers were not reproduced here. Coding-chat questions must include evidence correctness and temporal validity, not only rank position.

## Prototype reuse versus extracting patterns

The best initial reuse candidates are Nexibeo's small labeled fixtures/question builders and a contained retrieval or triage module. A whole memory product also brings its storage assumptions, hooks, maintenance policies and provider configuration. Extracting a pattern first makes it possible to evaluate whether it solves this user's missing-context and provenance problems before adopting those wider behaviors. This is a research recommendation, not an accepted architecture decision.

The following licenses were checked by opening the license files in downloaded repository snapshots. This is a file inspection, not a complete dependency-license review.

| Project | License file verified | Reuse implication for an initial experiment |
| --- | --- | --- |
| Hafıza OS | [MIT LICENSE](https://github.com/fornhere/hafiza-os/blob/main/LICENSE) | Study source validation and adapters; vault installation/runtime is much wider than an isolated judgment experiment. |
| Jev the Janitor | [MIT LICENSE](https://github.com/kylehovance-ai/jev-the-janitor/blob/main/LICENSE) | Taxonomy, policy and journal patterns fit; importing its entire filing/quarantine workflow would add mutation choices not yet settled. |
| Cheshi | [MIT LICENSE](https://github.com/CheshiAI/Cheshi/blob/main/LICENSE) | Extract passage/provenance/question design; a macOS agent workspace is a much broader product. |
| Hippo Memory | [MIT LICENSE](https://github.com/kitfunso/hippo-memory/blob/master/LICENSE) | Contained reranking is relevant; SQLite primary storage and lifecycle hooks deserve separate evaluation. |
| Jev Retrieval | [Apache-2.0 LICENSE.md](https://github.com/romeromarcelo/jev-retrieval/blob/main/LICENSE.md) | A plausible search prototype over Markdown; code-oriented defaults and provider transport require checking on chat questions. |
| Neo4jev | [MIT LICENSE](https://github.com/jexp/neo4jev/blob/main/LICENSE) | Transfer bounded observed-edge navigation; adding Neo4j would be a separate storage decision. |
| Nexibeo Cookbook | [MIT LICENSE](https://github.com/nexibeo/jev-cookbook/blob/main/LICENSE) | Small recipes and labeled fixtures provide the most contained experiment starting point; revalidate API/version details against current official docs. |

## Complete deduplicated one-hop inventory

The seed letters refer to the coverage table. **Source inspected** means the detailed evidence discussed above; **relevant README** means a clearly applicable README read; **screened lead** means a title/keyword match worth a later read; **screened; no strong match** means no high-priority match identified in this pass; **unavailable README** means the conventional raw paths failed. Every successful repository link below is its primary evidence location. This table is exhaustive for extracted GitHub repo links in the five seed Markdown snapshots, not for all projects in the wider Jev ecosystem.

| Linked repository / primary source | Seed(s) | Coverage / relevance |
| --- | --- | --- |
| [0x7067/claude-jev](https://github.com/0x7067/claude-jev) | E | screened; no strong match for this scope |
| [0x963D/last-exit](https://github.com/0x963D/last-exit) | E | screened; no strong match for this scope |
| [0xArx/jevegis](https://github.com/0xArx/jevegis) | E | screened; no strong match for this scope |
| [0xBakeer/arbiter](https://github.com/0xBakeer/arbiter) | E | screened; no strong match for this scope |
| [0xlau/jev-rps](https://github.com/0xlau/jev-rps) | E | screened; no strong match for this scope |
| [0xNatoshi/jev-codex-router](https://github.com/0xNatoshi/jev-codex-router) | C, D, E | screened; no strong match for this scope |
| [0xPlaygrounds/rig](https://github.com/0xPlaygrounds/rig) | D | screened; no strong match for this scope |
| [0xtrou/rubikjev](https://github.com/0xtrou/rubikjev) | E | screened; no strong match for this scope |
| [1612535983/deepreseach-learing](https://github.com/1612535983/deepreseach-learing) | E | screened; no strong match for this scope |
| [1Panel-dev/laya-server](https://github.com/1Panel-dev/laya-server) | C, E | screened; no strong match for this scope |
| [24601/Augustus](https://github.com/24601/Augustus) | A, D, E | screened; no strong match for this scope |
| [3clyp50/a0-typesafe-ai](https://github.com/3clyp50/a0-typesafe-ai) | E | screened; no strong match for this scope |
| [4esv/jev-eval](https://github.com/4esv/jev-eval) | D | screened; no strong match for this scope |
| [4rays/profanity-checker](https://github.com/4rays/profanity-checker) | A | screened; no strong match for this scope |
| [6Mikao9/jev-agent-design-with-topk-logits-choices](https://github.com/6Mikao9/jev-agent-design-with-topk-logits-choices) | E | screened; no strong match for this scope |
| [9pings/notjev](https://github.com/9pings/notjev) | E | screened; no strong match for this scope |
| [a-Fig/jev-score](https://github.com/a-Fig/jev-score) | E | screened; no strong match for this scope |
| [aabolfazl/typesafe-local](https://github.com/aabolfazl/typesafe-local) | E | screened; no strong match for this scope |
| [aaddrick/building-with-typesafe-jev](https://github.com/aaddrick/building-with-typesafe-jev) | E | screened; no strong match for this scope |
| [aaronshaf/opencode-jev-orchestrator](https://github.com/aaronshaf/opencode-jev-orchestrator) | D | screened; no strong match for this scope |
| [AbdelStark/awesome-typesafe](https://github.com/AbdelStark/awesome-typesafe) | B, D, E | collection / discovery source; README screened |
| [AbdelStark/awesome-typesafe-jev](https://github.com/AbdelStark/awesome-typesafe-jev) | C, E | collection / discovery source; README screened |
| [AbdelStark/bicameral](https://github.com/AbdelStark/bicameral) | E | screened; no strong match for this scope |
| [AbdelStark/heist-one](https://github.com/AbdelStark/heist-one) | D, E | screened; no strong match for this scope |
| [AbdelStark/jev-benchmarks](https://github.com/AbdelStark/jev-benchmarks) | D, E | screened; no strong match for this scope |
| [AbdelStark/s1-rs](https://github.com/AbdelStark/s1-rs) | E | screened; no strong match for this scope |
| [AbdelStark/typesafe-rs](https://github.com/AbdelStark/typesafe-rs) | E | screened; no strong match for this scope |
| [abe75ch/tiltmeter](https://github.com/abe75ch/tiltmeter) | E | screened; no strong match for this scope |
| [abeldzan/jev-rs](https://github.com/abeldzan/jev-rs) | C | screened; no strong match for this scope |
| [abhishek085/open-spark-jev](https://github.com/abhishek085/open-spark-jev) | E | screened; no strong match for this scope |
| [abhishekashokvkumar/jev-mcp-dispatcher](https://github.com/abhishekashokvkumar/jev-mcp-dispatcher) | C, E | screened; no strong match for this scope |
| [abhixhek/jevcal](https://github.com/abhixhek/jevcal) | A, C, D, E | screened; no strong match for this scope |
| [ably-labs/jev-pong](https://github.com/ably-labs/jev-pong) | A, E | screened; no strong match for this scope |
| [AboveColin/HA-Jev](https://github.com/AboveColin/HA-Jev) | A, C, D, E | screened; no strong match for this scope |
| [AboveColin/jevclient](https://github.com/AboveColin/jevclient) | D, E | screened; no strong match for this scope |
| [achimala/jev-paint](https://github.com/achimala/jev-paint) | D, E | screened; no strong match for this scope |
| [achimala/jevinci](https://github.com/achimala/jevinci) | A | screened; no strong match for this scope |
| [adarshmishra07/jcm-router](https://github.com/adarshmishra07/jcm-router) | A, C, E | screened; no strong match for this scope |
| [adhamelhayek-lab/jev-connector](https://github.com/adhamelhayek-lab/jev-connector) | E | unavailable README |
| [adhyaay-karnwal/jev-chat](https://github.com/adhyaay-karnwal/jev-chat) | E | screened; no strong match for this scope |
| [adtyavrdhn/pydantic-jev-examples](https://github.com/adtyavrdhn/pydantic-jev-examples) | E | screened; no strong match for this scope |
| [affirmitv/ghosthands](https://github.com/affirmitv/ghosthands) | E | screened; no strong match for this scope |
| [afshinm/laya-mps](https://github.com/afshinm/laya-mps) | E | screened; no strong match for this scope |
| [agent-chaperone/agent-chaperone](https://github.com/agent-chaperone/agent-chaperone) | E | screened; no strong match for this scope |
| [agent-labs-dev/fastbrowse](https://github.com/agent-labs-dev/fastbrowse) | A, E | screened; no strong match for this scope |
| [agentconnect-md/agentconnect](https://github.com/agentconnect-md/agentconnect) | E | screened; no strong match for this scope |
| [agentgateway/agentgateway](https://github.com/agentgateway/agentgateway) | E | screened; no strong match for this scope |
| [AgriciDaniel/jev-seo](https://github.com/AgriciDaniel/jev-seo) | E | screened; no strong match for this scope |
| [aidil2105/jev-browser-pilot](https://github.com/aidil2105/jev-browser-pilot) | E | screened; no strong match for this scope |
| [AiPersonacademy/jev-resume-disqualifier](https://github.com/AiPersonacademy/jev-resume-disqualifier) | E | screened; no strong match for this scope |
| [ajensenwaud/hermes-jev-plugin](https://github.com/ajensenwaud/hermes-jev-plugin) | C, E | screened; no strong match for this scope |
| [akash-kamat/system-one-gemma](https://github.com/akash-kamat/system-one-gemma) | E | screened; no strong match for this scope |
| [AkashPriyadarshii/jev-curate](https://github.com/AkashPriyadarshii/jev-curate) | A, C, D, E | screened; no strong match for this scope |
| [AkashPriyadarshii/jev-git](https://github.com/AkashPriyadarshii/jev-git) | A, C, E | screened; no strong match for this scope |
| [AkashPriyadarshii/jev-scout](https://github.com/AkashPriyadarshii/jev-scout) | A, C, E | screened; no strong match for this scope |
| [AkashPriyadarshii/jev-seo](https://github.com/AkashPriyadarshii/jev-seo) | A, C, E | screened; no strong match for this scope |
| [AkashPriyadarshii/jev-superpowers](https://github.com/AkashPriyadarshii/jev-superpowers) | A, C, E | screened; no strong match for this scope |
| [akivet/Mica-v0.1-4B](https://github.com/akivet/Mica-v0.1-4B) | E | screened; no strong match for this scope |
| [AlbionaHoti/refgarden](https://github.com/AlbionaHoti/refgarden) | E | screened; no strong match for this scope |
| [Alex314618-create/JevRev](https://github.com/Alex314618-create/JevRev) | E | screened; no strong match for this scope |
| [alexj11324/open-jev-approvals](https://github.com/alexj11324/open-jev-approvals) | E | screened; no strong match for this scope |
| [alexshpunt/pi-agent-foreman](https://github.com/alexshpunt/pi-agent-foreman) | E | screened; no strong match for this scope |
| [AlgoVaultLabs/algovault-integrations](https://github.com/AlgoVaultLabs/algovault-integrations) | D | screened; no strong match for this scope |
| [aliaihub/awesome-jev-usecases](https://github.com/aliaihub/awesome-jev-usecases) | E | collection / discovery source; README screened |
| [AliceRoselia/Typesafe_chess_eval](https://github.com/AliceRoselia/Typesafe_chess_eval) | E | screened; no strong match for this scope |
| [allebee/jevgrep](https://github.com/allebee/jevgrep) | D | screened; no strong match for this scope |
| [allebee/jevk5](https://github.com/allebee/jevk5) | C, E | screened; no strong match for this scope |
| [allebee/pytest-jev](https://github.com/allebee/pytest-jev) | D, E | screened; no strong match for this scope |
| [alperiox/audio-jevlike](https://github.com/alperiox/audio-jevlike) | D | screened; no strong match for this scope |
| [alterhq/typesafe-sdk-swift](https://github.com/alterhq/typesafe-sdk-swift) | A, E | screened; no strong match for this scope |
| [altryne/jevify](https://github.com/altryne/jevify) | C, E | screened; no strong match for this scope |
| [alvarobartt/sys1](https://github.com/alvarobartt/sys1) | E | screened; no strong match for this scope |
| [Amal-David/awesome-jev](https://github.com/Amal-David/awesome-jev) | E | collection / discovery source; README screened |
| [Amrit-Nigam/jev-royal](https://github.com/Amrit-Nigam/jev-royal) | E | screened; no strong match for this scope |
| [amycardoso/jev-palette](https://github.com/amycardoso/jev-palette) | D | screened; no strong match for this scope |
| [anandi1989/awesome-jev-usecases](https://github.com/anandi1989/awesome-jev-usecases) | E | collection / discovery source; README screened |
| [andrelandgraf/safer-with-jev](https://github.com/andrelandgraf/safer-with-jev) | A, C, E | screened; no strong match for this scope |
| [AndrewPrifer/jimothy](https://github.com/AndrewPrifer/jimothy) | E | screened; no strong match for this scope |
| [andududu/jeview](https://github.com/andududu/jeview) | E | screened; no strong match for this scope |
| [anessbelbati/jev-rerank-bench](https://github.com/anessbelbati/jev-rerank-bench) | D, E | relevant README; see discussion |
| [Anil-matcha/awesome-jev-by-typesafe](https://github.com/Anil-matcha/awesome-jev-by-typesafe) | A, D, E | collection / discovery source; README screened |
| [anilsenay/jev](https://github.com/anilsenay/jev) | E | screened; no strong match for this scope |
| [aniruddh-krovvidi/switchboard](https://github.com/aniruddh-krovvidi/switchboard) | E | screened; no strong match for this scope |
| [anishfn/shapeshift](https://github.com/anishfn/shapeshift) | C, E | screened; no strong match for this scope |
| [anisselbd/jev-phishing-bench](https://github.com/anisselbd/jev-phishing-bench) | E | screened; no strong match for this scope |
| [ankit-aglawe/tinyjev](https://github.com/ankit-aglawe/tinyjev) | E | screened; no strong match for this scope |
| [AnotiaWang/awesome-jev](https://github.com/AnotiaWang/awesome-jev) | C, E | collection / discovery source; README screened |
| [anpicasso/hermes-jev-approvals](https://github.com/anpicasso/hermes-jev-approvals) | A, C, E | screened; no strong match for this scope |
| [AnshChoudhary/typesafe-ai-firewall](https://github.com/AnshChoudhary/typesafe-ai-firewall) | E | screened; no strong match for this scope |
| [AntonioCoppe/jev-harness](https://github.com/AntonioCoppe/jev-harness) | E | screened; no strong match for this scope |
| [aowang-ai/jev-trade](https://github.com/aowang-ai/jev-trade) | A, D, E | screened; no strong match for this scope |
| [apolinario/decision-index](https://github.com/apolinario/decision-index) | E | screened; no strong match for this scope |
| [AppitStudio/awesome-jev](https://github.com/AppitStudio/awesome-jev) | E | collection / discovery source; README screened |
| [APUS-AI-Lab/fast-browser-use](https://github.com/APUS-AI-Lab/fast-browser-use) | E | screened; no strong match for this scope |
| [ARCJ137442/jev-life](https://github.com/ARCJ137442/jev-life) | E | screened; no strong match for this scope |
| [ARCJ137442/jev-switch](https://github.com/ARCJ137442/jev-switch) | E | screened; no strong match for this scope |
| [ariel-frischer/jevkit](https://github.com/ariel-frischer/jevkit) | A | screened; no strong match for this scope |
| [arielweinberger/jev-autopilot](https://github.com/arielweinberger/jev-autopilot) | E | screened; no strong match for this scope |
| [arimanyus/warrenduffer](https://github.com/arimanyus/warrenduffer) | E | screened; no strong match for this scope |
| [arunav25/jev-mcp](https://github.com/arunav25/jev-mcp) | C, E | screened; no strong match for this scope |
| [asfarsadewa/human-compiler](https://github.com/asfarsadewa/human-compiler) | E | screened; no strong match for this scope |
| [ashaazami/river-run-typesafe](https://github.com/ashaazami/river-run-typesafe) | E | screened; no strong match for this scope |
| [ashishakkumar/Jev-Checkpoint](https://github.com/ashishakkumar/Jev-Checkpoint) | E | unavailable README |
| [AshutoshVJTI/progressgate](https://github.com/AshutoshVJTI/progressgate) | E | screened; no strong match for this scope |
| [Avinash-jetwani/jevmem](https://github.com/Avinash-jetwani/jevmem) | E | screened; no strong match for this scope |
| [awlevin/typesafe-computer-use](https://github.com/awlevin/typesafe-computer-use) | C, D, E | screened; no strong match for this scope |
| [awun8191/jev-resume-analyzer](https://github.com/awun8191/jev-resume-analyzer) | E | screened; no strong match for this scope |
| [ax-llm/ax](https://github.com/ax-llm/ax) | D | screened; no strong match for this scope |
| [Ayush0054/metis](https://github.com/Ayush0054/metis) | E | screened; no strong match for this scope |
| [Baba88611/detroit-ai-player](https://github.com/Baba88611/detroit-ai-player) | E | screened; no strong match for this scope |
| [baibizhe/jev-decision-benchmarks](https://github.com/baibizhe/jev-decision-benchmarks) | D | screened; no strong match for this scope |
| [bartlomein/oko](https://github.com/bartlomein/oko) | D | screened; no strong match for this scope |
| [bastani-inc/atomic](https://github.com/bastani-inc/atomic) | A | screened; no strong match for this scope |
| [BeatAPI/awesome-jev](https://github.com/BeatAPI/awesome-jev) | E | collection / discovery source; README screened |
| [BeLazy167/typesafe-mod](https://github.com/BeLazy167/typesafe-mod) | E | screened; no strong match for this scope |
| [BerriAI/liteagents](https://github.com/BerriAI/liteagents) | E | screened; no strong match for this scope |
| [BerriAI/litellm](https://github.com/BerriAI/litellm) | D, E | screened; no strong match for this scope |
| [bestagentkits/jev-skillful](https://github.com/bestagentkits/jev-skillful) | E | screened lead; requires follow-up |
| [bestagentkits/typesafe-demo-mcp](https://github.com/bestagentkits/typesafe-demo-mcp) | E | unavailable README |
| [Bewinxed/jevgpt](https://github.com/Bewinxed/jevgpt) | E | screened; no strong match for this scope |
| [bhaiG-de/jev-design-test](https://github.com/bhaiG-de/jev-design-test) | E | screened; no strong match for this scope |
| [BillionsBobby/JevRouter](https://github.com/BillionsBobby/JevRouter) | C, D, E | screened; no strong match for this scope |
| [binnash/typesafe-sdk](https://github.com/binnash/typesafe-sdk) | E | screened; no strong match for this scope |
| [bitnovus/jev-spam-eval](https://github.com/bitnovus/jev-spam-eval) | A, E | screened; no strong match for this scope |
| [bladedevoff/stuntd](https://github.com/bladedevoff/stuntd) | A, D, E | screened; no strong match for this scope |
| [blakestone-x/jev-mcp](https://github.com/blakestone-x/jev-mcp) | A, C, D, E | screened; no strong match for this scope |
| [blas0/jev-shadcn-lint-eval](https://github.com/blas0/jev-shadcn-lint-eval) | E | screened; no strong match for this scope |
| [blowxian/jev-fanout-bench](https://github.com/blowxian/jev-fanout-bench) | D | screened; no strong match for this scope |
| [bnsd55/jevmlx](https://github.com/bnsd55/jevmlx) | D, E | screened; no strong match for this scope |
| [bodepudimuneendra-netizen/laya-jev-GraphRAG](https://github.com/bodepudimuneendra-netizen/laya-jev-GraphRAG) | E | relevant README; see discussion |
| [Bodila51/grok-bot-jev](https://github.com/Bodila51/grok-bot-jev) | C, E | screened; no strong match for this scope |
| [bohutang/sift](https://github.com/bohutang/sift) | A, C, E | screened; no strong match for this scope |
| [boozedog/pi-codemode](https://github.com/boozedog/pi-codemode) | E | screened; no strong match for this scope |
| [BorisLeMeec/jev](https://github.com/BorisLeMeec/jev) | E | screened; no strong match for this scope |
| [bottlebrushes/jev-orb](https://github.com/bottlebrushes/jev-orb) | E | screened; no strong match for this scope |
| [BoundaryML/baml](https://github.com/BoundaryML/baml) | D | screened; no strong match for this scope |
| [bradAGI/ruling](https://github.com/bradAGI/ruling) | A, D | screened; no strong match for this scope |
| [brainstormity/Jev-Moderation-Bot](https://github.com/brainstormity/Jev-Moderation-Bot) | A, E | screened; no strong match for this scope |
| [brainstormity/Jev-X-Sentiment-Analysis](https://github.com/brainstormity/Jev-X-Sentiment-Analysis) | E | screened; no strong match for this scope |
| [Brainwires/jevwire](https://github.com/Brainwires/jevwire) | D, E | screened; no strong match for this scope |
| [brandonbryant12/transcript-scorecard](https://github.com/brandonbryant12/transcript-scorecard) | E | screened; no strong match for this scope |
| [brnyxx/jev-ra](https://github.com/brnyxx/jev-ra) | E | screened; no strong match for this scope |
| [browser-use/jev-ultrafast](https://github.com/browser-use/jev-ultrafast) | A, B, C, D, E | screened; no strong match for this scope |
| [brudarko/jev-mac-voice](https://github.com/brudarko/jev-mac-voice) | E | unavailable README |
| [buberlo/dsh-jev](https://github.com/buberlo/dsh-jev) | E | screened; no strong match for this scope |
| [buberlo/jev-trader](https://github.com/buberlo/jev-trader) | C | screened; no strong match for this scope |
| [Bud-ro/jev-demos](https://github.com/Bud-ro/jev-demos) | E | screened; no strong match for this scope |
| [BunsDev/clarity-judge](https://github.com/BunsDev/clarity-judge) | E | screened; no strong match for this scope |
| [BunsDev/river-oaks](https://github.com/BunsDev/river-oaks) | E | unavailable README |
| [BunsDev/typesafe-ai-playground](https://github.com/BunsDev/typesafe-ai-playground) | E | screened; no strong match for this scope |
| [BunsDev/typesafe-ui](https://github.com/BunsDev/typesafe-ui) | E | screened; no strong match for this scope |
| [burnigtm/jev-mcp](https://github.com/burnigtm/jev-mcp) | C, E | screened; no strong match for this scope |
| [Butochnikov/laravel-typesafe-jev](https://github.com/Butochnikov/laravel-typesafe-jev) | A, D, E | screened; no strong match for this scope |
| [Butochnikov/typesafe-sdk-php](https://github.com/Butochnikov/typesafe-sdk-php) | E | screened; no strong match for this scope |
| [byalex33/changelog.earth](https://github.com/byalex33/changelog.earth) | E | screened; no strong match for this scope |
| [BYK/jev-mcp](https://github.com/BYK/jev-mcp) | C, D, E | screened; no strong match for this scope |
| [bytelabs-oss/clash-jev](https://github.com/bytelabs-oss/clash-jev) | C, E | screened; no strong match for this scope |
| [Cab14bacc/jev-sheets](https://github.com/Cab14bacc/jev-sheets) | D | screened; no strong match for this scope |
| [caiovicentino/eikos](https://github.com/caiovicentino/eikos) | E | screened; no strong match for this scope |
| [caiovicentino/eikos-arena](https://github.com/caiovicentino/eikos-arena) | E | screened; no strong match for this scope |
| [caiovicentino/jev-shield](https://github.com/caiovicentino/jev-shield) | E | screened; no strong match for this scope |
| [can1357/jegrep](https://github.com/can1357/jegrep) | D, E | screened; no strong match for this scope |
| [can1357/oh-my-pi](https://github.com/can1357/oh-my-pi) | E | screened; no strong match for this scope |
| [CaptainCore/captaincore](https://github.com/CaptainCore/captaincore) | E | screened; no strong match for this scope |
| [cardotrejos/jev-ad-preflight](https://github.com/cardotrejos/jev-ad-preflight) | E | screened; no strong match for this scope |
| [cardotrejos/jev-should-i-apply](https://github.com/cardotrejos/jev-should-i-apply) | E | screened; no strong match for this scope |
| [cardotrejos/jev-user-jury](https://github.com/cardotrejos/jev-user-jury) | E | screened; no strong match for this scope |
| [carldaws/hunch](https://github.com/carldaws/hunch) | D, E | screened; no strong match for this scope |
| [carldaws/hunch-ts](https://github.com/carldaws/hunch-ts) | D | screened; no strong match for this scope |
| [carlsonchik/judging-with-typesafe](https://github.com/carlsonchik/judging-with-typesafe) | E | screened; no strong match for this scope |
| [caudena/beam_weaver](https://github.com/caudena/beam_weaver) | E | screened; no strong match for this scope |
| [cbetz/extremely-specific-council](https://github.com/cbetz/extremely-specific-council) | E | screened; no strong match for this scope |
| [cbruyndoncx/AskJev-MCP](https://github.com/cbruyndoncx/AskJev-MCP) | E | screened; no strong match for this scope |
| [cclank/jevclip](https://github.com/cclank/jevclip) | E | screened; no strong match for this scope |
| [CelestoAI/celesto](https://github.com/CelestoAI/celesto) | E | screened; no strong match for this scope |
| [cephalization/jev-triage](https://github.com/cephalization/jev-triage) | A, E | screened; no strong match for this scope |
| [cequence-io/openai-scala-client](https://github.com/cequence-io/openai-scala-client) | E | screened; no strong match for this scope |
| [chaitin/Decis](https://github.com/chaitin/Decis) | E | screened; no strong match for this scope |
| [Chandler-Sun/chat2jev](https://github.com/Chandler-Sun/chat2jev) | E | screened; no strong match for this scope |
| [Charlyhno-eng/jev-document-classification](https://github.com/Charlyhno-eng/jev-document-classification) | E | screened; no strong match for this scope |
| [CheshiAI/Cheshi](https://github.com/CheshiAI/Cheshi) | E | source inspected; directly relevant |
| [ChetasLua/jevmeter](https://github.com/ChetasLua/jevmeter) | D, E | screened; no strong match for this scope |
| [chez-shanpu/typesafeai-go](https://github.com/chez-shanpu/typesafeai-go) | C, E | screened; no strong match for this scope |
| [chopratejas/invalidate](https://github.com/chopratejas/invalidate) | C, E | screened; no strong match for this scope |
| [choxos/jev-reviewer](https://github.com/choxos/jev-reviewer) | D, E | screened; no strong match for this scope |
| [chrishan17/claude-jev-mod](https://github.com/chrishan17/claude-jev-mod) | C | screened; no strong match for this scope |
| [christianmat/jev-pokemon](https://github.com/christianmat/jev-pokemon) | E | screened; no strong match for this scope |
| [Chuf-H/jev-tree](https://github.com/Chuf-H/jev-tree) | E | screened; no strong match for this scope |
| [chunxiaoxx/nautilus-compass](https://github.com/chunxiaoxx/nautilus-compass) | D | screened; no strong match for this scope |
| [chy4pro/jev-for-chrome](https://github.com/chy4pro/jev-for-chrome) | A, C, D, E | screened; no strong match for this scope |
| [ckaraca/awesome-jev](https://github.com/ckaraca/awesome-jev) | E | collection / discovery source; README screened |
| [cline/plugins](https://github.com/cline/plugins) | A | screened; no strong match for this scope |
| [cobanov/awesome-jev](https://github.com/cobanov/awesome-jev) | D, E | collection / discovery source; README screened |
| [cocktailpeanut/jevthoven](https://github.com/cocktailpeanut/jevthoven) | D, E | screened; no strong match for this scope |
| [CodeAlive-AI/mastra-jev-moderation](https://github.com/CodeAlive-AI/mastra-jev-moderation) | A, E | screened; no strong match for this scope |
| [codeitlikemiley/system-one-adapter-rust](https://github.com/codeitlikemiley/system-one-adapter-rust) | E | screened; no strong match for this scope |
| [codeitlikemiley/typesafe-sdk-rust](https://github.com/codeitlikemiley/typesafe-sdk-rust) | C, E | screened; no strong match for this scope |
| [coldteadotai/abide](https://github.com/coldteadotai/abide) | A, E | screened; no strong match for this scope |
| [cole-gillespie/typesafe-go](https://github.com/cole-gillespie/typesafe-go) | E | screened; no strong match for this scope |
| [colinmcdermott/emoji-jev](https://github.com/colinmcdermott/emoji-jev) | E | screened; no strong match for this scope |
| [colliber/duckdb-jev](https://github.com/colliber/duckdb-jev) | D, E | screened; no strong match for this scope |
| [CommandCodeAI/cmd-mod-jev-nudge](https://github.com/CommandCodeAI/cmd-mod-jev-nudge) | E | screened; no strong match for this scope |
| [community-ports/typesafeai-sdk-rust-community](https://github.com/community-ports/typesafeai-sdk-rust-community) | C | screened; no strong match for this scope |
| [CompleteDotTech/paper-package](https://github.com/CompleteDotTech/paper-package) | E | screened; no strong match for this scope |
| [ComposioHQ/composio](https://github.com/ComposioHQ/composio) | D | screened; no strong match for this scope |
| [compozy/compozy](https://github.com/compozy/compozy) | E | screened; no strong match for this scope |
| [compozy/yoshi](https://github.com/compozy/yoshi) | A, D, E | screened; no strong match for this scope |
| [Contrastive-LM/CLM](https://github.com/Contrastive-LM/CLM) | E | screened; no strong match for this scope |
| [CorieW/JevTest](https://github.com/CorieW/JevTest) | E | screened; no strong match for this scope |
| [cpaczek/s1s](https://github.com/cpaczek/s1s) | E | screened; no strong match for this scope |
| [cwdx/1-million-emojis](https://github.com/cwdx/1-million-emojis) | D | screened; no strong match for this scope |
| [cwdx/chess-with-jev](https://github.com/cwdx/chess-with-jev) | D | screened; no strong match for this scope |
| [d-date/swift-jev](https://github.com/d-date/swift-jev) | E | screened; no strong match for this scope |
| [dabit3/jev-experiments](https://github.com/dabit3/jev-experiments) | A, D | screened; no strong match for this scope |
| [dakdevs/decide-mcp](https://github.com/dakdevs/decide-mcp) | A, E | screened; no strong match for this scope |
| [danielgshea/jev-as-a-judge](https://github.com/danielgshea/jev-as-a-judge) | E | screened; no strong match for this scope |
| [DanielKillenberger/jev-predict-skill](https://github.com/DanielKillenberger/jev-predict-skill) | E | screened; no strong match for this scope |
| [dannote/jev](https://github.com/dannote/jev) | A, C, D, E | screened; no strong match for this scope |
| [DanRWilloughby/snifftest](https://github.com/DanRWilloughby/snifftest) | A, E | screened; no strong match for this scope |
| [danvega/jev-spring-boot-starter](https://github.com/danvega/jev-spring-boot-starter) | C | screened; no strong match for this scope |
| [Das-rebel/a3m-router](https://github.com/Das-rebel/a3m-router) | E | screened; no strong match for this scope |
| [daseinlabs/open-jev](https://github.com/daseinlabs/open-jev) | D, E | screened; no strong match for this scope |
| [datawhalechina/jev-cookbook](https://github.com/datawhalechina/jev-cookbook) | E | screened; no strong match for this scope |
| [davertor/jev-slop-guard](https://github.com/davertor/jev-slop-guard) | A | screened; no strong match for this scope |
| [david-cermak/jevlike-esp32](https://github.com/david-cermak/jevlike-esp32) | A | screened; no strong match for this scope |
| [Davidcreador/pi-dcp](https://github.com/Davidcreador/pi-dcp) | E | screened; no strong match for this scope |
| [davila7/claude-code-templates](https://github.com/davila7/claude-code-templates) | E | screened; no strong match for this scope |
| [davila7/jev-explained](https://github.com/davila7/jev-explained) | E | screened; no strong match for this scope |
| [dbreunig/building-with-jev-skill](https://github.com/dbreunig/building-with-jev-skill) | C, D, E | screened; no strong match for this scope |
| [Dearest/plotveil](https://github.com/Dearest/plotveil) | A, D | screened; no strong match for this scope |
| [DecapodLabs/decapod](https://github.com/DecapodLabs/decapod) | E | screened; no strong match for this scope |
| [DECRUX9812/openjev](https://github.com/DECRUX9812/openjev) | E | screened; no strong match for this scope |
| [DECRUX9812/typesafe-skill-router](https://github.com/DECRUX9812/typesafe-skill-router) | C, E | screened; no strong match for this scope |
| [deepanwadhwa/OpenDecision](https://github.com/deepanwadhwa/OpenDecision) | E | screened; no strong match for this scope |
| [DeepBlueDynamics/typesafe-arena](https://github.com/DeepBlueDynamics/typesafe-arena) | E | screened; no strong match for this scope |
| [deepopen-com/deepopen](https://github.com/deepopen-com/deepopen) | E | screened; no strong match for this scope |
| [delexw/claude-code-trace](https://github.com/delexw/claude-code-trace) | E | screened; no strong match for this scope |
| [devagrawal09/jev-code](https://github.com/devagrawal09/jev-code) | E | screened; no strong match for this scope |
| [devagrawal09/jev-review](https://github.com/devagrawal09/jev-review) | A, C, D, E | screened; no strong match for this scope |
| [devagrawal09/stanley-code](https://github.com/devagrawal09/stanley-code) | A, C, E | screened; no strong match for this scope |
| [devanshbatham/commit-miner](https://github.com/devanshbatham/commit-miner) | D, E | screened; no strong match for this scope |
| [Devin-AXIS/jev-dsh-decision](https://github.com/Devin-AXIS/jev-dsh-decision) | E | screened; no strong match for this scope |
| [DevMortimer/pi-typesafe](https://github.com/DevMortimer/pi-typesafe) | E | screened; no strong match for this scope |
| [DevMortimer/pi-warden](https://github.com/DevMortimer/pi-warden) | C, D, E | screened; no strong match for this scope |
| [deyna256/langchain-skill-router](https://github.com/deyna256/langchain-skill-router) | D, E | screened; no strong match for this scope |
| [dfa1/typesafe-java](https://github.com/dfa1/typesafe-java) | D | screened; no strong match for this scope |
| [Dicklesworthstone/skillranker](https://github.com/Dicklesworthstone/skillranker) | C, D, E | screened; no strong match for this scope |
| [different-ai/openwork](https://github.com/different-ai/openwork) | A, E | screened; no strong match for this scope |
| [Dimesio/typesafe-chess](https://github.com/Dimesio/typesafe-chess) | C | screened; no strong match for this scope |
| [Dimweaker/jev-libero](https://github.com/Dimweaker/jev-libero) | E | screened; no strong match for this scope |
| [dnakhoa/jev-deferred-crispification](https://github.com/dnakhoa/jev-deferred-crispification) | E | screened; no strong match for this scope |
| [docxology/daf-jev](https://github.com/docxology/daf-jev) | E | screened; no strong match for this scope |
| [doeixd/discern](https://github.com/doeixd/discern) | D, E | screened; no strong match for this scope |
| [doeixd/jev-pref](https://github.com/doeixd/jev-pref) | A, D, E | screened; no strong match for this scope |
| [DomMonte/n8n-nodes-typesafe-ai](https://github.com/DomMonte/n8n-nodes-typesafe-ai) | E | screened; no strong match for this scope |
| [donvito/ai-backends](https://github.com/donvito/ai-backends) | E | screened; no strong match for this scope |
| [dorkitude/webctl](https://github.com/dorkitude/webctl) | E | screened; no strong match for this scope |
| [droidrun/mobile-jev](https://github.com/droidrun/mobile-jev) | C, D, E | screened; no strong match for this scope |
| [dtduc-git/jev-packs](https://github.com/dtduc-git/jev-packs) | E | screened; no strong match for this scope |
| [Dujaydis/JevSysUno](https://github.com/Dujaydis/JevSysUno) | E | screened; no strong match for this scope |
| [dzhng/duet-agent](https://github.com/dzhng/duet-agent) | A, E | screened; no strong match for this scope |
| [dzhng/jevgrep](https://github.com/dzhng/jevgrep) | E | screened; no strong match for this scope |
| [earlyaidopters/away-together-starter](https://github.com/earlyaidopters/away-together-starter) | E | screened; no strong match for this scope |
| [edgardcham/huncho](https://github.com/edgardcham/huncho) | A | screened; no strong match for this scope |
| [Effect-TS/effect](https://github.com/Effect-TS/effect) | D | screened; no strong match for this scope |
| [ehui1226/hookmeter-jev](https://github.com/ehui1226/hookmeter-jev) | E | screened; no strong match for this scope |
| [ekizito96/Turn](https://github.com/ekizito96/Turn) | E | screened; no strong match for this scope |
| [ekzhang/openjev-sglang](https://github.com/ekzhang/openjev-sglang) | D, E | screened; no strong match for this scope |
| [EliaAlberti/jev-rules](https://github.com/EliaAlberti/jev-rules) | E | screened; no strong match for this scope |
| [elie222/inbox-zero](https://github.com/elie222/inbox-zero) | D | screened; no strong match for this scope |
| [Eliot5566/JEV-Paper-Radar](https://github.com/Eliot5566/JEV-Paper-Radar) | C, D, E | screened; no strong match for this scope |
| [ellipsis-dev/blink](https://github.com/ellipsis-dev/blink) | D, E | screened; no strong match for this scope |
| [ellistev/typesafe-minecraft-demo](https://github.com/ellistev/typesafe-minecraft-demo) | E | screened; no strong match for this scope |
| [elpumberto/barrunto](https://github.com/elpumberto/barrunto) | E | screened; no strong match for this scope |
| [emnlmn/snap](https://github.com/emnlmn/snap) | E | screened; no strong match for this scope |
| [emrickgarrett/OneVOneJev](https://github.com/emrickgarrett/OneVOneJev) | C, E | screened; no strong match for this scope |
| [erendikmenn/jev-rag-benchmark](https://github.com/erendikmenn/jev-rag-benchmark) | E | screened lead; requires follow-up |
| [eriestra/almond-fastloop](https://github.com/eriestra/almond-fastloop) | E | unavailable README |
| [eriestra/browser-use-olympics](https://github.com/eriestra/browser-use-olympics) | E | screened; no strong match for this scope |
| [Eriskii/ErisLint](https://github.com/Eriskii/ErisLint) | E | screened; no strong match for this scope |
| [EthanAlgoX/AIStock](https://github.com/EthanAlgoX/AIStock) | E | screened; no strong match for this scope |
| [ethanplusai/jev-chat-for-twitch](https://github.com/ethanplusai/jev-chat-for-twitch) | E | screened; no strong match for this scope |
| [etweisberg/jev-ui](https://github.com/etweisberg/jev-ui) | E | screened; no strong match for this scope |
| [EugeneBoondock/jevsql](https://github.com/EugeneBoondock/jevsql) | D, E | screened; no strong match for this scope |
| [eugeniughelbur/jev-engineering](https://github.com/eugeniughelbur/jev-engineering) | A, D | screened; no strong match for this scope |
| [everyai-com/jev-directory](https://github.com/everyai-com/jev-directory) | E | screened; no strong match for this scope |
| [everyinfra/jev-radar](https://github.com/everyinfra/jev-radar) | C, E | screened; no strong match for this scope |
| [evoke-build/evoke](https://github.com/evoke-build/evoke) | E | screened; no strong match for this scope |
| [f/jev-leftpad](https://github.com/f/jev-leftpad) | E | screened; no strong match for this scope |
| [fabricioctelles/skills](https://github.com/fabricioctelles/skills) | E | screened; no strong match for this scope |
| [fatwang2/awesome-jev](https://github.com/fatwang2/awesome-jev) | E | collection / discovery source; README screened |
| [fazlerocks/jevmail](https://github.com/fazlerocks/jevmail) | D, E | screened; no strong match for this scope |
| [FBddcz/embodied-jev](https://github.com/FBddcz/embodied-jev) | D, E | screened; no strong match for this scope |
| [featherless-ai/simple-jev](https://github.com/featherless-ai/simple-jev) | D, E | screened; no strong match for this scope |
| [feder-cr/jev](https://github.com/feder-cr/jev) | E | screened; no strong match for this scope |
| [FeiLiuEM/open-medical-jev](https://github.com/FeiLiuEM/open-medical-jev) | A, E | screened; no strong match for this scope |
| [FeiLiuEM/tetrajev](https://github.com/FeiLiuEM/tetrajev) | A | screened; no strong match for this scope |
| [FerryCorleone/crush-monitor](https://github.com/FerryCorleone/crush-monitor) | E | screened; no strong match for this scope |
| [FFatTiger/new-api-plugin-typesafe](https://github.com/FFatTiger/new-api-plugin-typesafe) | A, C, E | screened; no strong match for this scope |
| [fgn/jevgo](https://github.com/fgn/jevgo) | C, E | screened; no strong match for this scope |
| [fhshaik/typesafe-mario](https://github.com/fhshaik/typesafe-mario) | A, C, D, E | screened; no strong match for this scope |
| [fidecastro/jevify](https://github.com/fidecastro/jevify) | E | screened; no strong match for this scope |
| [filedcom/playjev](https://github.com/filedcom/playjev) | E | screened; no strong match for this scope |
| [FirasSX914/Janus](https://github.com/FirasSX914/Janus) | D, E | screened; no strong match for this scope |
| [flyryan/ai-news-aggregator](https://github.com/flyryan/ai-news-aggregator) | E | screened; no strong match for this scope |
| [Foadsf/jev-for-engineers](https://github.com/Foadsf/jev-for-engineers) | E | screened; no strong match for this scope |
| [formulahendry/jev-acp](https://github.com/formulahendry/jev-acp) | D | screened; no strong match for this scope |
| [fornhere/hafiza-os](https://github.com/fornhere/hafiza-os) | E | source inspected; directly relevant |
| [forvela/jev-agent-browser](https://github.com/forvela/jev-agent-browser) | A, C, E | screened; no strong match for this scope |
| [Foxtailsss-Andy/Anna-Agent](https://github.com/Foxtailsss-Andy/Anna-Agent) | E | screened; no strong match for this scope |
| [FrancoisChastel/jev-code](https://github.com/FrancoisChastel/jev-code) | E | screened; no strong match for this scope |
| [frankda/jev-poly-crypto-demo](https://github.com/frankda/jev-poly-crypto-demo) | E | screened; no strong match for this scope |
| [FreedomIntelligence/Tiermem](https://github.com/FreedomIntelligence/Tiermem) | E | screened; no strong match for this scope |
| [freepik-company/jev-mcp](https://github.com/freepik-company/jev-mcp) | E | screened; no strong match for this scope |
| [Friedjof/jev-mobile](https://github.com/Friedjof/jev-mobile) | B, E | screened; no strong match for this scope |
| [frostney/clean-code-review](https://github.com/frostney/clean-code-review) | A, E | screened; no strong match for this scope |
| [fstandhartinger/jevbench](https://github.com/fstandhartinger/jevbench) | D, E | screened; no strong match for this scope |
| [gaborishka/jev-canvas](https://github.com/gaborishka/jev-canvas) | A, D | screened; no strong match for this scope |
| [gaborishka/jev-wrapped](https://github.com/gaborishka/jev-wrapped) | D | screened; no strong match for this scope |
| [gaborishka/jevtown](https://github.com/gaborishka/jevtown) | D | screened; no strong match for this scope |
| [gamesonrblx/Jevbridge](https://github.com/gamesonrblx/Jevbridge) | D, E | screened; no strong match for this scope |
| [gargpratyush/jev-router](https://github.com/gargpratyush/jev-router) | A, C, D, E | screened; no strong match for this scope |
| [Gaurav-Gosain/jev-alpha-bench](https://github.com/Gaurav-Gosain/jev-alpha-bench) | E | screened; no strong match for this scope |
| [Gaurav-Gosain/jev-go](https://github.com/Gaurav-Gosain/jev-go) | E | screened; no strong match for this scope |
| [Gaurav-Gosain/jev-headline-bench](https://github.com/Gaurav-Gosain/jev-headline-bench) | E | screened; no strong match for this scope |
| [Gaurav-Gosain/jev-sec-bench](https://github.com/Gaurav-Gosain/jev-sec-bench) | E | screened; no strong match for this scope |
| [GeekLinkDev/jev-subtitle-translator](https://github.com/GeekLinkDev/jev-subtitle-translator) | A, E | screened; no strong match for this scope |
| [geilt/typesafe-cli](https://github.com/geilt/typesafe-cli) | E | screened; no strong match for this scope |
| [genai-craft/openvons](https://github.com/genai-craft/openvons) | D, E | screened; no strong match for this scope |
| [GenieRobot/typesafe-ai-rails](https://github.com/GenieRobot/typesafe-ai-rails) | E | screened; no strong match for this scope |
| [genspark-ai/genoffice](https://github.com/genspark-ai/genoffice) | E | screened; no strong match for this scope |
| [get-convex/convex-evals](https://github.com/get-convex/convex-evals) | D, E | screened; no strong match for this scope |
| [getainode/jebadiah](https://github.com/getainode/jebadiah) | A | screened; no strong match for this scope |
| [getexcited/stepwarden](https://github.com/getexcited/stepwarden) | E | screened; no strong match for this scope |
| [GhalebDweikat/winnow](https://github.com/GhalebDweikat/winnow) | D, E | screened; no strong match for this scope |
| [GhrezaKh74/JevTicktRouter](https://github.com/GhrezaKh74/JevTicktRouter) | E | screened; no strong match for this scope |
| [GiesN/typesafe-jev-workflow](https://github.com/GiesN/typesafe-jev-workflow) | A, D, E | screened; no strong match for this scope |
| [gilljon/typesafe-ai-rs](https://github.com/gilljon/typesafe-ai-rs) | C, E | screened; no strong match for this scope |
| [githubnext/localjev](https://github.com/githubnext/localjev) | E | screened; no strong match for this scope |
| [giuliosmall/pg_typesafe](https://github.com/giuliosmall/pg_typesafe) | C, D, E | screened; no strong match for this scope |
| [gloridifice/pi-jev-router](https://github.com/gloridifice/pi-jev-router) | E | unavailable README |
| [glowbom/glowbom-oss](https://github.com/glowbom/glowbom-oss) | E | screened; no strong match for this scope |
| [GodsBoy/jev-agent-skill-router](https://github.com/GodsBoy/jev-agent-skill-router) | A, C, E | screened; no strong match for this scope |
| [GoldenLoaf24h/browserclaw](https://github.com/GoldenLoaf24h/browserclaw) | A, D | screened; no strong match for this scope |
| [goodrahstar/jev-column-race](https://github.com/goodrahstar/jev-column-race) | E | screened; no strong match for this scope |
| [greenyamao/Antigravity-mcp-semantic-search-with-TypeSafeAi](https://github.com/greenyamao/Antigravity-mcp-semantic-search-with-TypeSafeAi) | E | screened lead; requires follow-up |
| [grishahq/decisionbridge](https://github.com/grishahq/decisionbridge) | E | screened; no strong match for this scope |
| [grmkris/robo-harness](https://github.com/grmkris/robo-harness) | A, E | screened; no strong match for this scope |
| [gtaras7/typesafe-jev](https://github.com/gtaras7/typesafe-jev) | A, C, E | screened; no strong match for this scope |
| [guillemus/jev-go](https://github.com/guillemus/jev-go) | C, E | screened; no strong match for this scope |
| [HackSing/jev-report](https://github.com/HackSing/jev-report) | E | screened; no strong match for this scope |
| [haibt163/jev](https://github.com/haibt163/jev) | E | screened; no strong match for this scope |
| [hamakyo/jev-starter](https://github.com/hamakyo/jev-starter) | C | screened; no strong match for this scope |
| [Hangzhi/diffusion-jev-sglang](https://github.com/Hangzhi/diffusion-jev-sglang) | D, E | screened; no strong match for this scope |
| [Hardel-DW/jev.mods](https://github.com/Hardel-DW/jev.mods) | E | unavailable README |
| [hardness1020/learn-agent-architecture](https://github.com/hardness1020/learn-agent-architecture) | E | screened; no strong match for this scope |
| [harrymunro/decision-first](https://github.com/harrymunro/decision-first) | E | screened; no strong match for this scope |
| [harshil1712/slidepilot](https://github.com/harshil1712/slidepilot) | C, D, E | screened; no strong match for this scope |
| [harshithsunku/learn-jev-end-to-end](https://github.com/harshithsunku/learn-jev-end-to-end) | C | screened; no strong match for this scope |
| [haseeb-heaven/jev-system-one](https://github.com/haseeb-heaven/jev-system-one) | E | screened; no strong match for this scope |
| [hatt-io/jevkeep](https://github.com/hatt-io/jevkeep) | E | screened; no strong match for this scope |
| [hawkymisc/typed-decision-bert](https://github.com/hawkymisc/typed-decision-bert) | E | screened; no strong match for this scope |
| [Hawxy/TypeSafeAI.Net](https://github.com/Hawxy/TypeSafeAI.Net) | E | screened; no strong match for this scope |
| [haxitag/Yueli-Dex](https://github.com/haxitag/Yueli-Dex) | E | screened lead; requires follow-up |
| [hegargarcia/jev-playground](https://github.com/hegargarcia/jev-playground) | A, E | screened; no strong match for this scope |
| [heiwa4126/jev-bun1](https://github.com/heiwa4126/jev-bun1) | E | screened; no strong match for this scope |
| [hellogumbo/awesome-jev](https://github.com/hellogumbo/awesome-jev) | D, E | collection / discovery source; README screened |
| [hellogumbo/should-ai-kill-us-all](https://github.com/hellogumbo/should-ai-kill-us-all) | E | screened; no strong match for this scope |
| [hellozenstrategist-lab/eutrya](https://github.com/hellozenstrategist-lab/eutrya) | E | screened; no strong match for this scope |
| [Heman10x-NGU/openJev-verdict-2.0](https://github.com/Heman10x-NGU/openJev-verdict-2.0) | C, D | screened; no strong match for this scope |
| [Heman10x-NGU/Verdict-open-jev](https://github.com/Heman10x-NGU/Verdict-open-jev) | C, E | screened; no strong match for this scope |
| [hemanth/pkg-gate](https://github.com/hemanth/pkg-gate) | E | screened; no strong match for this scope |
| [hev/reranker](https://github.com/hev/reranker) | E | screened lead; requires follow-up |
| [Hexdigest123/typesafe-comment](https://github.com/Hexdigest123/typesafe-comment) | E | screened; no strong match for this scope |
| [heyjunpenn/awesome-jev](https://github.com/heyjunpenn/awesome-jev) | E | collection / discovery source; README screened |
| [hfiguera/typesafe_ai](https://github.com/hfiguera/typesafe_ai) | E | screened; no strong match for this scope |
| [HiepPP/hiep-paseo-plugin](https://github.com/HiepPP/hiep-paseo-plugin) | E | screened; no strong match for this scope |
| [hifizz/jev-finance-benchmark](https://github.com/hifizz/jev-finance-benchmark) | E | unavailable README |
| [Hiwoniu/Jev-Case](https://github.com/Hiwoniu/Jev-Case) | E | screened; no strong match for this scope |
| [hnegishi/typesafe-ai-ruby](https://github.com/hnegishi/typesafe-ai-ruby) | C | screened; no strong match for this scope |
| [Horace-Maxwell/horosa-skill](https://github.com/Horace-Maxwell/horosa-skill) | E | screened; no strong match for this scope |
| [hotchpotch/jev-reranker](https://github.com/hotchpotch/jev-reranker) | A, D, E | relevant README; see discussion |
| [hqman/jev-browser-skill](https://github.com/hqman/jev-browser-skill) | E | screened; no strong match for this scope |
| [hr98w/jev-visual](https://github.com/hr98w/jev-visual) | C, D, E | screened; no strong match for this scope |
| [hraness/algal](https://github.com/hraness/algal) | E | screened; no strong match for this scope |
| [hunkim/solar-mini4-jev](https://github.com/hunkim/solar-mini4-jev) | E | screened; no strong match for this scope |
| [huntedman/JevLint](https://github.com/huntedman/JevLint) | E | screened; no strong match for this scope |
| [hyperspaceai/jevcache](https://github.com/hyperspaceai/jevcache) | C | screened; no strong match for this scope |
| [HyunjunJeon/jev-judgment](https://github.com/HyunjunJeon/jev-judgment) | A, E | screened; no strong match for this scope |
| [HyunjunJeon/pi-quiet-ask](https://github.com/HyunjunJeon/pi-quiet-ask) | A, C, E | screened; no strong match for this scope |
| [i2cjak/RISC-jeV](https://github.com/i2cjak/RISC-jeV) | E | screened; no strong match for this scope |
| [iamaamir/system-one](https://github.com/iamaamir/system-one) | E | screened; no strong match for this scope |
| [IamBusy/OpenJev-Vision](https://github.com/IamBusy/OpenJev-Vision) | E | screened; no strong match for this scope |
| [iammrduncan/typesafe-ai-benchmark](https://github.com/iammrduncan/typesafe-ai-benchmark) | D, E | screened; no strong match for this scope |
| [iammusham/jev-snake](https://github.com/iammusham/jev-snake) | E | screened; no strong match for this scope |
| [iamtoomas/JevLint](https://github.com/iamtoomas/JevLint) | E | screened; no strong match for this scope |
| [IAmUnbounded/save-token-jev-clean](https://github.com/IAmUnbounded/save-token-jev-clean) | E | screened; no strong match for this scope |
| [iamvatsalpatel/tiershift](https://github.com/iamvatsalpatel/tiershift) | D, E | screened; no strong match for this scope |
| [iapp-technology/openthai-systemone](https://github.com/iapp-technology/openthai-systemone) | E | screened; no strong match for this scope |
| [Icohen007/jev-play-ping-pong](https://github.com/Icohen007/jev-play-ping-pong) | E | screened; no strong match for this scope |
| [idovmamane/dejevu](https://github.com/idovmamane/dejevu) | E | screened; no strong match for this scope |
| [iefnaf/pi-jev](https://github.com/iefnaf/pi-jev) | E | screened; no strong match for this scope |
| [iJ03l/jear](https://github.com/iJ03l/jear) | C, E | screened; no strong match for this scope |
| [ikermoel/open-alternative-jev](https://github.com/ikermoel/open-alternative-jev) | A, E | screened; no strong match for this scope |
| [ilkerulusoy/pi-jev-compact](https://github.com/ilkerulusoy/pi-jev-compact) | D | screened; no strong match for this scope |
| [ilyamk/jev-gmail-ai-spam-filter-and-labeling](https://github.com/ilyamk/jev-gmail-ai-spam-filter-and-labeling) | E | screened; no strong match for this scope |
| [imikerussell/beebots](https://github.com/imikerussell/beebots) | E | screened; no strong match for this scope |
| [inanna-malick/jev-dsl](https://github.com/inanna-malick/jev-dsl) | D, E | screened; no strong match for this scope |
| [innocentdiaz/s1_ruby](https://github.com/innocentdiaz/s1_ruby) | A | screened; no strong match for this scope |
| [InsaneArts/typesafe-sdk-swift](https://github.com/InsaneArts/typesafe-sdk-swift) | E | screened; no strong match for this scope |
| [intikhab49/open-jev-typed-decision-engine](https://github.com/intikhab49/open-jev-typed-decision-engine) | C, E | screened; no strong match for this scope |
| [ipenywis/laya-ultrafast](https://github.com/ipenywis/laya-ultrafast) | E | screened; no strong match for this scope |
| [irfndi/prism-liquidity-agent](https://github.com/irfndi/prism-liquidity-agent) | E | screened; no strong match for this scope |
| [islee23520/omo-jevlike-router](https://github.com/islee23520/omo-jevlike-router) | A | screened; no strong match for this scope |
| [ItIsCuthNotCup/MetaCog](https://github.com/ItIsCuthNotCup/MetaCog) | E | screened; no strong match for this scope |
| [ItisNoMatter/kojev](https://github.com/ItisNoMatter/kojev) | A, D | screened; no strong match for this scope |
| [ItisShikhar/gg-friggin-ez](https://github.com/ItisShikhar/gg-friggin-ez) | C | screened; no strong match for this scope |
| [itsmostafa/typesafe-mcp](https://github.com/itsmostafa/typesafe-mcp) | C, D, E | screened; no strong match for this scope |
| [Jackalope-Dev/jackalope](https://github.com/Jackalope-Dev/jackalope) | E | screened; no strong match for this scope |
| [jackbarunz/jev-tool-router](https://github.com/jackbarunz/jev-tool-router) | E | screened; no strong match for this scope |
| [JackZeng/Jev_apps](https://github.com/JackZeng/Jev_apps) | E | screened; no strong match for this scope |
| [JacobLinCool/jev-paper-judge](https://github.com/JacobLinCool/jev-paper-judge) | E | screened; no strong match for this scope |
| [JacquesGariepy/ORIGIN-CIVILIZATION](https://github.com/JacquesGariepy/ORIGIN-CIVILIZATION) | C | screened; no strong match for this scope |
| [jamesward/zio-typesafe-ai](https://github.com/jamesward/zio-typesafe-ai) | A, E | screened; no strong match for this scope |
| [jammaru/jev-lab](https://github.com/jammaru/jev-lab) | E | screened; no strong match for this scope |
| [JanOstrowka/typesafe-assist](https://github.com/JanOstrowka/typesafe-assist) | E | screened; no strong match for this scope |
| [jaredpalmer/kev](https://github.com/jaredpalmer/kev) | A, C, D, E | screened; no strong match for this scope |
| [jarrodwatts/jev-trader](https://github.com/jarrodwatts/jev-trader) | C, D, E | screened; no strong match for this scope |
| [jbt95/jev-toolkit](https://github.com/jbt95/jev-toolkit) | E | screened; no strong match for this scope |
| [jcpsimmons/jev-macos-loop](https://github.com/jcpsimmons/jev-macos-loop) | E | screened; no strong match for this scope |
| [JedimEmO/typesafe-client](https://github.com/JedimEmO/typesafe-client) | E | screened; no strong match for this scope |
| [jekozyra/pi-typesafe-router](https://github.com/jekozyra/pi-typesafe-router) | A | screened; no strong match for this scope |
| [jerryfane/omp-jev-compaction](https://github.com/jerryfane/omp-jev-compaction) | E | screened lead; requires follow-up |
| [jerryjliu/docjev](https://github.com/jerryjliu/docjev) | E | screened; no strong match for this scope |
| [jesset/pi-verdict](https://github.com/jesset/pi-verdict) | A | screened; no strong match for this scope |
| [Jessie-QingYu/jev-in-the-wild](https://github.com/Jessie-QingYu/jev-in-the-wild) | D | screened; no strong match for this scope |
| [jev-chat/jev-chat-jarvis](https://github.com/jev-chat/jev-chat-jarvis) | E | screened; no strong match for this scope |
| [jev-chat/jev-chat-jarvis-mac](https://github.com/jev-chat/jev-chat-jarvis-mac) | E | screened; no strong match for this scope |
| [jev-chat/jev-chat-windows](https://github.com/jev-chat/jev-chat-windows) | E | screened; no strong match for this scope |
| [jev-ids/jev-ids](https://github.com/jev-ids/jev-ids) | D | screened; no strong match for this scope |
| [Jevals/jevals-data](https://github.com/Jevals/jevals-data) | D, E | screened; no strong match for this scope |
| [jexp/neo4jev](https://github.com/jexp/neo4jev) | E | source inspected; directly relevant |
| [jgridifier/jev-research-eval](https://github.com/jgridifier/jev-research-eval) | A, E | screened lead; requires follow-up |
| [jhd3197/Prompture](https://github.com/jhd3197/Prompture) | E | screened; no strong match for this scope |
| [Jhonnyr97/JevGuard](https://github.com/Jhonnyr97/JevGuard) | E | screened; no strong match for this scope |
| [jiawei686/jev-ultrafast-mcp](https://github.com/jiawei686/jev-ultrafast-mcp) | E | screened; no strong match for this scope |
| [jkudish/jev-browser](https://github.com/jkudish/jev-browser) | A, B, C, D, E | screened; no strong match for this scope |
| [jkudish/jev-mcp](https://github.com/jkudish/jev-mcp) | A, C, E | screened; no strong match for this scope |
| [jmanhype/jev-dspy-lab](https://github.com/jmanhype/jev-dspy-lab) | E | screened; no strong match for this scope |
| [joelhooks/pi-fast-jev-compaction](https://github.com/joelhooks/pi-fast-jev-compaction) | A, C, D, E | screened lead; requires follow-up |
| [joevidev/ui-generator-instinct-jev](https://github.com/joevidev/ui-generator-instinct-jev) | E | screened; no strong match for this scope |
| [johnhughes3/LegalForecastBench](https://github.com/johnhughes3/LegalForecastBench) | A, E | screened; no strong match for this scope |
| [jomatsu/pi-jev-auto-mode](https://github.com/jomatsu/pi-jev-auto-mode) | C, E | screened; no strong match for this scope |
| [jomatsu/zod-jev](https://github.com/jomatsu/zod-jev) | D, E | screened; no strong match for this scope |
| [jon-devlapaz/jev-me](https://github.com/jon-devlapaz/jev-me) | E | screened; no strong match for this scope |
| [joonan-lab/byeori](https://github.com/joonan-lab/byeori) | E | relevant README; see discussion |
| [JordiParraCrespo/typesafe-ai-trading-showcase](https://github.com/JordiParraCrespo/typesafe-ai-trading-showcase) | E | screened; no strong match for this scope |
| [joshlarsen/jev-t-rex-runner](https://github.com/joshlarsen/jev-t-rex-runner) | E | screened; no strong match for this scope |
| [joshmn/typesafe-sdk](https://github.com/joshmn/typesafe-sdk) | E | screened; no strong match for this scope |
| [JoshuaSP/open-jev](https://github.com/JoshuaSP/open-jev) | C, E | screened; no strong match for this scope |
| [jtnkminimal/awesome-jev](https://github.com/jtnkminimal/awesome-jev) | E | collection / discovery source; README screened |
| [jtsang4/jev-cli](https://github.com/jtsang4/jev-cli) | E | screened; no strong match for this scope |
| [juancamiloqhz/roverlab](https://github.com/juancamiloqhz/roverlab) | E | screened; no strong match for this scope |
| [jujumilk3/jev-calibration-audit](https://github.com/jujumilk3/jev-calibration-audit) | C, E | screened; no strong match for this scope |
| [juspay/neurolink](https://github.com/juspay/neurolink) | D, E | screened; no strong match for this scope |
| [justinhe16/trade-jev](https://github.com/justinhe16/trade-jev) | E | screened; no strong match for this scope |
| [jvsteiner/jevex](https://github.com/jvsteiner/jevex) | E | screened; no strong match for this scope |
| [jxucoder/mimicry](https://github.com/jxucoder/mimicry) | E | screened; no strong match for this scope |
| [jyatesdotdev/jev-logtriage](https://github.com/jyatesdotdev/jev-logtriage) | D | screened; no strong match for this scope |
| [KamilPostrozny/pi-jev-code](https://github.com/KamilPostrozny/pi-jev-code) | E | screened; no strong match for this scope |
| [KantaHayashiAI/jev-does-not-play-dice](https://github.com/KantaHayashiAI/jev-does-not-play-dice) | D, E | screened; no strong match for this scope |
| [karanb192/jev-architect](https://github.com/karanb192/jev-architect) | E | screened; no strong match for this scope |
| [karimatayuta/tiny-jev](https://github.com/karimatayuta/tiny-jev) | E | screened; no strong match for this scope |
| [karminski/Jev-Quantum](https://github.com/karminski/Jev-Quantum) | E | screened; no strong match for this scope |
| [kaustav1996/reflex](https://github.com/kaustav1996/reflex) | C | screened; no strong match for this scope |
| [kavehmz/typesafe-playground](https://github.com/kavehmz/typesafe-playground) | A, C, E | screened; no strong match for this scope |
| [kbhuw/jev-sift](https://github.com/kbhuw/jev-sift) | E | screened; no strong match for this scope |
| [keeltrace/hermes-jev](https://github.com/keeltrace/hermes-jev) | C, E | screened; no strong match for this scope |
| [keeltrace/hermes-nerve](https://github.com/keeltrace/hermes-nerve) | C, E | screened; no strong match for this scope |
| [Kelbie/hunch](https://github.com/Kelbie/hunch) | A, E | screened; no strong match for this scope |
| [keltokhy/jgrep](https://github.com/keltokhy/jgrep) | C, E | screened; no strong match for this scope |
| [keltokhy/jlink](https://github.com/keltokhy/jlink) | E | screened; no strong match for this scope |
| [keltokhy/jselect](https://github.com/keltokhy/jselect) | E | screened; no strong match for this scope |
| [keltokhy/jsort](https://github.com/keltokhy/jsort) | E | screened; no strong match for this scope |
| [kerpopule/hermes-jev-skills](https://github.com/kerpopule/hermes-jev-skills) | C, E | screened; no strong match for this scope |
| [KesavanKing/jev-browser](https://github.com/KesavanKing/jev-browser) | E | screened; no strong match for this scope |
| [kesku/jev-freeform](https://github.com/kesku/jev-freeform) | E | screened; no strong match for this scope |
| [kevinbadi/jev-voice](https://github.com/kevinbadi/jev-voice) | E | screened; no strong match for this scope |
| [kevinpita/pi-jev-context](https://github.com/kevinpita/pi-jev-context) | D | screened; no strong match for this scope |
| [Kevthetech143/super-jev](https://github.com/Kevthetech143/super-jev) | A, C, E | screened; no strong match for this scope |
| [khordoo/jev-reflex-autonomy-lab](https://github.com/khordoo/jev-reflex-autonomy-lab) | C, E | screened; no strong match for this scope |
| [kiarina/labs](https://github.com/kiarina/labs) | E | screened; no strong match for this scope |
| [kierandotai/jev-scout](https://github.com/kierandotai/jev-scout) | D | screened; no strong match for this scope |
| [kieranklaassen/ruby_llm-typesafe](https://github.com/kieranklaassen/ruby_llm-typesafe) | E | screened; no strong match for this scope |
| [kikoncuo/jevfire](https://github.com/kikoncuo/jevfire) | E | screened; no strong match for this scope |
| [kisshan13/typesafe-ai-go](https://github.com/kisshan13/typesafe-ai-go) | E | screened; no strong match for this scope |
| [kitfunso/hippo-memory](https://github.com/kitfunso/hippo-memory) | E | source inspected; directly relevant |
| [kitze/pagegrade](https://github.com/kitze/pagegrade) | A, C, E | screened; no strong match for this scope |
| [kitze/skillbox](https://github.com/kitze/skillbox) | A, C, D, E | screened; no strong match for this scope |
| [kitze/unclutter](https://github.com/kitze/unclutter) | A, C, D, E | screened; no strong match for this scope |
| [kiwi0719/jev-edge](https://github.com/kiwi0719/jev-edge) | C, E | screened; no strong match for this scope |
| [klauswg/jev-guard](https://github.com/klauswg/jev-guard) | C, D, E | screened; no strong match for this scope |
| [klauswg/jev-suite](https://github.com/klauswg/jev-suite) | C, E | screened; no strong match for this scope |
| [knowlet/jevlens](https://github.com/knowlet/jevlens) | E | screened; no strong match for this scope |
| [koala73/worldmonitor](https://github.com/koala73/worldmonitor) | D | screened; no strong match for this scope |
| [komikat/jev-bfs](https://github.com/komikat/jev-bfs) | A, E | screened; no strong match for this scope |
| [komikat/psearch](https://github.com/komikat/psearch) | E | screened lead; requires follow-up |
| [kotoba-lang/typed-decisions](https://github.com/kotoba-lang/typed-decisions) | E | screened; no strong match for this scope |
| [kraayenjon/awesome-jev](https://github.com/kraayenjon/awesome-jev) | E | collection / discovery source; README screened |
| [krw82/jev-playwright-mcp](https://github.com/krw82/jev-playwright-mcp) | E | screened; no strong match for this scope |
| [krzyzanowskim/TypeSafe](https://github.com/krzyzanowskim/TypeSafe) | E | screened; no strong match for this scope |
| [kshetrajna12/reflex](https://github.com/kshetrajna12/reflex) | D, E | screened; no strong match for this scope |
| [ktaletsk/jevframe](https://github.com/ktaletsk/jevframe) | E | screened; no strong match for this scope |
| [kubet/azdaja](https://github.com/kubet/azdaja) | E | screened; no strong match for this scope |
| [kuhung/understanding-jev](https://github.com/kuhung/understanding-jev) | A | screened; no strong match for this scope |
| [kunchenguid/compact-adviser](https://github.com/kunchenguid/compact-adviser) | E | screened; no strong match for this scope |
| [kunobi-ninja/kunobi-jev](https://github.com/kunobi-ninja/kunobi-jev) | E | screened; no strong match for this scope |
| [kunpengtalk/OmniStudio](https://github.com/kunpengtalk/OmniStudio) | E | screened; no strong match for this scope |
| [kxzk/typesafe-jev-drone-demo](https://github.com/kxzk/typesafe-jev-drone-demo) | A, E | screened; no strong match for this scope |
| [kydlikebtc/awesome-jev](https://github.com/kydlikebtc/awesome-jev) | E | collection / discovery source; README screened |
| [kyegomez/open-jev](https://github.com/kyegomez/open-jev) | E | screened; no strong match for this scope |
| [kylehovance-ai/jev-the-janitor](https://github.com/kylehovance-ai/jev-the-janitor) | E | source inspected; directly relevant |
| [KyleKreuter/jev2048](https://github.com/KyleKreuter/jev2048) | E | screened; no strong match for this scope |
| [kylemclaren/jevpdf](https://github.com/kylemclaren/jevpdf) | A, C, D, E | screened; no strong match for this scope |
| [kylemclaren/jevql](https://github.com/kylemclaren/jevql) | A, C, D, E | screened; no strong match for this scope |
| [kylemclaren/jevsearch](https://github.com/kylemclaren/jevsearch) | A, C, D, E | relevant README; see discussion |
| [kyotofin/tax-doc-classifier](https://github.com/kyotofin/tax-doc-classifier) | D, E | screened; no strong match for this scope |
| [kyrylosyzonenko/jev-browse](https://github.com/kyrylosyzonenko/jev-browse) | E | screened; no strong match for this scope |
| [kyu1204/jgrep](https://github.com/kyu1204/jgrep) | C, D, E | screened; no strong match for this scope |
| [kyu1204/oh-my-harness](https://github.com/kyu1204/oh-my-harness) | E | screened; no strong match for this scope |
| [lab-dados/jev-anotacao-sentencas](https://github.com/lab-dados/jev-anotacao-sentencas) | E | screened; no strong match for this scope |
| [lahfir/agent-desktop](https://github.com/lahfir/agent-desktop) | E | screened; no strong match for this scope |
| [lakeday-org/perch](https://github.com/lakeday-org/perch) | D, E | screened; no strong match for this scope |
| [lambertsj/beatjev](https://github.com/lambertsj/beatjev) | E | screened; no strong match for this scope |
| [LamplighterPaul/forma-system1-experiment](https://github.com/LamplighterPaul/forma-system1-experiment) | E | screened; no strong match for this scope |
| [langchain-ai/langchain](https://github.com/langchain-ai/langchain) | D | screened; no strong match for this scope |
| [langchain-ai/langchainjs](https://github.com/langchain-ai/langchainjs) | D | screened; no strong match for this scope |
| [langwatch/langwatch](https://github.com/langwatch/langwatch) | E | screened; no strong match for this scope |
| [laravel/ai](https://github.com/laravel/ai) | E | screened; no strong match for this scope |
| [lawrence3699/jev-style](https://github.com/lawrence3699/jev-style) | E | screened; no strong match for this scope |
| [lbotinelly/jev-little-airways](https://github.com/lbotinelly/jev-little-airways) | E | screened; no strong match for this scope |
| [leepokai/jev-guard](https://github.com/leepokai/jev-guard) | A, D, E | screened; no strong match for this scope |
| [leesk212/JEV-CPU](https://github.com/leesk212/JEV-CPU) | E | screened; no strong match for this scope |
| [legacybridge-tech/pi-typesafe-jev](https://github.com/legacybridge-tech/pi-typesafe-jev) | A, E | screened; no strong match for this scope |
| [leonaaardob/fast-dev-compaction](https://github.com/leonaaardob/fast-dev-compaction) | A, D, E | screened lead; requires follow-up |
| [lexmount/jev-browser-bridge](https://github.com/lexmount/jev-browser-bridge) | A, E | screened; no strong match for this scope |
| [lhemerly/mcts-agent](https://github.com/lhemerly/mcts-agent) | E | screened; no strong match for this scope |
| [lhotwll217/jev-cli](https://github.com/lhotwll217/jev-cli) | E | screened; no strong match for this scope |
| [liaoyuhua/jev-trip](https://github.com/liaoyuhua/jev-trip) | D | screened; no strong match for this scope |
| [libingzheren/Jev-Mem](https://github.com/libingzheren/Jev-Mem) | E | screened; no strong match for this scope |
| [lirantal/discoprint](https://github.com/lirantal/discoprint) | E | screened; no strong match for this scope |
| [Little-Planet-Labs/jev-playground](https://github.com/Little-Planet-Labs/jev-playground) | E | screened; no strong match for this scope |
| [liu-x27/XavierJev](https://github.com/liu-x27/XavierJev) | E | screened; no strong match for this scope |
| [liuhongrui087-art/jev-routed-agent](https://github.com/liuhongrui087-art/jev-routed-agent) | E | screened; no strong match for this scope |
| [Liuziyu77/Valen](https://github.com/Liuziyu77/Valen) | E | screened; no strong match for this scope |
| [Liyucheng1997/332_lab-jev-chat](https://github.com/Liyucheng1997/332_lab-jev-chat) | E | screened; no strong match for this scope |
| [logan-markewich/jeff](https://github.com/logan-markewich/jeff) | C, E | screened; no strong match for this scope |
| [logicrw/awesome-jev-projects](https://github.com/logicrw/awesome-jev-projects) | E | collection / discovery source; README screened |
| [lomeshdutta/skill-router](https://github.com/lomeshdutta/skill-router) | E | screened; no strong match for this scope |
| [longkou1988/cnki-skills](https://github.com/longkou1988/cnki-skills) | E | screened; no strong match for this scope |
| [lu-zero/systemone](https://github.com/lu-zero/systemone) | C | screened; no strong match for this scope |
| [luantak/is-malicious](https://github.com/luantak/is-malicious) | A, D, E | screened; no strong match for this scope |
| [luigivis/jev-sdk-java](https://github.com/luigivis/jev-sdk-java) | E | screened; no strong match for this scope |
| [lukaske/jev-doom-agent](https://github.com/lukaske/jev-doom-agent) | E | screened; no strong match for this scope |
| [lukstei/slop-grader](https://github.com/lukstei/slop-grader) | A, D, E | screened; no strong match for this scope |
| [lykycy123/RoboJEV](https://github.com/lykycy123/RoboJEV) | E | screened; no strong match for this scope |
| [lyramakesmusic/jevbot](https://github.com/lyramakesmusic/jevbot) | E | screened; no strong match for this scope |
| [m0rphtail/triagedy](https://github.com/m0rphtail/triagedy) | E | screened; no strong match for this scope |
| [maayanlevy/mysql-ailike](https://github.com/maayanlevy/mysql-ailike) | E | screened; no strong match for this scope |
| [MachineLearning-Nerd/jev-tetris](https://github.com/MachineLearning-Nerd/jev-tetris) | E | screened; no strong match for this scope |
| [madeye/pi-jev](https://github.com/madeye/pi-jev) | E | screened; no strong match for this scope |
| [mahlernim/jev-korean-benchmark](https://github.com/mahlernim/jev-korean-benchmark) | D, E | screened; no strong match for this scope |
| [MahmoudAdelbghany/jev-browser](https://github.com/MahmoudAdelbghany/jev-browser) | E | screened; no strong match for this scope |
| [mahynotch/newsscore](https://github.com/mahynotch/newsscore) | E | screened; no strong match for this scope |
| [maker-KK/todo-jev](https://github.com/maker-KK/todo-jev) | E | screened; no strong match for this scope |
| [malDuffin/typesafe-3d-chess](https://github.com/malDuffin/typesafe-3d-chess) | E | screened; no strong match for this scope |
| [malevrigns/agent-jev](https://github.com/malevrigns/agent-jev) | E | screened; no strong match for this scope |
| [mameli/jev-vs-luna](https://github.com/mameli/jev-vs-luna) | E | screened; no strong match for this scope |
| [Manavarya09/verdict](https://github.com/Manavarya09/verdict) | E | screened; no strong match for this scope |
| [Mandrilsquad1441/jev-model-router](https://github.com/Mandrilsquad1441/jev-model-router) | C | screened; no strong match for this scope |
| [manifoldor/xtags](https://github.com/manifoldor/xtags) | E | screened lead; requires follow-up |
| [Mapika/decider](https://github.com/Mapika/decider) | A, C, E | screened; no strong match for this scope |
| [marcus/frost](https://github.com/marcus/frost) | E | screened; no strong match for this scope |
| [MarissaFamularo/citation-verifier](https://github.com/MarissaFamularo/citation-verifier) | A, E | relevant README; see discussion |
| [markjaquith/typesafe-ai-playground](https://github.com/markjaquith/typesafe-ai-playground) | E | screened; no strong match for this scope |
| [martinopiaggi/summarize](https://github.com/martinopiaggi/summarize) | E | screened; no strong match for this scope |
| [MartinPuli/f1](https://github.com/MartinPuli/f1) | E | screened; no strong match for this scope |
| [masonlee39/Multi-Agent](https://github.com/masonlee39/Multi-Agent) | E | screened; no strong match for this scope |
| [mateonunez/jod](https://github.com/mateonunez/jod) | E | screened; no strong match for this scope |
| [matrixorigin/Astra](https://github.com/matrixorigin/Astra) | E | screened; no strong match for this scope |
| [matthewp/flue-jev-demo](https://github.com/matthewp/flue-jev-demo) | A, C | screened; no strong match for this scope |
| [mattn/go-jev](https://github.com/mattn/go-jev) | C, E | screened; no strong match for this scope |
| [mattneel/typesafe](https://github.com/mattneel/typesafe) | C, E | screened; no strong match for this scope |
| [mattneel/typesafe.zig](https://github.com/mattneel/typesafe.zig) | C, E | screened; no strong match for this scope |
| [mayank953/Jev](https://github.com/mayank953/Jev) | E | screened; no strong match for this scope |
| [mblode/taste-lint](https://github.com/mblode/taste-lint) | D | screened; no strong match for this scope |
| [mejiasd3v/pi-jev-router](https://github.com/mejiasd3v/pi-jev-router) | A, C, E | screened; no strong match for this scope |
| [memorysaver/jev-atari-lab](https://github.com/memorysaver/jev-atari-lab) | E | screened lead; requires follow-up |
| [Menny1337/jev-lab](https://github.com/Menny1337/jev-lab) | E | screened; no strong match for this scope |
| [mgaitan/sqlite-jev](https://github.com/mgaitan/sqlite-jev) | A | screened; no strong match for this scope |
| [MiaoWuNYA/rikkahub-sillytavern-android](https://github.com/MiaoWuNYA/rikkahub-sillytavern-android) | E | screened; no strong match for this scope |
| [Micha0827/snapjudge](https://github.com/Micha0827/snapjudge) | E | screened; no strong match for this scope |
| [michaelswissa/jevry](https://github.com/michaelswissa/jevry) | E | screened; no strong match for this scope |
| [MichelKerkmeester/skilled-agent-harness_spec-driven-loops](https://github.com/MichelKerkmeester/skilled-agent-harness_spec-driven-loops) | E | screened; no strong match for this scope |
| [mikecann/magic-jev-ball](https://github.com/mikecann/magic-jev-ball) | D, E | screened; no strong match for this scope |
| [milanboers/jev-plays-pokemon](https://github.com/milanboers/jev-plays-pokemon) | A, E | screened; no strong match for this scope |
| [milvus-io/bootcamp](https://github.com/milvus-io/bootcamp) | D, E | screened; no strong match for this scope |
| [milvus-io/milvus-model](https://github.com/milvus-io/milvus-model) | D, E | screened; no strong match for this scope |
| [miniLV/Jev-Auto-Router](https://github.com/miniLV/Jev-Auto-Router) | D | screened; no strong match for this scope |
| [misbahsy/doc-router](https://github.com/misbahsy/doc-router) | E | screened; no strong match for this scope |
| [mithalouni/system-one-open](https://github.com/mithalouni/system-one-open) | E | screened; no strong match for this scope |
| [miuuyy/Astra-Ares](https://github.com/miuuyy/Astra-Ares) | E | screened; no strong match for this scope |
| [mizchi/jev-gomoku](https://github.com/mizchi/jev-gomoku) | E | screened; no strong match for this scope |
| [mizchi/jev-lint](https://github.com/mizchi/jev-lint) | E | screened; no strong match for this scope |
| [mizchi/jev-test-filter](https://github.com/mizchi/jev-test-filter) | E | screened; no strong match for this scope |
| [mizzlelover/jev-hub](https://github.com/mizzlelover/jev-hub) | E | screened; no strong match for this scope |
| [mkotlikov/jev-grug](https://github.com/mkotlikov/jev-grug) | E | screened; no strong match for this scope |
| [mmastrac/djev](https://github.com/mmastrac/djev) | E | screened; no strong match for this scope |
| [mmastrac/djev-spark](https://github.com/mmastrac/djev-spark) | E | screened; no strong match for this scope |
| [mocchalera/naimono-lab](https://github.com/mocchalera/naimono-lab) | E | screened; no strong match for this scope |
| [mode-io/vllm-jev](https://github.com/mode-io/vllm-jev) | C, E | screened; no strong match for this scope |
| [moezubair/check-risk](https://github.com/moezubair/check-risk) | E | screened; no strong match for this scope |
| [MohibShaikh/jev-skillbench](https://github.com/MohibShaikh/jev-skillbench) | E | screened; no strong match for this scope |
| [mohsen1/llm-debugger-vscode-extension](https://github.com/mohsen1/llm-debugger-vscode-extension) | E | screened; no strong match for this scope |
| [MoLeMo-Lab/mojev](https://github.com/MoLeMo-Lab/mojev) | E | screened; no strong match for this scope |
| [molis-ai/jev-workbench](https://github.com/molis-ai/jev-workbench) | E | screened; no strong match for this scope |
| [MongLong0214/jev-gate](https://github.com/MongLong0214/jev-gate) | E | screened; no strong match for this scope |
| [monteduro/killmyidea](https://github.com/monteduro/killmyidea) | D, E | screened; no strong match for this scope |
| [morganlinton/Albatross](https://github.com/morganlinton/Albatross) | E | screened; no strong match for this scope |
| [moritzkremb/jev-voice-browser](https://github.com/moritzkremb/jev-voice-browser) | C, D, E | screened; no strong match for this scope |
| [MorrisZJ/AnyJev](https://github.com/MorrisZJ/AnyJev) | D | screened; no strong match for this scope |
| [mossyfield/ST-jeved](https://github.com/mossyfield/ST-jeved) | E | screened; no strong match for this scope |
| [MrJev/awesome-jev](https://github.com/MrJev/awesome-jev) | D, E | collection / discovery source; README screened |
| [mrnugget/jev-shell-history](https://github.com/mrnugget/jev-shell-history) | C, D, E | screened; no strong match for this scope |
| [myc0576/SmartMoney-Cub](https://github.com/myc0576/SmartMoney-Cub) | E | screened; no strong match for this scope |
| [myokoym/misereru-slide-jev](https://github.com/myokoym/misereru-slide-jev) | E | screened; no strong match for this scope |
| [n-yokomachi/jev-dev](https://github.com/n-yokomachi/jev-dev) | E | screened; no strong match for this scope |
| [n3ndor/n8n-nodes-typesafe-jev](https://github.com/n3ndor/n8n-nodes-typesafe-jev) | D | screened; no strong match for this scope |
| [Nainish-Rai/jev-frontend-qa](https://github.com/Nainish-Rai/jev-frontend-qa) | E | screened; no strong match for this scope |
| [NandhaKishorM/laya](https://github.com/NandhaKishorM/laya) | A, E | screened; no strong match for this scope |
| [NanmiCoder/jev-arena](https://github.com/NanmiCoder/jev-arena) | E | screened; no strong match for this scope |
| [narulaskaran/jev-data-questions](https://github.com/narulaskaran/jev-data-questions) | E | screened; no strong match for this scope |
| [Nasrallah-AL/jev-cli](https://github.com/Nasrallah-AL/jev-cli) | E | screened; no strong match for this scope |
| [nassim-arifette/jevgrep](https://github.com/nassim-arifette/jevgrep) | E | screened; no strong match for this scope |
| [ndolinschi/cartshield](https://github.com/ndolinschi/cartshield) | E | screened; no strong match for this scope |
| [ndolinschi/harnessjudge](https://github.com/ndolinschi/harnessjudge) | E | screened; no strong match for this scope |
| [ndolinschi/hiresignal](https://github.com/ndolinschi/hiresignal) | E | screened; no strong match for this scope |
| [ndolinschi/jevplay](https://github.com/ndolinschi/jevplay) | E | screened; no strong match for this scope |
| [ndolinschi/lanebreak](https://github.com/ndolinschi/lanebreak) | E | screened; no strong match for this scope |
| [ndolinschi/mcpmatch](https://github.com/ndolinschi/mcpmatch) | E | screened; no strong match for this scope |
| [ndolinschi/pulselane](https://github.com/ndolinschi/pulselane) | E | screened; no strong match for this scope |
| [ndolinschi/spendbrake](https://github.com/ndolinschi/spendbrake) | E | screened; no strong match for this scope |
| [ndolinschi/swarmrouter](https://github.com/ndolinschi/swarmrouter) | E | screened; no strong match for this scope |
| [ndolinschi/toolgate](https://github.com/ndolinschi/toolgate) | E | screened; no strong match for this scope |
| [ndolinschi/trustgate](https://github.com/ndolinschi/trustgate) | E | screened; no strong match for this scope |
| [neddes/sloppy-jevs-extension](https://github.com/neddes/sloppy-jevs-extension) | E | screened; no strong match for this scope |
| [nekuda-ai/WindTunnel](https://github.com/nekuda-ai/WindTunnel) | A, E | screened; no strong match for this scope |
| [NevaMind-AI/JevTown](https://github.com/NevaMind-AI/JevTown) | E | screened; no strong match for this scope |
| [newfull5/malkuth](https://github.com/newfull5/malkuth) | E | screened; no strong match for this scope |
| [nexibeo/jev-browser-control](https://github.com/nexibeo/jev-browser-control) | B | screened; no strong match for this scope |
| [nexibeo/jev-cookbook](https://github.com/nexibeo/jev-cookbook) | A, B, C, D, E | source inspected; directly relevant |
| [NiazMorshed2007/jcr](https://github.com/NiazMorshed2007/jcr) | E | screened; no strong match for this scope |
| [NiazMorshed2007/jev-review](https://github.com/NiazMorshed2007/jev-review) | D, E | screened; no strong match for this scope |
| [nican2018/shade-arena-jev-monitor](https://github.com/nican2018/shade-arena-jev-monitor) | E | screened; no strong match for this scope |
| [nickthompson480/typesafe-ai-playground](https://github.com/nickthompson480/typesafe-ai-playground) | E | screened; no strong match for this scope |
| [nico-martin/open-jev](https://github.com/nico-martin/open-jev) | E | screened; no strong match for this scope |
| [nicobailon/surf-cli](https://github.com/nicobailon/surf-cli) | E | screened; no strong match for this scope |
| [NicolaiLassen/open-bonsai-jev](https://github.com/NicolaiLassen/open-bonsai-jev) | D | screened; no strong match for this scope |
| [nidhi-singh02/agent-router](https://github.com/nidhi-singh02/agent-router) | D, E | screened; no strong match for this scope |
| [nikkoxgonzales/jev-certify](https://github.com/nikkoxgonzales/jev-certify) | E | screened; no strong match for this scope |
| [nirgal-soft/typesafe-rs](https://github.com/nirgal-soft/typesafe-rs) | C | screened; no strong match for this scope |
| [Nisaka520/JevIntent](https://github.com/Nisaka520/JevIntent) | E | screened; no strong match for this scope |
| [NobleSpartan6/otto](https://github.com/NobleSpartan6/otto) | E | screened; no strong match for this scope |
| [noelzappy/tripwire](https://github.com/noelzappy/tripwire) | E | screened; no strong match for this scope |
| [noetion/dsh-jev](https://github.com/noetion/dsh-jev) | C, E | screened; no strong match for this scope |
| [nokia-applied-research/AnyJev](https://github.com/nokia-applied-research/AnyJev) | E | screened lead; requires follow-up |
| [noperator/siftrank](https://github.com/noperator/siftrank) | E | screened; no strong match for this scope |
| [noplan-inc/limpet](https://github.com/noplan-inc/limpet) | A, E | screened; no strong match for this scope |
| [NorbertBodziony/guard-jev](https://github.com/NorbertBodziony/guard-jev) | E | screened; no strong match for this scope |
| [notque/vexjoy-agent](https://github.com/notque/vexjoy-agent) | E | screened; no strong match for this scope |
| [nozomi-koborinai/jev-spec](https://github.com/nozomi-koborinai/jev-spec) | E | screened; no strong match for this scope |
| [nshkrdotcom/typesafe_sdk](https://github.com/nshkrdotcom/typesafe_sdk) | E | screened; no strong match for this scope |
| [ntedvs/commentcop](https://github.com/ntedvs/commentcop) | E | screened; no strong match for this scope |
| [NullPo-jp/PocketJev](https://github.com/NullPo-jp/PocketJev) | E | screened; no strong match for this scope |
| [Nyarlathoteppppp/pi-heed](https://github.com/Nyarlathoteppppp/pi-heed) | A, E | screened; no strong match for this scope |
| [Nyarlathoteppppp/pi-jev-context](https://github.com/Nyarlathoteppppp/pi-jev-context) | E | screened; no strong match for this scope |
| [obie/ruby_decision_model](https://github.com/obie/ruby_decision_model) | A, C, D, E | screened; no strong match for this scope |
| [oceanByte/tsai-cli](https://github.com/oceanByte/tsai-cli) | C | screened; no strong match for this scope |
| [odin-labs-ai/odin-rnd](https://github.com/odin-labs-ai/odin-rnd) | A | screened; no strong match for this scope |
| [ojusave/beat-jev](https://github.com/ojusave/beat-jev) | E | screened; no strong match for this scope |
| [okinaaudio/live-jev](https://github.com/okinaaudio/live-jev) | E | screened; no strong match for this scope |
| [olanotolu/jevbetter](https://github.com/olanotolu/jevbetter) | A, E | screened; no strong match for this scope |
| [ollaya-dev/ollaya](https://github.com/ollaya-dev/ollaya) | D, E | screened; no strong match for this scope |
| [Olli0103/openclaw-typesafe-ai](https://github.com/Olli0103/openclaw-typesafe-ai) | C, E | screened; no strong match for this scope |
| [Olti1947/jev-java](https://github.com/Olti1947/jev-java) | C, E | screened; no strong match for this scope |
| [omni-/ask-jev](https://github.com/omni-/ask-jev) | E | screened; no strong match for this scope |
| [OmniJev/awesome-jev](https://github.com/OmniJev/awesome-jev) | D, E | collection / discovery source; README screened |
| [OmniJev/awesome-jev-gallery](https://github.com/OmniJev/awesome-jev-gallery) | E | collection / discovery source; README screened |
| [OmniJev/OneJev](https://github.com/OmniJev/OneJev) | A, E | screened; no strong match for this scope |
| [OmniJev/PlayJev](https://github.com/OmniJev/PlayJev) | A, D, E | screened; no strong match for this scope |
| [onionminionops-beep/pdoom-protocol](https://github.com/onionminionops-beep/pdoom-protocol) | E | screened; no strong match for this scope |
| [onlyoneaman/jev-eval](https://github.com/onlyoneaman/jev-eval) | E | screened; no strong match for this scope |
| [opaielsheikh/ai-elo-ranker](https://github.com/opaielsheikh/ai-elo-ranker) | E | screened; no strong match for this scope |
| [opaielsheikh/ps2-ai-agent](https://github.com/opaielsheikh/ps2-ai-agent) | E | screened; no strong match for this scope |
| [opaielsheikh/typesafe-migration-guard](https://github.com/opaielsheikh/typesafe-migration-guard) | E | screened; no strong match for this scope |
| [OpenByteInc/QuantDinger](https://github.com/OpenByteInc/QuantDinger) | E | screened; no strong match for this scope |
| [opencx-labs/zevals](https://github.com/opencx-labs/zevals) | E | screened; no strong match for this scope |
| [openlayer-ai/jevals](https://github.com/openlayer-ai/jevals) | E | screened; no strong match for this scope |
| [openqa-cn/jev-browser](https://github.com/openqa-cn/jev-browser) | E | screened; no strong match for this scope |
| [openroboto-ai/jev-robot-control](https://github.com/openroboto-ai/jev-robot-control) | E | screened; no strong match for this scope |
| [OpenRouterTeam/ai-sdk-provider](https://github.com/OpenRouterTeam/ai-sdk-provider) | E | screened; no strong match for this scope |
| [oso95/x-scanner](https://github.com/oso95/x-scanner) | E | screened; no strong match for this scope |
| [Paca-AI/paca](https://github.com/Paca-AI/paca) | E | screened; no strong match for this scope |
| [PanAchy/jevvy](https://github.com/PanAchy/jevvy) | E | screened; no strong match for this scope |
| [Para-FR/casse-brique-typesafe](https://github.com/Para-FR/casse-brique-typesafe) | E | screened; no strong match for this scope |
| [parkavenue9639/jevloop](https://github.com/parkavenue9639/jevloop) | D | screened; no strong match for this scope |
| [parth-kp/jev-mail-classifier](https://github.com/parth-kp/jev-mail-classifier) | E | screened; no strong match for this scope |
| [patryckalves/jev-no-enem](https://github.com/patryckalves/jev-no-enem) | E | screened; no strong match for this scope |
| [paulsmith/computer-use-jev](https://github.com/paulsmith/computer-use-jev) | E | screened; no strong match for this scope |
| [pc418/jev-calculator](https://github.com/pc418/jev-calculator) | E | screened; no strong match for this scope |
| [pedrocivita/tocket](https://github.com/pedrocivita/tocket) | E | screened; no strong match for this scope |
| [pekth/draftpulse](https://github.com/pekth/draftpulse) | E | screened; no strong match for this scope |
| [pengchujin/ad-radar](https://github.com/pengchujin/ad-radar) | E | screened; no strong match for this scope |
| [PerryLink/jevcore](https://github.com/PerryLink/jevcore) | C, E | screened; no strong match for this scope |
| [peterfriese/jev-foundation-models](https://github.com/peterfriese/jev-foundation-models) | E | screened; no strong match for this scope |
| [peternguyen777/ai-icon-generator](https://github.com/peternguyen777/ai-icon-generator) | C | screened; no strong match for this scope |
| [philipbrembeck/pi-advisor](https://github.com/philipbrembeck/pi-advisor) | E | screened; no strong match for this scope |
| [philippdubach/pi-jev-router](https://github.com/philippdubach/pi-jev-router) | E | screened; no strong match for this scope |
| [phureewat29/got-jev](https://github.com/phureewat29/got-jev) | E | screened; no strong match for this scope |
| [phyous/tsai-civ2](https://github.com/phyous/tsai-civ2) | E | screened; no strong match for this scope |
| [phyous/tsai-sc](https://github.com/phyous/tsai-sc) | A, C, D, E | screened; no strong match for this scope |
| [picaye/jev-compaction](https://github.com/picaye/jev-compaction) | E | screened lead; requires follow-up |
| [pinkroosterai/SystemOneSharp](https://github.com/pinkroosterai/SystemOneSharp) | E | screened; no strong match for this scope |
| [Pinutss/jev-plugins](https://github.com/Pinutss/jev-plugins) | E | screened; no strong match for this scope |
| [PistachioAIHQ/jev-synergy-screening](https://github.com/PistachioAIHQ/jev-synergy-screening) | E | screened; no strong match for this scope |
| [pithings/advocaat](https://github.com/pithings/advocaat) | A, C, D, E | screened; no strong match for this scope |
| [prasanthj/duckdb-jev](https://github.com/prasanthj/duckdb-jev) | E | screened; no strong match for this scope |
| [Premo-Cloud/typesafe-sdk-java](https://github.com/Premo-Cloud/typesafe-sdk-java) | D, E | screened; no strong match for this scope |
| [prismhq/jev-router](https://github.com/prismhq/jev-router) | A, C, E | screened; no strong match for this scope |
| [Promethe-us/awesome-jev](https://github.com/Promethe-us/awesome-jev) | E | collection / discovery source; README screened |
| [PromptEngineer48/laya-vs-jev-arena](https://github.com/PromptEngineer48/laya-vs-jev-arena) | C, E | screened; no strong match for this scope |
| [pst2154/Nemotron_Jev](https://github.com/pst2154/Nemotron_Jev) | E | screened; no strong match for this scope |
| [punk2898/awesome-jev-verified](https://github.com/punk2898/awesome-jev-verified) | E | collection / discovery source; README screened |
| [pydantic/pydantic-ai](https://github.com/pydantic/pydantic-ai) | D | screened; no strong match for this scope |
| [PyModel/jev-judge-mcp](https://github.com/PyModel/jev-judge-mcp) | E | screened; no strong match for this scope |
| [pZacca/askjev](https://github.com/pZacca/askjev) | E | screened; no strong match for this scope |
| [QAInsights/jmeter-ai](https://github.com/QAInsights/jmeter-ai) | E | screened; no strong match for this scope |
| [qddegtya/qualm](https://github.com/qddegtya/qualm) | E | screened; no strong match for this scope |
| [Qew7/jev-feels](https://github.com/Qew7/jev-feels) | E | screened; no strong match for this scope |
| [qkal/Canny](https://github.com/qkal/Canny) | D, E | screened; no strong match for this scope |
| [quantskills/QuantStudio](https://github.com/quantskills/QuantStudio) | E | screened; no strong match for this scope |
| [QuentinDanblon/pi-fast-jev-compaction](https://github.com/QuentinDanblon/pi-fast-jev-compaction) | E | screened lead; requires follow-up |
| [Query-farm/vgi-typesafe](https://github.com/Query-farm/vgi-typesafe) | E | screened; no strong match for this scope |
| [r-ms/mini-jev](https://github.com/r-ms/mini-jev) | A, E | screened; no strong match for this scope |
| [r33drichards/laya-vision](https://github.com/r33drichards/laya-vision) | D | screened; no strong match for this scope |
| [RafalWilinski/vibecheck](https://github.com/RafalWilinski/vibecheck) | C, D, E | screened; no strong match for this scope |
| [raihankhan-rk/diffjury](https://github.com/raihankhan-rk/diffjury) | A, C, E | screened; no strong match for this scope |
| [raihankhan-rk/jevarena](https://github.com/raihankhan-rk/jevarena) | C, E | screened; no strong match for this scope |
| [rainbowpuffpuff/jev-builder-loop](https://github.com/rainbowpuffpuff/jev-builder-loop) | E | screened; no strong match for this scope |
| [rajdhakad9826/jev-router](https://github.com/rajdhakad9826/jev-router) | C, E | screened; no strong match for this scope |
| [rajivkuriakose/typesafe-jev-examples](https://github.com/rajivkuriakose/typesafe-jev-examples) | A, E | screened; no strong match for this scope |
| [ranjan2829/AskJev](https://github.com/ranjan2829/AskJev) | E | screened; no strong match for this scope |
| [rashedInt32/jev-mcp](https://github.com/rashedInt32/jev-mcp) | C, E | screened; no strong match for this scope |
| [Ravinder82/jev-flash-router](https://github.com/Ravinder82/jev-flash-router) | E | screened; no strong match for this scope |
| [Ray-Hughes/jevalyn](https://github.com/Ray-Hughes/jevalyn) | E | screened; no strong match for this scope |
| [razaanstha/ulka](https://github.com/razaanstha/ulka) | E | screened; no strong match for this scope |
| [razorback16/openjev](https://github.com/razorback16/openjev) | C, D, E | screened; no strong match for this scope |
| [rbalch/typesafeai-review](https://github.com/rbalch/typesafeai-review) | E | screened; no strong match for this scope |
| [rchovatiya88/cyber-breach-jev](https://github.com/rchovatiya88/cyber-breach-jev) | E | screened; no strong match for this scope |
| [reachjalil/jev-tree](https://github.com/reachjalil/jev-tree) | D, E | screened; no strong match for this scope |
| [reachjalil/jevlogs](https://github.com/reachjalil/jevlogs) | D, E | screened; no strong match for this scope |
| [ReallyArtificial/jev-by-example](https://github.com/ReallyArtificial/jev-by-example) | C, D, E | screened; no strong match for this scope |
| [ReallyArtificial/stuntdouble](https://github.com/ReallyArtificial/stuntdouble) | C, E | screened; no strong match for this scope |
| [realZachi/pg-jev](https://github.com/realZachi/pg-jev) | C, D, E | screened; no strong match for this scope |
| [realZachi/typesafe-adblock](https://github.com/realZachi/typesafe-adblock) | A, C, E | screened; no strong match for this scope |
| [receptron/laya](https://github.com/receptron/laya) | E | screened; no strong match for this scope |
| [reinhard-z/vision-jev](https://github.com/reinhard-z/vision-jev) | E | screened; no strong match for this scope |
| [RenaGao/jev-dataops](https://github.com/RenaGao/jev-dataops) | E | screened; no strong match for this scope |
| [renatosousa/jev-trader](https://github.com/renatosousa/jev-trader) | E | screened; no strong match for this scope |
| [replynodes/jev-web-analyzer](https://github.com/replynodes/jev-web-analyzer) | D | screened; no strong match for this scope |
| [reticlehq/reticle](https://github.com/reticlehq/reticle) | E | screened; no strong match for this scope |
| [reycn/smart-switch](https://github.com/reycn/smart-switch) | E | screened; no strong match for this scope |
| [Reza2kn/Bev](https://github.com/Reza2kn/Bev) | E | screened; no strong match for this scope |
| [rhc98/awesome-jev](https://github.com/rhc98/awesome-jev) | E | collection / discovery source; README screened |
| [RINNECODER/jev-behavior-study](https://github.com/RINNECODER/jev-behavior-study) | D, E | screened; no strong match for this scope |
| [Ripwords/agent-gate-loop](https://github.com/Ripwords/agent-gate-loop) | E | screened; no strong match for this scope |
| [Rizzo-AI-Academy/rizzo-flow](https://github.com/Rizzo-AI-Academy/rizzo-flow) | E | screened; no strong match for this scope |
| [rlaope/jeval](https://github.com/rlaope/jeval) | E | screened; no strong match for this scope |
| [rohanarun/computer-use-cache](https://github.com/rohanarun/computer-use-cache) | E | screened; no strong match for this scope |
| [rokbenko/quackd](https://github.com/rokbenko/quackd) | E | screened; no strong match for this scope |
| [rolki-png/JevArena](https://github.com/rolki-png/JevArena) | E | screened; no strong match for this scope |
| [romaluev/jev-ego](https://github.com/romaluev/jev-ego) | B, E | screened; no strong match for this scope |
| [RomanSlack/jev-drone](https://github.com/RomanSlack/jev-drone) | A, C, D, E | screened; no strong match for this scope |
| [romeromarcelo/jev-retrieval](https://github.com/romeromarcelo/jev-retrieval) | E | source inspected; directly relevant |
| [ronadin2002/jev-cua](https://github.com/ronadin2002/jev-cua) | E | screened; no strong match for this scope |
| [rorshopping/jev-on-a-laptop](https://github.com/rorshopping/jev-on-a-laptop) | E | screened; no strong match for this scope |
| [rsdkrasen/hermes-jev-router](https://github.com/rsdkrasen/hermes-jev-router) | C | screened; no strong match for this scope |
| [ruban-24/switchboard](https://github.com/ruban-24/switchboard) | A, D | screened; no strong match for this scope |
| [rupeshpoojary9/awesome-open-system-one](https://github.com/rupeshpoojary9/awesome-open-system-one) | D | collection / discovery source; README screened |
| [rupeshpoojary9/poorjev](https://github.com/rupeshpoojary9/poorjev) | D, E | screened; no strong match for this scope |
| [RyanKung/rotom](https://github.com/RyanKung/rotom) | A | screened; no strong match for this scope |
| [ryanwaits/secondlayer](https://github.com/ryanwaits/secondlayer) | A, E | screened; no strong match for this scope |
| [sabeel111/OpenSourceJev](https://github.com/sabeel111/OpenSourceJev) | E | screened; no strong match for this scope |
| [Sac-Y/Jev-cu](https://github.com/Sac-Y/Jev-cu) | E | screened; no strong match for this scope |
| [saembit/jeff-cli](https://github.com/saembit/jeff-cli) | D | screened; no strong match for this scope |
| [safzanpirani/pi-jev-skill-picker](https://github.com/safzanpirani/pi-jev-skill-picker) | C, E | screened; no strong match for this scope |
| [safzanpirani/pong-jev](https://github.com/safzanpirani/pong-jev) | E | screened; no strong match for this scope |
| [saibimajdi/typesafeai-dotnet-sdk](https://github.com/saibimajdi/typesafeai-dotnet-sdk) | C, E | screened; no strong match for this scope |
| [samdotmak/jev-recall](https://github.com/samdotmak/jev-recall) | E | relevant README; see discussion |
| [samtay32/jev-system-architect](https://github.com/samtay32/jev-system-architect) | C, E | screened; no strong match for this scope |
| [samuelfaj/distill](https://github.com/samuelfaj/distill) | E | screened; no strong match for this scope |
| [SamuelSacco/jev-exploration](https://github.com/SamuelSacco/jev-exploration) | E | screened; no strong match for this scope |
| [sandeco/pix-golpe](https://github.com/sandeco/pix-golpe) | E | screened; no strong match for this scope |
| [sanmai/typesafe-ai-php](https://github.com/sanmai/typesafe-ai-php) | E | screened; no strong match for this scope |
| [santos-sanz/jev-audio-beeper](https://github.com/santos-sanz/jev-audio-beeper) | E | screened; no strong match for this scope |
| [savka777/jev-use](https://github.com/savka777/jev-use) | E | screened; no strong match for this scope |
| [sc2musa/Jev_Star](https://github.com/sc2musa/Jev_Star) | E | screened; no strong match for this scope |
| [scale-venture-partners/riff](https://github.com/scale-venture-partners/riff) | E | screened; no strong match for this scope |
| [scienthoon/jev-ood-calibration](https://github.com/scienthoon/jev-ood-calibration) | A, E | screened; no strong match for this scope |
| [sdras/jev-webmcp-extension](https://github.com/sdras/jev-webmcp-extension) | E | screened; no strong match for this scope |
| [seb4ez/jevguard-mcp](https://github.com/seb4ez/jevguard-mcp) | D | screened; no strong match for this scope |
| [SeeAPI/awesome-jev-use-cases](https://github.com/SeeAPI/awesome-jev-use-cases) | E | collection / discovery source; README screened |
| [sensahin/transcript-lens](https://github.com/sensahin/transcript-lens) | E | screened; no strong match for this scope |
| [sgoedecke/system-one](https://github.com/sgoedecke/system-one) | E | screened; no strong match for this scope |
| [shaharia-lab/jev-cli](https://github.com/shaharia-lab/jev-cli) | C, E | screened; no strong match for this scope |
| [shamazharikh/qwen-rlcd](https://github.com/shamazharikh/qwen-rlcd) | E | screened; no strong match for this scope |
| [Shanghua-Gao/RSI-Jev](https://github.com/Shanghua-Gao/RSI-Jev) | A | screened; no strong match for this scope |
| [shantanugoel/ask-jev-skill](https://github.com/shantanugoel/ask-jev-skill) | C, E | screened; no strong match for this scope |
| [shantanugoel/jev-games](https://github.com/shantanugoel/jev-games) | E | screened; no strong match for this scope |
| [shantanugoel/mario-jev](https://github.com/shantanugoel/mario-jev) | E | screened; no strong match for this scope |
| [sharziki/semdecide](https://github.com/sharziki/semdecide) | D, E | screened; no strong match for this scope |
| [shhivv/arc-cua](https://github.com/shhivv/arc-cua) | E | screened; no strong match for this scope |
| [shibadogcap/kyotsu-ai-bench](https://github.com/shibadogcap/kyotsu-ai-bench) | E | unavailable README |
| [Shifros/Search-Function-Test](https://github.com/Shifros/Search-Function-Test) | E | unavailable README |
| [shiftynick/jev-axi](https://github.com/shiftynick/jev-axi) | A, D, E | screened; no strong match for this scope |
| [shimo4228/jev-research-pipeline](https://github.com/shimo4228/jev-research-pipeline) | D | screened lead; requires follow-up |
| [shimo4228/jev-skill-router](https://github.com/shimo4228/jev-skill-router) | D, E | screened; no strong match for this scope |
| [shinpr/jev-reranker](https://github.com/shinpr/jev-reranker) | D | relevant README; see discussion |
| [shitianfang/jev-use](https://github.com/shitianfang/jev-use) | A, D, E | screened; no strong match for this scope |
| [shitianfang/wakegate](https://github.com/shitianfang/wakegate) | A, D | screened; no strong match for this scope |
| [shivam2003-dev/typesafe-triage-guard](https://github.com/shivam2003-dev/typesafe-triage-guard) | E | screened; no strong match for this scope |
| [ShivamPansuriya/jev-skill-gate](https://github.com/ShivamPansuriya/jev-skill-gate) | E | screened; no strong match for this scope |
| [Shogo-nfrealmusic/jev-eval](https://github.com/Shogo-nfrealmusic/jev-eval) | A | screened; no strong match for this scope |
| [Shubham510/typesafe-go](https://github.com/Shubham510/typesafe-go) | C, E | screened; no strong match for this scope |
| [ShuhanSun/jev-oas-sentinel](https://github.com/ShuhanSun/jev-oas-sentinel) | E | screened; no strong match for this scope |
| [ShupingR/scam-shield](https://github.com/ShupingR/scam-shield) | E | screened; no strong match for this scope |
| [siddicky/omp-typesafe](https://github.com/siddicky/omp-typesafe) | C, E | screened; no strong match for this scope |
| [sightmap/jev-turbo](https://github.com/sightmap/jev-turbo) | E | screened; no strong match for this scope |
| [Silbercue/public-browser](https://github.com/Silbercue/public-browser) | A | screened; no strong match for this scope |
| [sileod/tasksource](https://github.com/sileod/tasksource) | E | screened; no strong match for this scope |
| [SiliconLabAI/OpenJev](https://github.com/SiliconLabAI/OpenJev) | E | screened; no strong match for this scope |
| [silverstein/minutes](https://github.com/silverstein/minutes) | A | screened; no strong match for this scope |
| [simota/tenbin](https://github.com/simota/tenbin) | C | screened; no strong match for this scope |
| [simxnherrera/jevr](https://github.com/simxnherrera/jevr) | D | screened; no strong match for this scope |
| [siroccomask/snake-jev](https://github.com/siroccomask/snake-jev) | E | screened; no strong match for this scope |
| [skcache/jevtrafficsim](https://github.com/skcache/jevtrafficsim) | D | screened; no strong match for this scope |
| [skeptrunedev/jev-recruiter](https://github.com/skeptrunedev/jev-recruiter) | E | screened; no strong match for this scope |
| [Skyvern-AI/jevscape](https://github.com/Skyvern-AI/jevscape) | D | screened; no strong match for this scope |
| [smasato/jev-jp-address](https://github.com/smasato/jev-jp-address) | E | screened; no strong match for this scope |
| [smithersai/smithers](https://github.com/smithersai/smithers) | A, E | screened; no strong match for this scope |
| [smkrv/jev-calibrate](https://github.com/smkrv/jev-calibrate) | E | screened; no strong match for this scope |
| [socai-io/jev-social](https://github.com/socai-io/jev-social) | A, C, D, E | screened; no strong match for this scope |
| [sonnylazuardi/superterminal](https://github.com/sonnylazuardi/superterminal) | E | screened; no strong match for this scope |
| [sontakey/awesome-jev](https://github.com/sontakey/awesome-jev) | C, E | collection / discovery source; README screened |
| [sorrycc/typesafe-snake](https://github.com/sorrycc/typesafe-snake) | D, E | screened; no strong match for this scope |
| [sosopop/jev_stock](https://github.com/sosopop/jev_stock) | A, E | screened; no strong match for this scope |
| [SoundBlaster/Jev4Mellea](https://github.com/SoundBlaster/Jev4Mellea) | E | screened; no strong match for this scope |
| [spoonnotfound/soupbase](https://github.com/spoonnotfound/soupbase) | D | screened; no strong match for this scope |
| [spring-ai-community/spring-ai-typesafe](https://github.com/spring-ai-community/spring-ai-typesafe) | C, E | screened; no strong match for this scope |
| [Spykoninho/trading-bot-jev](https://github.com/Spykoninho/trading-bot-jev) | E | screened; no strong match for this scope |
| [sqliteai/blink](https://github.com/sqliteai/blink) | E | screened; no strong match for this scope |
| [SREGym/SREGym](https://github.com/SREGym/SREGym) | E | screened; no strong match for this scope |
| [sriganesh/jevibe-check](https://github.com/sriganesh/jevibe-check) | E | screened; no strong match for this scope |
| [standardagents/jevpilot](https://github.com/standardagents/jevpilot) | D | screened; no strong match for this scope |
| [stefw/lkclean](https://github.com/stefw/lkclean) | E | screened; no strong match for this scope |
| [stephanj/parallelConstraintDecoding](https://github.com/stephanj/parallelConstraintDecoding) | D | screened; no strong match for this scope |
| [steven-shoemaker/hunch](https://github.com/steven-shoemaker/hunch) | A, D, E | screened; no strong match for this scope |
| [steven-shoemaker/hunch-js](https://github.com/steven-shoemaker/hunch-js) | A, D, E | screened; no strong match for this scope |
| [STRML/omp-jevens-classifier](https://github.com/STRML/omp-jevens-classifier) | E | screened; no strong match for this scope |
| [Stumble/jev-go](https://github.com/Stumble/jev-go) | A, C | screened; no strong match for this scope |
| [suenot/codex-jev-router](https://github.com/suenot/codex-jev-router) | A, C, D, E | screened; no strong match for this scope |
| [sufianetaouil/every](https://github.com/sufianetaouil/every) | E | screened; no strong match for this scope |
| [sugarforever/tryjev](https://github.com/sugarforever/tryjev) | E | screened; no strong match for this scope |
| [sumleo/prompt2jev](https://github.com/sumleo/prompt2jev) | E | screened; no strong match for this scope |
| [superagents-lab/jev-search](https://github.com/superagents-lab/jev-search) | A, C, D, E | screened lead; requires follow-up |
| [supercorp-ai/supercov](https://github.com/supercorp-ai/supercov) | A, D, E | screened; no strong match for this scope |
| [sutro-sh/jev-align](https://github.com/sutro-sh/jev-align) | D, E | screened; no strong match for this scope |
| [sypherin/jev-trace-classifier](https://github.com/sypherin/jev-trace-classifier) | E | screened; no strong match for this scope |
| [syumai/jevyoumean](https://github.com/syumai/jevyoumean) | E | screened; no strong match for this scope |
| [szczyglis-dev/py-gpt](https://github.com/szczyglis-dev/py-gpt) | E | screened; no strong match for this scope |
| [T-moz/typesafe-ai-dart](https://github.com/T-moz/typesafe-ai-dart) | C | unavailable README |
| [tacticocc/Jevbridge](https://github.com/tacticocc/Jevbridge) | A, C, E | screened; no strong match for this scope |
| [takeshy/obsidian-llm-hub](https://github.com/takeshy/obsidian-llm-hub) | E | screened; no strong match for this scope |
| [tamaratran/fast-jev-compaction](https://github.com/tamaratran/fast-jev-compaction) | A, C, D, E | screened lead; requires follow-up |
| [tamaratran/jev-pruner](https://github.com/tamaratran/jev-pruner) | A, C, E | screened; no strong match for this scope |
| [TanayPadar/gpt-vs-jev](https://github.com/TanayPadar/gpt-vs-jev) | E | screened; no strong match for this scope |
| [Tangerg/typesafe-sdk-go](https://github.com/Tangerg/typesafe-sdk-go) | C, E | screened; no strong match for this scope |
| [TanStack/ai](https://github.com/TanStack/ai) | D | screened; no strong match for this scope |
| [tanxarx/awesome-jev](https://github.com/tanxarx/awesome-jev) | E | collection / discovery source; README screened |
| [TarunTomar122/jev-askable-arm](https://github.com/TarunTomar122/jev-askable-arm) | E | screened; no strong match for this scope |
| [TauricResearch/TradingAgents](https://github.com/TauricResearch/TradingAgents) | E | screened lead; requires follow-up |
| [tedliou/decision-model-playground](https://github.com/tedliou/decision-model-playground) | C | screened; no strong match for this scope |
| [teyhouse/jev-secret-detection](https://github.com/teyhouse/jev-secret-detection) | E | screened; no strong match for this scope |
| [TheEleventhAvatar/triage-bot](https://github.com/TheEleventhAvatar/triage-bot) | C | screened; no strong match for this scope |
| [TheGali/terrarium](https://github.com/TheGali/terrarium) | E | screened; no strong match for this scope |
| [thelau/jev-tetris](https://github.com/thelau/jev-tetris) | D | screened; no strong match for this scope |
| [TheoLeeCJ/SemIf](https://github.com/TheoLeeCJ/SemIf) | C, D, E | screened; no strong match for this scope |
| [TheoLeeCJ/SemIf-OpenJev](https://github.com/TheoLeeCJ/SemIf-OpenJev) | E | screened; no strong match for this scope |
| [TheoOliveira/pi-jev](https://github.com/TheoOliveira/pi-jev) | A, C, E | screened; no strong match for this scope |
| [ThePatriczek/playwright-recast](https://github.com/ThePatriczek/playwright-recast) | E | screened; no strong match for this scope |
| [theSekyi/jevusecases](https://github.com/theSekyi/jevusecases) | E | screened; no strong match for this scope |
| [thezem/jev-one](https://github.com/thezem/jev-one) | C | screened; no strong match for this scope |
| [thiago-ss/jev-review](https://github.com/thiago-ss/jev-review) | E | screened; no strong match for this scope |
| [ThinkFlowLab/system1-agents](https://github.com/ThinkFlowLab/system1-agents) | E | screened; no strong match for this scope |
| [thodoh1/FinancialPredictionJev](https://github.com/thodoh1/FinancialPredictionJev) | E | unavailable README |
| [TholeG/typesafe-chess](https://github.com/TholeG/typesafe-chess) | E | screened; no strong match for this scope |
| [thruwire/foreman](https://github.com/thruwire/foreman) | A, C, D, E | screened; no strong match for this scope |
| [TianyuCodings/NanoJev](https://github.com/TianyuCodings/NanoJev) | A, D, E | screened; no strong match for this scope |
| [ticofab/scala-jev-sdk](https://github.com/ticofab/scala-jev-sdk) | E | screened; no strong match for this scope |
| [timrogers/formanator](https://github.com/timrogers/formanator) | E | screened; no strong match for this scope |
| [tinyhumansai/tinyjevclient](https://github.com/tinyhumansai/tinyjevclient) | E | screened; no strong match for this scope |
| [TKY-27/JevSlop](https://github.com/TKY-27/JevSlop) | A, E | screened; no strong match for this scope |
| [togethercomputer/tev1](https://github.com/togethercomputer/tev1) | E | screened; no strong match for this scope |
| [TokenTrim/jev-agent-failure-benchmark](https://github.com/TokenTrim/jev-agent-failure-benchmark) | C, E | screened; no strong match for this scope |
| [TokenTrim/jev-routing-experiment](https://github.com/TokenTrim/jev-routing-experiment) | C, E | screened; no strong match for this scope |
| [tontoko/jev-browser](https://github.com/tontoko/jev-browser) | E | screened; no strong match for this scope |
| [topherchris420/james_library](https://github.com/topherchris420/james_library) | E | screened; no strong match for this scope |
| [Towow-ai/jpp](https://github.com/Towow-ai/jpp) | E | screened; no strong match for this scope |
| [tpaulshippy/shady-town](https://github.com/tpaulshippy/shady-town) | E | screened; no strong match for this scope |
| [TrainLCD/Functions](https://github.com/TrainLCD/Functions) | E | screened; no strong match for this scope |
| [trophee-bot/typesafe-oracles](https://github.com/trophee-bot/typesafe-oracles) | E | unavailable README |
| [trungdq88/youtube-sponsor-detection](https://github.com/trungdq88/youtube-sponsor-detection) | D | screened; no strong match for this scope |
| [tryaksh/jev-pick-and-place-study](https://github.com/tryaksh/jev-pick-and-place-study) | E | screened; no strong match for this scope |
| [trycua/cua](https://github.com/trycua/cua) | E | screened; no strong match for this scope |
| [tshmieldev/sharp](https://github.com/tshmieldev/sharp) | E | screened; no strong match for this scope |
| [tumf/jev-cli](https://github.com/tumf/jev-cli) | A, C, E | screened; no strong match for this scope |
| [twilwa/pi-typesafe](https://github.com/twilwa/pi-typesafe) | C, E | screened; no strong match for this scope |
| [Twister915/typesafe-ai](https://github.com/Twister915/typesafe-ai) | E | screened; no strong match for this scope |
| [tyler-dot-earth/patdown](https://github.com/tyler-dot-earth/patdown) | E | screened; no strong match for this scope |
| [tylergibbs1/sift](https://github.com/tylergibbs1/sift) | E | screened; no strong match for this scope |
| [tylerjharden/ailerix](https://github.com/tylerjharden/ailerix) | E | screened; no strong match for this scope |
| [tylerjharden/harden-jev-decides](https://github.com/tylerjharden/harden-jev-decides) | E | screened; no strong match for this scope |
| [typakon4/jev-layer](https://github.com/typakon4/jev-layer) | E | screened; no strong match for this scope |
| [TypeLLM/pijev](https://github.com/TypeLLM/pijev) | E | screened; no strong match for this scope |
| [TypeLLM/TypeLLM](https://github.com/TypeLLM/TypeLLM) | E | screened; no strong match for this scope |
| [typesafe-ai/daggerverse](https://github.com/typesafe-ai/daggerverse) | E | screened; no strong match for this scope |
| [typesafe-ai/Overwatch](https://github.com/typesafe-ai/Overwatch) | E | unavailable README |
| [typesafe-ai/skills](https://github.com/typesafe-ai/skills) | A, C, D, E | screened; no strong match for this scope |
| [typesafe-ai/system-one-adapter-python](https://github.com/typesafe-ai/system-one-adapter-python) | D, E | screened; no strong match for this scope |
| [typesafe-ai/typesafe-sdk-js](https://github.com/typesafe-ai/typesafe-sdk-js) | D, E | screened; no strong match for this scope |
| [typesafe-ai/typesafe-sdk-python](https://github.com/typesafe-ai/typesafe-sdk-python) | D, E | screened; no strong match for this scope |
| [TypeSafeAI/typesafe-playground](https://github.com/TypeSafeAI/typesafe-playground) | E | screened; no strong match for this scope |
| [typesend/typesafe_ai](https://github.com/typesend/typesafe_ai) | E | screened; no strong match for this scope |
| [UditAkhourii/quicksilver](https://github.com/UditAkhourii/quicksilver) | E | screened; no strong match for this scope |
| [uehaj/jev-semgrep](https://github.com/uehaj/jev-semgrep) | C, E | screened; no strong match for this scope |
| [uezo/aiavatarkit](https://github.com/uezo/aiavatarkit) | E | screened; no strong match for this scope |
| [unicodeveloper/jevocks](https://github.com/unicodeveloper/jevocks) | A, E | screened; no strong match for this scope |
| [us/jev-local](https://github.com/us/jev-local) | A, E | screened; no strong match for this scope |
| [usenotra/notra](https://github.com/usenotra/notra) | A, E | screened; no strong match for this scope |
| [useopencompany/opencompany](https://github.com/useopencompany/opencompany) | A, E | screened; no strong match for this scope |
| [ussyverse/hermes-jev-router](https://github.com/ussyverse/hermes-jev-router) | E | screened; no strong match for this scope |
| [utk2103/jev-studio](https://github.com/utk2103/jev-studio) | E | screened; no strong match for this scope |
| [v-modal/awesome-jev-tools](https://github.com/v-modal/awesome-jev-tools) | E | collection / discovery source; README screened |
| [v60samurai/jev-atlas](https://github.com/v60samurai/jev-atlas) | E | unavailable README |
| [valentynkit/awesome-jev-typesafe](https://github.com/valentynkit/awesome-jev-typesafe) | D, E | collection / discovery source; README screened |
| [valentynkit/jev-belay](https://github.com/valentynkit/jev-belay) | A, D, E | screened; no strong match for this scope |
| [valentynkit/jev-commit](https://github.com/valentynkit/jev-commit) | A, D, E | screened; no strong match for this scope |
| [valentynkit/jev-plays-pokemon-red](https://github.com/valentynkit/jev-plays-pokemon-red) | A, D, E | screened; no strong match for this scope |
| [valentynkit/jev-skip](https://github.com/valentynkit/jev-skip) | A, D, E | screened; no strong match for this scope |
| [valentynkit/jev.nvim](https://github.com/valentynkit/jev.nvim) | A, D, E | screened; no strong match for this scope |
| [valksor/typesafe-sdk-go](https://github.com/valksor/typesafe-sdk-go) | C, E | screened; no strong match for this scope |
| [valksor/typesafe-sdk-php](https://github.com/valksor/typesafe-sdk-php) | C, E | screened; no strong match for this scope |
| [vbcherepanov/total-agent-memory](https://github.com/vbcherepanov/total-agent-memory) | E | relevant README; see discussion |
| [vehas/thaiexam-jev-charts](https://github.com/vehas/thaiexam-jev-charts) | E | screened; no strong match for this scope |
| [vercel-labs/ai-cli](https://github.com/vercel-labs/ai-cli) | A, C, E | screened; no strong match for this scope |
| [vercel-labs/ai-python](https://github.com/vercel-labs/ai-python) | A | screened; no strong match for this scope |
| [vercel-labs/fx](https://github.com/vercel-labs/fx) | A | screened; no strong match for this scope |
| [vercel-labs/json-render](https://github.com/vercel-labs/json-render) | A, E | screened; no strong match for this scope |
| [vercel/eve](https://github.com/vercel/eve) | A, E | screened; no strong match for this scope |
| [VeridicalTech/Edward](https://github.com/VeridicalTech/Edward) | D | screened; no strong match for this scope |
| [VGabriel45/polymarket-btc5m-jev-trading](https://github.com/VGabriel45/polymarket-btc5m-jev-trading) | C | screened; no strong match for this scope |
| [vinibrsl/gut](https://github.com/vinibrsl/gut) | E | screened; no strong match for this scope |
| [vinilana/jev-browser](https://github.com/vinilana/jev-browser) | E | screened; no strong match for this scope |
| [vinilana/jev-eval-agent](https://github.com/vinilana/jev-eval-agent) | C, D, E | screened; no strong match for this scope |
| [vinilana/jev-gateway](https://github.com/vinilana/jev-gateway) | E | screened; no strong match for this scope |
| [vinilana/jev-gateway-bench](https://github.com/vinilana/jev-gateway-bench) | C | screened; no strong match for this scope |
| [vinilana/live-jev](https://github.com/vinilana/live-jev) | C, E | screened; no strong match for this scope |
| [vinnie357/typesafe_sdk_ex](https://github.com/vinnie357/typesafe_sdk_ex) | E | screened; no strong match for this scope |
| [vinnylarouge/jevlike](https://github.com/vinnylarouge/jevlike) | A, C, D, E | screened; no strong match for this scope |
| [virajbhartiya/laya-vs-jev](https://github.com/virajbhartiya/laya-vs-jev) | E | screened; no strong match for this scope |
| [virolea/lintus](https://github.com/virolea/lintus) | E | screened; no strong match for this scope |
| [vmendes90/jev-shield](https://github.com/vmendes90/jev-shield) | E | screened; no strong match for this scope |
| [vnmoorthy/siege](https://github.com/vnmoorthy/siege) | E | screened; no strong match for this scope |
| [w3cj/jev-chat](https://github.com/w3cj/jev-chat) | E | screened; no strong match for this scope |
| [walidboulanouar/awesome-jev-use-cases](https://github.com/walidboulanouar/awesome-jev-use-cases) | E | collection / discovery source; README screened |
| [walidboulanouar/jev-agent-kit](https://github.com/walidboulanouar/jev-agent-kit) | C, E | screened; no strong match for this scope |
| [wd041216-bit/zero-api-key-web-search](https://github.com/wd041216-bit/zero-api-key-web-search) | E | screened lead; requires follow-up |
| [WebGrga/btc-jev-signal](https://github.com/WebGrga/btc-jev-signal) | E | screened; no strong match for this scope |
| [WebGrga/jev-board-lab](https://github.com/WebGrga/jev-board-lab) | E | screened; no strong match for this scope |
| [weitianxin/JevAny](https://github.com/weitianxin/JevAny) | E | screened; no strong match for this scope |
| [WeSecureYou/Jev-test](https://github.com/WeSecureYou/Jev-test) | E | screened; no strong match for this scope |
| [wfzyx/von](https://github.com/wfzyx/von) | A, C, E | screened; no strong match for this scope |
| [wh000wh000/awesome-jev-live](https://github.com/wh000wh000/awesome-jev-live) | E | collection / discovery source; README screened |
| [whyashthakker/awesome-jev-use-cases](https://github.com/whyashthakker/awesome-jev-use-cases) | E | collection / discovery source; README screened |
| [WiktorB2004/llama-index-jev](https://github.com/WiktorB2004/llama-index-jev) | A, D, E | screened; no strong match for this scope |
| [willkelly/jev-evaluation](https://github.com/willkelly/jev-evaluation) | E | screened; no strong match for this scope |
| [win4r/jev-skill-suggester](https://github.com/win4r/jev-skill-suggester) | C, E | screened; no strong match for this scope |
| [win4r/pi-jev-router](https://github.com/win4r/pi-jev-router) | E | screened; no strong match for this scope |
| [Wizhill05/typesafe-image-diffusion](https://github.com/Wizhill05/typesafe-image-diffusion) | E | screened; no strong match for this scope |
| [wobsoriano/is-jeven](https://github.com/wobsoriano/is-jeven) | E | screened; no strong match for this scope |
| [wobsoriano/oxlint-plugin-jev](https://github.com/wobsoriano/oxlint-plugin-jev) | E | screened; no strong match for this scope |
| [wondertwins/jev-benchmark](https://github.com/wondertwins/jev-benchmark) | E | screened; no strong match for this scope |
| [wundercorp/loki](https://github.com/wundercorp/loki) | E | screened; no strong match for this scope |
| [wustep/jev-playground](https://github.com/wustep/jev-playground) | E | screened; no strong match for this scope |
| [wuxie888/jev-yaba-wechat](https://github.com/wuxie888/jev-yaba-wechat) | E | screened; no strong match for this scope |
| [wuyoscar/jev-skill](https://github.com/wuyoscar/jev-skill) | C, E | screened; no strong match for this scope |
| [wy-coliney/jev-browser-use](https://github.com/wy-coliney/jev-browser-use) | C, D, E | screened; no strong match for this scope |
| [xerj-org/xerj](https://github.com/xerj-org/xerj) | E | screened; no strong match for this scope |
| [XieChengYuan/jev-gomoku](https://github.com/XieChengYuan/jev-gomoku) | E | screened; no strong match for this scope |
| [xienda/dsh-jev-verify](https://github.com/xienda/dsh-jev-verify) | E | screened; no strong match for this scope |
| [xingwudao/OpenJev](https://github.com/xingwudao/OpenJev) | C, E | screened; no strong match for this scope |
| [xinyao27/jevonian](https://github.com/xinyao27/jevonian) | A, C, D, E | screened; no strong match for this scope |
| [y0usaf/jev-lm](https://github.com/y0usaf/jev-lm) | E | screened; no strong match for this scope |
| [y0usaf/pi-jev](https://github.com/y0usaf/pi-jev) | A, C, D, E | screened; no strong match for this scope |
| [y0usaf/typesafe-cli](https://github.com/y0usaf/typesafe-cli) | E | screened; no strong match for this scope |
| [yairshy/decido](https://github.com/yairshy/decido) | E | screened; no strong match for this scope |
| [Yasserbhb/Agent-JEV-Tetris](https://github.com/Yasserbhb/Agent-JEV-Tetris) | E | screened; no strong match for this scope |
| [YEDASAVG/Stratum](https://github.com/YEDASAVG/Stratum) | E | screened; no strong match for this scope |
| [yibie/awesome-jev](https://github.com/yibie/awesome-jev) | D, E | collection / discovery source; README screened |
| [Yifan-Lan/awesome-jev-robustness](https://github.com/Yifan-Lan/awesome-jev-robustness) | E | collection / discovery source; README screened |
| [yikangy873-gif/jev-desktop](https://github.com/yikangy873-gif/jev-desktop) | A, C, E | screened; no strong match for this scope |
| [Ying-Kai-Liao/jev-browser](https://github.com/Ying-Kai-Liao/jev-browser) | C, E | screened; no strong match for this scope |
| [Yinsongxu/LLM2Jev](https://github.com/Yinsongxu/LLM2Jev) | E | screened; no strong match for this scope |
| [YIRC99/yanwai](https://github.com/YIRC99/yanwai) | E | screened; no strong match for this scope |
| [YIZY-API/yizy-web-app](https://github.com/YIZY-API/yizy-web-app) | C | screened; no strong match for this scope |
| [yodablocks/jev-orderby-bench](https://github.com/yodablocks/jev-orderby-bench) | A, D, E | screened; no strong match for this scope |
| [youkiti/tiab-review-plugin](https://github.com/youkiti/tiab-review-plugin) | E | screened; no strong match for this scope |
| [ypcypc/JEMM](https://github.com/ypcypc/JEMM) | E | screened; no strong match for this scope |
| [YuanKJing/Jev-as-Policy](https://github.com/YuanKJing/Jev-as-Policy) | E | screened; no strong match for this scope |
| [YueBit/robodiag-harness](https://github.com/YueBit/robodiag-harness) | E | screened; no strong match for this scope |
| [yunusey/typesafe-sdk-cpp](https://github.com/yunusey/typesafe-sdk-cpp) | C | screened; no strong match for this scope |
| [yushen100/wechat-jev-assistant](https://github.com/yushen100/wechat-jev-assistant) | E | screened; no strong match for this scope |
| [yusukebe/hono-jev-router](https://github.com/yusukebe/hono-jev-router) | A, C, D, E | screened; no strong match for this scope |
| [yutkat/github-star-organizer-jev](https://github.com/yutkat/github-star-organizer-jev) | E | screened; no strong match for this scope |
| [yuyang2230/jev-agent-skill](https://github.com/yuyang2230/jev-agent-skill) | D | screened; no strong match for this scope |
| [yzbcs/Should-I-Jev](https://github.com/yzbcs/Should-I-Jev) | A | screened; no strong match for this scope |
| [yzfly/awesome-jev-zh](https://github.com/yzfly/awesome-jev-zh) | C, E | collection / discovery source; README screened |
| [yzfly/edgejev](https://github.com/yzfly/edgejev) | E | screened; no strong match for this scope |
| [Z761293629/pi-jev-helm](https://github.com/Z761293629/pi-jev-helm) | E | screened; no strong match for this scope |
| [zadescoxp/Jev-Trades](https://github.com/zadescoxp/Jev-Trades) | E | screened; no strong match for this scope |
| [Zahrannnn/zcode-jev](https://github.com/Zahrannnn/zcode-jev) | E | screened; no strong match for this scope |
| [Zaious/jev-capability-atlas](https://github.com/Zaious/jev-capability-atlas) | D, E | screened; no strong match for this scope |
| [zavocc/ground-zero](https://github.com/zavocc/ground-zero) | E | screened; no strong match for this scope |
| [zbloss/jev-plays-pokemon](https://github.com/zbloss/jev-plays-pokemon) | E | screened; no strong match for this scope |
| [Zefan-Cai/Open-Jev](https://github.com/Zefan-Cai/Open-Jev) | E | screened; no strong match for this scope |
| [zefir1990/openjev-experiments](https://github.com/zefir1990/openjev-experiments) | E | screened; no strong match for this scope |
| [ZephyrDeng/pi-review](https://github.com/ZephyrDeng/pi-review) | E | screened; no strong match for this scope |
| [zeredy879/minojev](https://github.com/zeredy879/minojev) | D | screened; no strong match for this scope |
| [ZeroGold/call-coach-ai](https://github.com/ZeroGold/call-coach-ai) | E | screened; no strong match for this scope |
| [zhangcy122/OpenJev](https://github.com/zhangcy122/OpenJev) | C, E | screened; no strong match for this scope |
| [zhengxuyu/litjev](https://github.com/zhengxuyu/litjev) | A, D, E | screened; no strong match for this scope |
| [zhihz/openjev](https://github.com/zhihz/openjev) | A, C, D, E | screened; no strong match for this scope |
| [zhirschtritt/typesafe-go](https://github.com/zhirschtritt/typesafe-go) | C, D, E | screened; no strong match for this scope |
| [zhulinchng/jevper](https://github.com/zhulinchng/jevper) | E | screened; no strong match for this scope |
| [zhuobichen/weflow-cli](https://github.com/zhuobichen/weflow-cli) | E | screened; no strong match for this scope |
| [zhuyansen/jev-search-rerank-eval](https://github.com/zhuyansen/jev-search-rerank-eval) | D, E | relevant README; see discussion |
| [zilliztech/deep-searcher](https://github.com/zilliztech/deep-searcher) | C | screened lead; requires follow-up |
| [zilliztech/GPTCache](https://github.com/zilliztech/GPTCache) | C | screened; no strong match for this scope |
| [zilliztech/memsearch](https://github.com/zilliztech/memsearch) | C, E | relevant README; see discussion |
| [zjunlp/JevLoop](https://github.com/zjunlp/JevLoop) | E | screened; no strong match for this scope |
| [zmtomorrow/TypeAR](https://github.com/zmtomorrow/TypeAR) | E | screened; no strong match for this scope |
| [zoidsh/tenet](https://github.com/zoidsh/tenet) | E | unavailable README |
| [zsavage8/padflow-jev-evals](https://github.com/zsavage8/padflow-jev-evals) | E | screened; no strong match for this scope |
| [zsoXi/agent-handoff-gate](https://github.com/zsoXi/agent-handoff-gate) | E | screened; no strong match for this scope |
| [zszz3/Pi-Jev-Guide](https://github.com/zszz3/Pi-Jev-Guide) | E | screened; no strong match for this scope |
| [zurfyx/jev-browser-skill](https://github.com/zurfyx/jev-browser-skill) | E | screened; no strong match for this scope |
| [zwliJay/jev-forge](https://github.com/zwliJay/jev-forge) | D, E | screened; no strong match for this scope |
