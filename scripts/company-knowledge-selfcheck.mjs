#!/usr/bin/env node

import { readFile } from "node:fs/promises"
import path from "node:path"
import process from "node:process"
import { fileURLToPath } from "node:url"

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")

function parseArgs(argv) {
  if (argv.length !== 2 || argv[0] !== "--project") {
    throw new Error("Usage: company-knowledge-selfcheck --project DIR")
  }
  return path.resolve(argv[1])
}

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

async function main() {
  const project = parseArgs(process.argv.slice(2))
  const state = path.join(project, ".llm-wiki/metas-sync")
  const [registry, company, manifest, golden] = await Promise.all([
    readFile(path.join(state, "metas.json"), "utf8").then(JSON.parse),
    readFile(path.join(state, "company-index.json"), "utf8").then(JSON.parse),
    readFile(path.join(state, "company.manifest.json"), "utf8").then(JSON.parse),
    readFile(path.join(repositoryRoot, "scripts/company-knowledge-golden.json"), "utf8").then(JSON.parse),
  ])
  assert(registry.knowledgeDomains?.some((domain) => domain.id === "company" && domain.kind === "company"), "company domain is missing")
  assert(!registry.metas?.some((meta) => meta.id === "company"), "company must not be registered as a business meta")
  assert(company.employees?.length === 8, "expected 8 canonical employees")
  const guozhihao = company.employees.find((employee) => employee.id === "mickeysir")
  assert(["gzh", "xiaoguo", "guozhihao"].every((alias) => guozhihao?.aliases.includes(alias)), "Guo aliases are incomplete")
  assert(company.ownership.some((item) => item.metaId === "ddnsto" && item.owner === "luyong"), "DDNSTO owner is incorrect")
  assert(company.portfolio.priorityStatus === "unverified" && company.portfolio.priorityKnowledgeGap, "unknown company priorities were not preserved")
  assert(company.strategies.some((record) => record.id === "STRAT-2026-001" && record.status === "draft"), "company strategy is missing")
  assert(company.decisions.some((record) => record.id === "CDEC-2026-001" && record.status === "accepted"), "company decision is missing")
  const paths = Object.keys(manifest.files ?? {})
  assert(!paths.some((name) => name.startsWith("99-archive-") || name.startsWith("90-inbox-") || name.startsWith("migration/")), "non-current company material was indexed")
  const serialized = JSON.stringify({ registry, company, manifest })
  assert(!serialized.includes("/projects/") && !serialized.includes("workspace-linkease"), "company index leaked a machine path")
  assert(golden.questions?.length === 5, "golden company question set is incomplete")
  for (const item of golden.questions) {
    const expected = item.expect
    if (item.id === "employees-and-aliases") {
      assert(company.employees.length === expected.employeeCount, `${item.id}: employee count mismatch`)
      const employee = company.employees.find((candidate) => candidate.id === expected.employeeId)
      assert(expected.aliases.every((alias) => employee?.aliases.includes(alias)), `${item.id}: aliases mismatch`)
    } else if (item.id === "business-owner") {
      assert(company.ownership.some((entry) => entry.metaId === expected.metaId && entry.owner === expected.owner), `${item.id}: owner mismatch`)
    } else if (item.id === "current-strategy") {
      assert(company.strategies.some((entry) => entry.id === expected.recordId && entry.status === expected.status), `${item.id}: strategy mismatch`)
    } else if (item.id === "accepted-decision") {
      assert(company.decisions.some((entry) => entry.id === expected.recordId && entry.status === expected.status), `${item.id}: decision mismatch`)
    } else if (item.id === "unknown-priority") {
      assert(company.portfolio.priorityStatus === expected.priorityStatus && (!expected.mustHaveKnowledgeGap || company.portfolio.priorityKnowledgeGap), `${item.id}: priority certainty mismatch`)
    } else {
      throw new Error(`unknown golden question: ${item.id}`)
    }
  }
  process.stdout.write(`${JSON.stringify({ ok: true, employees: company.employees.length, strategies: company.strategies.length, decisions: company.decisions.length, indexedFiles: paths.length, goldenQuestions: golden.questions.length })}\n`)
}

main().catch((error) => {
  process.stderr.write(`Company knowledge self-check failed: ${error instanceof Error ? error.message : String(error)}\n`)
  process.exitCode = 1
})
