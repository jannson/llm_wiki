# Metas Business Knowledge Schema

## Authority Boundary

Files under `raw/sources/` mirror Git-managed meta repositories and are
authoritative inputs. Files under `wiki/` are generated, disposable read models.
Generated pages must never be copied back over authoritative records.

## Stable Identity

Every business record is identified by the pair `(meta_id, id)`. Never merge
records merely because their titles, filenames, or owners are similar.

## Page Types

| Type | Directory | Purpose |
| --- | --- | --- |
| business | `wiki/businesses/` | A meta business context |
| person | `wiki/people/` | Stable person identity and aliases |
| task | `wiki/tasks/` | An actionable item with owner and business date |
| operation | `wiki/operations/` | Growth or operating initiative |
| feedback | `wiki/feedback/` | Direct user report or observation |
| insight | `wiki/insights/` | Evidence-backed interpretation of feedback |
| idea | `wiki/ideas/` | Unverified business or product hypothesis |
| experiment | `wiki/experiments/` | Planned or running validation |
| result | `wiki/results/` | Measured experiment outcome |
| decision | `wiki/decisions/` | Accepted, rejected, or superseded decision |
| playbook | `wiki/playbooks/` | Reusable operating procedure |
| metric | `wiki/metrics/` | Revenue, conversion, retention, cost, or usage measure |
| source | `wiki/sources/` | A traceable summary of an authoritative source |
| query | `wiki/queries/` | A durable investigated question |
| synthesis | `wiki/synthesis/` | Cross-source analysis, explicitly marked as inference |

## Required Frontmatter

Every generated record page must include:

```yaml
---
type: task
title: Human-readable title
meta_id: ddnsto
record_id: TASK-YYYY-NNN
status: active
owner_ids: []
priority: null
business_date: YYYY-MM-DD
last_verified: YYYY-MM-DD
evidence_type: fact
source_paths: []
relations: []
created: YYYY-MM-DD
updated: YYYY-MM-DD
---
```

Allowed `evidence_type` values are `fact`, `feedback`, `insight`, `idea`,
`experiment`, `result`, `decision`, `playbook`, and `inference`.

Use `null` for an unknown scalar and `[]` for an unknown list. Never invent a
value to complete the schema.

## Typed Relations

Each relation entry uses stable IDs:

```yaml
relations:
  - type: derived_from
    target_meta_id: ddnsto
    target_id: FB-YYYY-NNN
```

Allowed relation types:

- `derived_from`: insight or conclusion derives from evidence.
- `motivates`: feedback or insight motivates an idea or operation.
- `tests`: experiment tests an idea.
- `produces`: experiment produces a result.
- `supports`: result supports a decision.
- `contradicts`: evidence conflicts with another record.
- `decides`: decision accepts, rejects, or changes an idea or operation.
- `codified_as`: decision or result becomes a playbook.
- `impacts`: work affects a metric.
- `owned_by`: task or operation is owned by a person.
- `part_of`: record belongs to a business or larger operation.
- `supersedes`: newer record replaces an older one.

Also emit `[[wikilink]]` links for navigation, but never treat an untyped
wikilink alone as proof of a business relationship.

## Time Rules

- Use explicit business dates and `last_verified` for recency.
- Never use filesystem modification time as a business date.
- Preserve historical records; use `supersedes` instead of silently rewriting
  history.
- If date fields are absent, mark recency as unknown.

## Source and Citation Rules

- `source_paths` must point under `raw/sources/` and map back to an authoritative
  meta repository path.
- Every factual field must be present in or directly supported by a listed
  source.
- A generated `wiki/` file, `wiki/log.md`, or previous answer is never an
  authoritative source.
- Conflicting sources must both be linked and described.

## Index and Log

- `wiki/index.md` groups records by `meta_id` and type.
- `wiki/log.md` records ingest activity in reverse chronological order.
- `wiki/overview.md` summarizes current evidence and clearly marks inference.

## Lint Failures

Treat these as errors: missing stable ID, duplicate `(meta_id, id)`, unknown
owner ID, invalid date, unknown relation target, source path outside
`raw/sources/`, missing citation for a factual field, or generated content
presented as authoritative fact.

