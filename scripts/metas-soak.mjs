#!/usr/bin/env node

import process from "node:process"

function parseArgs(argv) {
  const options = { apiBase: "http://127.0.0.1:19828", cycles: 100, intervalMs: 50, p95LimitMs: 250 }
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index]
    const value = argv[index + 1]
    if (!["--api-base", "--project-id", "--cycles", "--interval-ms", "--duration-seconds", "--p95-limit-ms"].includes(key) || !value) {
      throw new Error("Usage: metas-soak --project-id ID [--cycles N | --duration-seconds N] [--interval-ms N] [--p95-limit-ms N]")
    }
    const name = key.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())
    options[name] = key === "--api-base" || key === "--project-id" ? value : Number(value)
    index += 1
  }
  if (!options.projectId) throw new Error("--project-id is required")
  if (options.durationSeconds !== undefined) options.cycles = Number.POSITIVE_INFINITY
  return options
}

const delay = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds))

function percentile(values, percentage) {
  const sorted = [...values].sort((left, right) => left - right)
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * percentage) - 1)] ?? 0
}

async function main() {
  const options = parseArgs(process.argv.slice(2))
  const base = options.apiBase.replace(/\/+$/, "")
  const project = `${base}/api/v1/projects/${encodeURIComponent(options.projectId)}`
  const paths = [
    `${base}/api/v1/health`,
    `${project}/metas/businesses`,
    `${project}/metas/records?metaId=ddnsto&recordId=IDEA-2026-001`,
    `${project}/metas/feedback-trace?metaId=ddnsto&recordId=FB-20260901-03`,
  ]
  const startedAt = performance.now()
  const deadline = options.durationSeconds === undefined
    ? Number.POSITIVE_INFINITY
    : startedAt + options.durationSeconds * 1000
  const durations = []
  const failures = []
  let cycles = 0
  while (cycles < options.cycles && performance.now() < deadline) {
    for (const url of paths) {
      const started = performance.now()
      try {
        const response = await fetch(url)
        const body = await response.text()
        if (!response.ok) throw new Error(`${response.status}: ${body.slice(0, 120)}`)
        JSON.parse(body)
      } catch (error) {
        failures.push({ cycle: cycles + 1, url: new URL(url).pathname, error: String(error) })
      }
      durations.push(performance.now() - started)
    }
    cycles += 1
    if (options.intervalMs > 0) await delay(options.intervalMs)
  }
  const p95Ms = percentile(durations, 0.95)
  const report = {
    ok: failures.length === 0 && p95Ms <= options.p95LimitMs,
    cycles,
    requests: durations.length,
    failures: failures.length,
    failureSamples: failures.slice(0, 10),
    elapsedSeconds: Math.round((performance.now() - startedAt) / 10) / 100,
    latencyMs: {
      p50: Math.round(percentile(durations, 0.5) * 100) / 100,
      p95: Math.round(p95Ms * 100) / 100,
      max: Math.round(Math.max(...durations) * 100) / 100,
      thresholdP95: options.p95LimitMs,
    },
  }
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`)
  if (!report.ok) process.exitCode = 1
}

main().catch((error) => {
  process.stderr.write(`Metas soak failed: ${error instanceof Error ? error.message : String(error)}\n`)
  process.exitCode = 1
})
