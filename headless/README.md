# Headless Wiki core

Independent Rust processing/storage and HTTP-over-Unix entry point, built without the desktop package or its build script. The desktop imports only shared multilingual text utilities and provider limits with `default-features = false`; server dependencies are behind the `server` feature. Company adapters and external authentication remain in the platform repository.

```sh
rtk cargo test --manifest-path headless/Cargo.toml --locked
rtk cargo build --manifest-path headless/Cargo.toml --release --locked
rtk proxy headless/target/release/llm-wiki-server --check-config /private/engine.json
rtk proxy headless/target/release/llm-wiki-server --config /private/engine.json
```

Private JSON config (0600) explicitly supplies `socket`, `database`, `model_file`, and `projects` (`id`, `name`, `root`, `requirements`). The model file uses `SERVER`, `KEY`, `LLM_MODEL`, `EMBEDDING_MODEL`. No environment/path guessing or desktop configuration import occurs. `allow_local_provider` is false by default and exists for synthetic loopback fixtures. `extensions` is an opaque adapter policy object.

The trusted gateway sends POST `/rpc` with an operation and explicit grants. Required grants are conjunctive, with exact IDs or gateway-generated prefix wildcards. The Unix socket is owner-only; do not publish the RPC endpoint or forward arbitrary old desktop API routes. Operations include discovery, pages, version reading, search, queue submit/control, explicit revocation and reindex. Extensions consume company envelopes without coupling the generic core to company record semantics.

SQLite atomically publishes immutable evidence/analysis/vector revisions and importer checkpoints; Markdown files are repairable projections. One worker persists completed generation parts and embedding batches and resumes interrupted jobs. Model/profile changes preserve old evidence, mark indexes stale and require reindex. Pause/cancel takes effect at a checkpoint boundary. An interrupted in-flight external model call may be repeated. Revocation tombstones prevent empty deltas or repeat imports from accidentally resurrecting removed sources.

A trusted adapter may explicitly prepare `source.processing_policy: {mode: "metadata_only", summary: "..."}` when source fields provide no basis for generative analysis. The core validates a nonempty summary of at most 2000 characters, retains the original evidence, skips generation and embeds the prepared summary with the configured model. Public versions report `analysis_category=source_metadata_summary`; ordinary generated versions report `ai_analysis`. The policy participates in revision identity, with history preserved. This is an importer-controlled policy, not an instruction extracted from untrusted document text. Adapter scheduling must consider policy changes as well as source fingerprints. It uses existing free-form source metadata, leaving the SQLite schema and vector profile unchanged.

UTF-8 Markdown/text is the initial supported import format. Keyword and exact vector ranking use a small embedded corpus; graph enrichment, desktop multimodal parsing and chats are outside this server. Provider errors are explicit safe categories, query failure degrades to local keywords, and all 1024-dimensional embedding requests contain at most 10 inputs. Full company deployment/backup/rollback instructions live in the platform's `integrations/llm-wiki/web-runbook.md`.
