#!/usr/bin/env node
// Compatibility entry; implementation is installed from metas-platform.
import { spawnSync } from "node:child_process"
import { fileURLToPath } from "node:url"
const result = spawnSync(process.execPath, [fileURLToPath(new URL("../.metas-platform/platform/integrations/llm-wiki/scripts/metas-selfcheck.mjs", import.meta.url)), ...process.argv.slice(2)], { stdio: "inherit" })
process.exit(result.status ?? 1)
