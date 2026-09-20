# Metas Business projection

This directory contains the versioned configuration used to create a local,
rebuildable LLM Wiki read model for Git-managed `*-meta` repositories.

The generated project and mirrored sources must live outside every Git worktree.
They are caches, not authoritative business records.

Bootstrap a project:

```bash
./scripts/bootstrap-metas-project.sh /absolute/local/project/path [project-uuid]
```

Open the resulting directory in LLM Wiki to register it with the desktop API.
Deleting the local project is safe as long as the authoritative meta repositories
remain available; rerun the bootstrap and ingest steps to rebuild it.

