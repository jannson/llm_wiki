# Headless core provenance and structured generation

This directory was brought into the AgentFlow task worktree from the source repository commit `98a5778df6b28437a934bbbfd799121f378a9922`. The task worktree originally pointed at `e8082119649e6a8e1cf85eaf289adcabfdf39d4e`, before this generic headless core existed. Only the eleven `headless/` baseline files were imported; no desktop, MCP, integration, branch history, or source-repository changes were merged. No commits or pushes were made by this task.

The existing independent Cargo package remains the service core. `Provider::structured(instruction, input)` reuses its model configuration, credentials, HTTP client, redirect restrictions, timeouts, errors and redaction. It sends server-owned instructions separately from untrusted evidence, requests bounded JSON output, and rejects invalid/truncated JSON. `generation_profile()` identifies the generic generation configuration without returning secrets.

Company-specific task fields, candidate strategy semantics, task/document authorization and result validation belong to the `metas-platform/integrations/llm-wiki/server` adapter and `company-heartbeat/taskoverview`, not this package. `context_profile` / `generate_context` are private extension operations; the public HTTP Wiki allowlist does not expose them. The adapter must be built against a core containing this interface.

Verification: `cargo test --manifest-path headless/Cargo.toml --locked`, plus `make check` in the corresponding metas-platform worktree for the joint server and task workflow. Tests use synthetic local Providers; they do not call a production model. No production provider config was edited.
