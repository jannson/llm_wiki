import assert from "node:assert/strict"
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import test from "node:test"
import { syncMetasWorkspace } from "./sync-metas-workspace.mjs"

async function write(root, relativePath, content) {
  const destination = path.join(root, relativePath)
  await mkdir(path.dirname(destination), { recursive: true })
  await writeFile(destination, content)
}

test("workspace sync discovers cataloged and uncataloged meta directories without merging identities", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "llm-wiki-metas-workspace-"))
  const workspace = path.join(root, "metas")
  const project = path.join(root, "project")
  try {
    await write(workspace, "01-meta-catalog/alpha.yaml", "id: alpha\nname: Alpha\nmeta_path: alpha-meta\n")
    await write(workspace, "alpha-meta/a.md", "---\nid: TASK-1\ntype: task\n---\n# Alpha task\n")
    await write(workspace, "beta-meta/b.md", "---\nid: TASK-1\ntype: task\n---\n# Beta task\n")

    const result = await syncMetasWorkspace({ workspace, project })

    assert.deepEqual(result.metas.map(({ id, cataloged }) => ({ id, cataloged })), [
      { id: "alpha", cataloged: true },
      { id: "beta", cataloged: false },
    ])
    const alpha = JSON.parse(await readFile(path.join(project, ".llm-wiki/metas-sync/alpha.records.json"), "utf8"))
    const beta = JSON.parse(await readFile(path.join(project, ".llm-wiki/metas-sync/beta.records.json"), "utf8"))
    assert.equal(alpha.records[0].id, "TASK-1")
    assert.equal(beta.records[0].id, "TASK-1")
    assert.notEqual(alpha.records[0].metaId, beta.records[0].metaId)
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})
