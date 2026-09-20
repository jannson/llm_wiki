#!/usr/bin/env node

import process from "node:process"

function parseArgs(argv) {
  const options = { apiBase: "http://127.0.0.1:19828", metaId: "ddnsto" }
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index]
    const value = argv[index + 1]
    if (!["--api-base", "--project-id", "--meta-id"].includes(key) || !value) {
      throw new Error("Usage: metas-selfcheck --project-id ID [--api-base URL] [--meta-id ID]")
    }
    options[key.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = value
    index += 1
  }
  if (!options.projectId) throw new Error("--project-id is required")
  return options
}

async function json(url) {
  const started = performance.now()
  const response = await fetch(url)
  const text = await response.text()
  if (!response.ok) throw new Error(`${response.status} ${url}: ${text.slice(0, 300)}`)
  return { body: JSON.parse(text), text, durationMs: performance.now() - started }
}

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

async function main() {
  const options = parseArgs(process.argv.slice(2))
  const base = options.apiBase.replace(/\/+$/, "")
  const project = `${base}/api/v1/projects/${encodeURIComponent(options.projectId)}`
  const meta = encodeURIComponent(options.metaId)
  const checks = []
  const run = async (name, path, verify) => {
    const result = await json(path)
    verify(result.body, result.text)
    checks.push({ name, durationMs: Math.round(result.durationMs * 100) / 100 })
  }
  await run("health", `${base}/api/v1/health`, (body) => assert(body.ok === true && body.enabled === true, "API is not healthy and enabled"))
  await run("businesses", `${project}/metas/businesses`, (body) => assert(body.businesses?.some((item) => item.id === options.metaId), `Missing business ${options.metaId}`))
  await run("exact-record", `${project}/metas/records?metaId=${meta}&recordId=IDEA-2026-001`, (body) => assert(body.count === 1 && body.records[0]?.id === "IDEA-2026-001", "Exact record lookup failed"))
  await run("feedback-trace", `${project}/metas/feedback-trace?metaId=${meta}&recordId=FB-20260901-03`, (body) => assert(body.nodeCount === 7 && body.nodes.some((item) => item.id === "SOP-2026-001"), "Feedback chain is incomplete"))
  await run("source-context-privacy", `${project}/metas/source-context?metaId=${meta}&recordId=EXP-2026-001`, (body, text) => {
    assert(body.sources?.some((item) => item.id === "ddns-fe" && item.localAvailable === true), "Source context is unavailable")
    assert(!text.includes("/projects/") && !text.includes("workspace-linkease"), "Source context leaked a physical path")
    assert(body.remoteFallback === false, "Remote source fallback must remain disabled")
  })
  await run("missing-source-boundary", `${project}/metas/source-search?metaId=${meta}&sourceId=not-configured&query=anything`, (body, text) => {
    assert(body.matches?.length === 0 && body.remoteFallback === false, "Missing source did not fail closed")
    assert(!text.includes("/projects/") && !text.includes("workspace-linkease"), "Missing source response leaked a physical path")
  })
  process.stdout.write(`${JSON.stringify({ ok: true, checks, maxDurationMs: Math.max(...checks.map((item) => item.durationMs)) }, null, 2)}\n`)
}

main().catch((error) => {
  process.stderr.write(`Metas self-check failed: ${error instanceof Error ? error.message : String(error)}\n`)
  process.exitCode = 1
})
