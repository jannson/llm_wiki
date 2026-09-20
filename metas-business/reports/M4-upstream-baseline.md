# M4 — Upstream retrieval baseline

Verified: 2026-09-20

## Dataset

- Project: local Metas Business projection
- Authoritative source: `ddnsto-meta`
- Mirrored supported files: 196
- Structured records with stable IDs: 6
- Benchmark cases: 6

## Results

| Case | Category | Expected | Found | Recall | Result |
| --- | --- | ---: | ---: | ---: | --- |
| source-read | authoritative file read | 1 | 1 | 100% | PASS |
| exact-record-id | exact ID search | 1 | 0 | 0% | FAIL |
| chinese-topic | Chinese topic search | 2 | 0 | 0% | FAIL |
| person-owner | owner query | 5 | 0 | 0% | FAIL |
| recent-records | recency query | 6 | 0 | 0% | FAIL |
| knowledge-loop | graph relationship query | 6 | 0 | 0% | FAIL |

Aggregate record recall was 1/21 (4.76%). The successful record came from the
direct file-read endpoint, not retrieval.

## Confirmed upstream capabilities

- API and MCP can list the registered project and read an authoritative mirrored
  file by exact path.
- Chinese paths and content survive the mirror and API read unchanged.
- Keyword search and graph calls are fast on this empty generated Wiki.

## Confirmed gaps

1. Upstream search indexes generated `wiki/` pages, not authoritative files under
   `raw/sources/`; mirroring a meta does not make its records searchable.
2. The graph is built from generated Wiki links. It contains no nodes or edges
   before LLM-generated pages exist.
3. Search has no owner, status, date, priority, business, or record-type filters.
4. Graph edges are wikilinks rather than typed business relations.
5. Recency cannot be determined reliably from the current search response.

## Configuration versus source changes

Configuration can enable an LLM provider, run the generic ingest pipeline, and
optionally enable embeddings. That would improve exploratory retrieval but would
not make owner/date/status filtering or typed business joins deterministic.

Source changes are required for:

- indexing authoritative frontmatter without waiting for LLM generation;
- exact record-ID lookup;
- owner, status, date, priority, type, and business filters;
- deterministic recency sorting;
- typed relation traversal with missing-target reporting;
- cross-meta aggregation.

## Reproduction

Run the checked-in benchmark while the local desktop API is active:

```bash
npm run metas:benchmark -- \
  --project-id PROJECT_UUID \
  --benchmark metas-business/benchmarks/ddnsto-baseline.json
```

The project UUID and machine paths remain local and are not stored in this
report or benchmark definition.
