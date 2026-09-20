import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { mkdtemp, mkdir, readFile, rename, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import test from "node:test"
import { fileURLToPath } from "node:url"

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
const cliPath = path.join(repoRoot, "scripts", "sync-metas-sources.mjs")

async function write(root, relativePath, content) {
  const destination = path.join(root, relativePath)
  await mkdir(path.dirname(destination), { recursive: true })
  await writeFile(destination, content)
}

function runSync(source, project, metaId = "ddnsto", extraArgs = []) {
  const output = execFileSync(
    process.execPath,
    [cliPath, "--source", source, "--project", project, "--meta-id", metaId, ...extraArgs, "--json"],
    { cwd: repoRoot, encoding: "utf8" },
  )
  return JSON.parse(output)
}

test("initial sync mirrors business Markdown and YAML with exact record fields", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "llm-wiki-metas-sync-"))
  const source = path.join(root, "ddnsto-meta")
  const project = path.join(root, "wiki-project")

  try {
    await write(source, "04-user-feedback/FB-2026-001.md", `---
id: FB-2026-001
type: feedback
status: confirmed
owner: CarolChen
priority: high
created_at: 2026-09-08
last_verified: 2026-09-18
derived_from: [FB-2026-000]
---
# Feedback
`)
    await write(source, "meta.yaml", "id: ddnsto\nname: DDNSTO\n")
    await write(source, "02-source-catalog/local_ddnsto.sources.yaml", "path: /private/source\n")
    await write(source, "04-user-feedback/feedback-template.md", "id: FB-YYYY-NNN\n")
    await write(source, ".git/config", "must-not-copy\n")

    const summary = runSync(source, project)

    assert.deepEqual(
      {
        added: summary.added,
        updated: summary.updated,
        deleted: summary.deleted,
        unchanged: summary.unchanged,
        records: summary.records,
        failures: summary.failures,
      },
      { added: 2, updated: 0, deleted: 0, unchanged: 0, records: 1, failures: 0 },
    )

    const copied = await readFile(
      path.join(project, "raw/sources/ddnsto/04-user-feedback/FB-2026-001.md"),
      "utf8",
    )
    assert.match(copied, /owner: CarolChen/)
    assert.equal(
      await readFile(path.join(project, "raw/sources/ddnsto/meta.yaml"), "utf8"),
      "id: ddnsto\nname: DDNSTO\n",
    )

    const inventory = JSON.parse(
      await readFile(path.join(project, ".llm-wiki/metas-sync/ddnsto.records.json"), "utf8"),
    )
    assert.deepEqual(inventory.records, [
      {
        metaId: "ddnsto",
        id: "FB-2026-001",
        type: "feedback",
        title: "Feedback",
        status: "confirmed",
        owner: "CarolChen",
        priority: "high",
        businessDate: "2026-09-08",
        createdAt: "2026-09-08",
        lastVerified: "2026-09-18",
        sourcePath: "04-user-feedback/FB-2026-001.md",
        relatedSources: [],
        relations: [
          {
            type: "derived_from",
            targetMetaId: "ddnsto",
            targetId: "FB-2026-000",
            sourceField: "derived_from",
          },
        ],
      },
    ])

    await assert.rejects(
      readFile(path.join(project, "raw/sources/ddnsto/02-source-catalog/local_ddnsto.sources.yaml")),
      { code: "ENOENT" },
    )
    await assert.rejects(
      readFile(path.join(project, "raw/sources/ddnsto/04-user-feedback/feedback-template.md")),
      { code: "ENOENT" },
    )
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})

test("source catalog sync keeps physical paths private and exposes only availability", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "llm-wiki-metas-catalog-"))
  const source = path.join(root, "ddnsto-meta")
  const project = path.join(root, "wiki-project")
  const code = path.join(root, "code", "ddns-fe")
  const catalog = path.join(root, "ddnsto.yaml")
  const bindings = path.join(root, "local_ddnsto.sources.yaml")
  try {
    await write(source, "README.md", "# Meta\n")
    await mkdir(code, { recursive: true })
    await writeFile(catalog, `schema_version: 1\nmeta_id: ddnsto\nsources:\n  - id: ddns-fe\n    repository: https://example.test/ddns-fe\n    role: frontend\n    verifies: [user-flow]\n  - id: missing\n    repository: https://example.test/missing\n    role: absent\n`)
    await writeFile(bindings, `schema_version: 1\nmeta_id: ddnsto\nsources:\n  ddns-fe: {path: ${code}}\n  missing: {path: ${path.join(root, "absent")}}\n`)

    const summary = runSync(source, project, "ddnsto", ["--catalog", catalog, "--local-bindings", bindings])
    assert.equal(summary.catalogSources, 2)
    const publicInventory = await readFile(path.join(project, ".llm-wiki/metas-sync/ddnsto.sources.json"), "utf8")
    assert.doesNotMatch(publicInventory, new RegExp(root.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")))
    assert.deepEqual(JSON.parse(publicInventory).sources.map(({ id, localAvailable }) => ({ id, localAvailable })), [
      { id: "ddns-fe", localAvailable: true },
      { id: "missing", localAvailable: false },
    ])
    const privateInventory = await readFile(path.join(project, ".llm-wiki/metas-local-sources/ddnsto.json"), "utf8")
    assert.match(privateInventory, /ddns-fe/)
    assert.match(privateInventory, new RegExp(code.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")))
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})

test("sync extracts explicit feedback rows and canonical typed relations", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "llm-wiki-metas-relations-"))
  const source = path.join(root, "ddnsto-meta")
  const project = path.join(root, "wiki-project")

  try {
    await write(source, "04-user-feedback/task-board.md", `# Feedback\n\n| ID | 标题 | 状态 | 日期 | 类型 | 来源 | 负责人 | 优先级 | 结论 | 下一步 | 备注 | 文件 |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n| FB-20260901-03 | 套餐提示不清 | 已完成 | 2026-09-11 | 需求 | 用户 | YGAS / muting | P2 | 已上线待回归 | 回归四种状态 | - | [detail](detail.md) |\n`)
    await write(source, "05-insights/INS-2026-001.md", `---
id: INS-2026-001
type: insight
derived_from: [FB-20260901-03]
resulted_in: [IDEA-2026-001]
---
# 激活洞察
`)
    await write(source, "06-ideas/IDEA-2026-001.md", `---
id: IDEA-2026-001
type: idea
tested_by: [EXP-2026-001]
---
# 自动扫描
`)
    await write(source, "07-experiments/EXP-2026-001.md", `---
id: EXP-2026-001
type: experiment
tests: [IDEA-2026-001]
---
# 实验
`)

    runSync(source, project)
    const inventory = JSON.parse(
      await readFile(path.join(project, ".llm-wiki/metas-sync/ddnsto.records.json"), "utf8"),
    )

    assert.equal(inventory.records.find((record) => record.id === "FB-20260901-03")?.type, "feedback")
    assert.deepEqual(
      inventory.records.find((record) => record.id === "INS-2026-001")?.relations,
      [
        { type: "derived_from", targetMetaId: "ddnsto", targetId: "FB-20260901-03", sourceField: "derived_from" },
        { type: "motivates", targetMetaId: "ddnsto", targetId: "IDEA-2026-001", sourceField: "resulted_in" },
      ],
    )
    assert.deepEqual(
      inventory.records.find((record) => record.id === "EXP-2026-001")?.relations,
      [{ type: "tests", targetMetaId: "ddnsto", targetId: "IDEA-2026-001", sourceField: "tested_by" }],
    )
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})

test("repeat sync handles unchanged, update, rename, and delete without stale files", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "llm-wiki-metas-lifecycle-"))
  const source = path.join(root, "ddnsto-meta")
  const project = path.join(root, "wiki-project")
  const originalRelative = "06-ideas/IDEA-2026-001.md"
  const renamedRelative = "06-ideas/IDEA-2026-001-renamed.md"

  try {
    await write(source, originalRelative, "---\nid: IDEA-2026-001\ntype: idea\nstatus: proposed\n---\n")
    await write(source, "meta.yaml", "id: ddnsto\n")

    assert.deepEqual(
      (({ added, updated, deleted, unchanged }) => ({ added, updated, deleted, unchanged }))(
        runSync(source, project),
      ),
      { added: 2, updated: 0, deleted: 0, unchanged: 0 },
    )
    assert.deepEqual(
      (({ added, updated, deleted, unchanged }) => ({ added, updated, deleted, unchanged }))(
        runSync(source, project),
      ),
      { added: 0, updated: 0, deleted: 0, unchanged: 2 },
    )

    await write(source, originalRelative, "---\nid: IDEA-2026-001\ntype: idea\nstatus: testing\n---\n")
    assert.equal(runSync(source, project).updated, 1)

    await rename(path.join(source, originalRelative), path.join(source, renamedRelative))
    const renamed = runSync(source, project)
    assert.deepEqual(
      { added: renamed.added, deleted: renamed.deleted, unchanged: renamed.unchanged },
      { added: 1, deleted: 1, unchanged: 1 },
    )
    await assert.rejects(
      readFile(path.join(project, "raw/sources/ddnsto", originalRelative)),
      { code: "ENOENT" },
    )

    await rm(path.join(source, renamedRelative))
    const removed = runSync(source, project)
    assert.deepEqual(
      { added: removed.added, updated: removed.updated, deleted: removed.deleted, unchanged: removed.unchanged },
      { added: 0, updated: 0, deleted: 1, unchanged: 1 },
    )
    await assert.rejects(
      readFile(path.join(project, "raw/sources/ddnsto", renamedRelative)),
      { code: "ENOENT" },
    )
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})

test("sync repairs a missing mirror file even when the source hash is unchanged", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "llm-wiki-metas-repair-"))
  const source = path.join(root, "ddnsto-meta")
  const project = path.join(root, "wiki-project")
  const relativePath = "08-decisions/DEC-2026-001.md"

  try {
    await write(source, relativePath, "---\nid: DEC-2026-001\ntype: decision\nstatus: accepted\n---\n")
    runSync(source, project)
    const mirrored = path.join(project, "raw/sources/ddnsto", relativePath)
    await rm(mirrored)

    const repaired = runSync(source, project)

    assert.equal(repaired.updated, 1)
    assert.match(await readFile(mirrored, "utf8"), /status: accepted/)
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})
