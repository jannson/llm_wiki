# Metas Business Knowledge Purpose

## Goal

Provide a local, rebuildable read model for querying business knowledge across
the Metas workspace. The Git-managed `*-meta` repositories remain the only
authoritative source of business facts.

## Core Questions

1. What are the most recent tasks?
2. What are a given person's recent tasks?
3. What are the highest-priority tasks for a business, and why?
4. What are the latest core operations for a business?
5. Which tasks, owners, and user feedback are connected to an operation?
6. How does evidence flow from feedback to insight, idea, experiment, result,
   decision, playbook, and measurable business impact?

## Knowledge Loop

```text
feedback -> insight -> idea -> experiment -> result -> decision -> playbook
         -> revenue / conversion / retention
```

## Evidence Rules

- Preserve the original `meta_id`, record `id`, `type`, `owner`, `status`,
  `priority`, business date, `last_verified`, relations, and source path.
- Treat direct user reports as `feedback`, not as confirmed `insight`.
- Treat proposed ideas as unverified hypotheses, not as decisions.
- Do not claim that an experiment succeeded without a baseline, sample, time
  range, measured result, and recorded decision.
- Do not call a playbook validated unless its authoritative status is `active`.
- Product source code may verify an implemented capability, but never overrides
  user experience, business metrics, or recorded feedback.
- Surface stale, conflicting, missing, and weakly supported information instead
  of silently resolving or inventing it.
- Every factual answer must cite the authoritative source file, stable record ID,
  and `last_verified` or source date.
- Generated wiki prose is a navigation aid. When it conflicts with a source,
  the source wins.

## Scope

In scope:

- Positioning, monetization, acquisition, conversion, retention, operations,
  user feedback, insights, ideas, experiments, decisions, playbooks, and metrics.
- Cross-meta discovery and relationship tracing.
- Local source-code verification only when a product capability must be checked.

Out of scope:

- Editing authoritative meta records from generated wiki pages.
- Treating source-code implementation details as business conclusions.
- General software development history unrelated to user value or business
  outcomes.
- Publishing machine-local paths, credentials, tokens, or generated indexes.

## Answer Contract

Answers should include only applicable sections: confirmed facts, feedback,
insights, experiments and results, decisions and playbooks, metric impact,
unverified hypotheses, knowledge gaps, and evidence paths with verification
dates. Clearly label inference separately from confirmed evidence.

