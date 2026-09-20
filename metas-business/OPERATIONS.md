# Metas Business Operations

## Build

```bash
npm ci
npm run build
cargo build --manifest-path src-tauri/Cargo.toml --bin llm-wiki
npm --prefix mcp-server run build
```

Linux desktop execution requires GTK/WebKit and Ayatana AppIndicator runtime
libraries. A headless machine additionally needs an X server such as Xvfb.

## Bootstrap and synchronize

The generated project must be outside Git worktrees:

```bash
./scripts/bootstrap-metas-project.sh /absolute/local/metas-business
npm run metas:sync-workspace -- \
  --workspace /absolute/path/to/metas \
  --project /absolute/local/metas-business
```

Workspace synchronization discovers every `*-meta` directory. Entries in
`01-meta-catalog/` supply stable names and optional source catalogs. A directory
without structured frontmatter remains visible with zero records and a knowledge
gap; ordinary prose is not promoted to a task or decision.

Machine-local source paths are written only under the generated project's
`.llm-wiki/metas-local-sources/`. API and MCP responses expose source IDs,
availability, repository-relative paths, lines, and snippets, never binding
paths. There is no GitHub fallback.

## Start and verify

Open the generated project once in the source-built desktop app so it receives
a project UUID. Then run:

```bash
npm run metas:selfcheck -- --project-id PROJECT_UUID
npm run metas:benchmark -- --project-id PROJECT_UUID --benchmark metas-business/benchmarks/ddnsto-structured.json
npm run metas:benchmark -- --project-id PROJECT_UUID --benchmark metas-business/benchmarks/ddnsto-knowledge-chain.json
npm run metas:benchmark -- --project-id PROJECT_UUID --benchmark metas-business/benchmarks/cross-meta.json
```

For a 24-hour soak with one four-request cycle every second:

```bash
npm run metas:soak -- \
  --project-id PROJECT_UUID \
  --duration-seconds 86400 \
  --interval-ms 1000 \
  --p95-limit-ms 250
```

Use the default 100-cycle accelerated soak for release smoke testing.

## Recovery

- API unavailable: restart the source-built desktop app, then rerun self-check.
- MCP disconnected: restart `node mcp-server/dist/src/index.js`, initialize it,
  and pin the generated project UUID again.
- Missing or damaged mirror: rerun workspace synchronization. Hash comparison
  repairs missing/corrupted mirrors and deletes stale mirrors.
- Missing local source: fix `02-source-catalog/local_<meta>.sources.yaml`, run
  `meta-doctor --meta <id> --require-local`, then synchronize again.
- Generated project lost: recreate it with the bootstrap script and synchronize;
  authoritative records remain in the Git-managed meta repositories.

Never copy generated `wiki/`, inventories, or mirrored files back over an
authoritative meta repository.
