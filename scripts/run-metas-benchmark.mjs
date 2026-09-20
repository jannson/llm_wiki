#!/usr/bin/env node

import { readFile } from "node:fs/promises"
import path from "node:path"
import process from "node:process"

function parseArgs(argv) {
  const options = { apiBase: "http://127.0.0.1:19828", topK: 10 }
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]
    const value = argv[index + 1]
    if (["--api-base", "--project-id", "--benchmark"].includes(argument)) {
      if (!value || value.startsWith("--")) throw new Error(`Missing value for ${argument}`)
      options[argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())] = value
      index += 1
      continue
    }
    if (argument === "--top-k") {
      options.topK = Number(value)
      index += 1
      continue
    }
    throw new Error(`Unknown argument: ${argument}`)
  }
  if (!options.projectId || !options.benchmark) {
    throw new Error("Usage: run-metas-benchmark --project-id ID --benchmark FILE [--api-base URL]")
  }
  return options
}

async function requestJson(url, init) {
  const response = await fetch(url, init)
  const body = await response.text()
  if (!response.ok) throw new Error(`${response.status} ${body.slice(0, 300)}`)
  return JSON.parse(body)
}

function searchableText(payload) {
  return JSON.stringify(payload)
}

async function runCase(apiBase, projectId, testCase, topK) {
  const started = performance.now()
  let payload
  if (testCase.mode === "read") {
    const params = new URLSearchParams({ path: testCase.path })
    payload = await requestJson(
      `${apiBase}/api/v1/projects/${encodeURIComponent(projectId)}/files/content?${params}`,
    )
  } else if (testCase.mode === "search") {
    payload = await requestJson(
      `${apiBase}/api/v1/projects/${encodeURIComponent(projectId)}/search`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ query: testCase.query, topK, includeContent: true }),
      },
    )
  } else if (testCase.mode === "graph") {
    const params = new URLSearchParams({ q: testCase.query, limit: String(topK) })
    payload = await requestJson(
      `${apiBase}/api/v1/projects/${encodeURIComponent(projectId)}/graph?${params}`,
    )
  } else if (testCase.mode.startsWith("metas-")) {
    const endpoint = testCase.mode.slice("metas-".length)
    const params = new URLSearchParams()
    for (const [key, value] of Object.entries(testCase.params ?? {})) {
      params.set(key, String(value))
    }
    payload = await requestJson(
      `${apiBase}/api/v1/projects/${encodeURIComponent(projectId)}/metas/${endpoint}?${params}`,
    )
  } else {
    throw new Error(`Unknown benchmark mode: ${testCase.mode}`)
  }

  const text = searchableText(payload)
  const expectedIds = testCase.expectedIds ?? []
  const foundIds = expectedIds.filter((id) => text.includes(id))
  const resultCount = Array.isArray(payload.results)
    ? payload.results.length
    : Array.isArray(payload.nodes)
      ? payload.nodes.length
      : Array.isArray(payload.records)
        ? payload.records.length
        : Array.isArray(payload.businesses)
          ? payload.businesses.length
        : payload.content
          ? 1
          : 0
  const countMatches = testCase.expectedCount === undefined || resultCount === testCase.expectedCount
  const gapMatches = testCase.expectedGapContains === undefined
    || (payload.knowledgeGaps ?? []).some((gap) => JSON.stringify(gap).includes(testCase.expectedGapContains))
  return {
    id: testCase.id,
    category: testCase.category,
    mode: testCase.mode,
    expectedIds,
    foundIds,
    recall: expectedIds.length === 0 ? 1 : foundIds.length / expectedIds.length,
    expectedCount: testCase.expectedCount,
    expectedGapContains: testCase.expectedGapContains,
    pass: foundIds.length === expectedIds.length && countMatches && gapMatches,
    resultCount,
    durationMs: Math.round((performance.now() - started) * 100) / 100,
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2))
  const benchmark = JSON.parse(await readFile(path.resolve(options.benchmark), "utf8"))
  const cases = []
  for (const testCase of benchmark.cases) {
    cases.push(await runCase(options.apiBase.replace(/\/+$/, ""), options.projectId, testCase, options.topK))
  }
  const passed = cases.filter((testCase) => testCase.pass).length
  const expected = cases.reduce((sum, testCase) => sum + testCase.expectedIds.length, 0)
  const found = cases.reduce((sum, testCase) => sum + testCase.foundIds.length, 0)
  process.stdout.write(`${JSON.stringify({
    name: benchmark.name,
    passed,
    failed: cases.length - passed,
    total: cases.length,
    recordRecall: expected === 0 ? 1 : found / expected,
    cases,
  }, null, 2)}\n`)
}

main().catch((error) => {
  process.stderr.write(`Benchmark failed: ${error instanceof Error ? error.message : String(error)}\n`)
  process.exitCode = 1
})
