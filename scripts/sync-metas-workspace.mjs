#!/usr/bin/env node

import { mkdir, readFile, readdir, rename, writeFile } from "node:fs/promises"
import path from "node:path"
import process from "node:process"
import yaml from "js-yaml"
import { syncMetasSources } from "./sync-metas-sources.mjs"

function parseArgs(argv) {
  const options = { json: false }
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]
    if (argument === "--json") {
      options.json = true
      continue
    }
    if (["--workspace", "--project"].includes(argument)) {
      const value = argv[index + 1]
      if (!value || value.startsWith("--")) throw new Error(`Missing value for ${argument}`)
      options[argument.slice(2)] = value
      index += 1
      continue
    }
    throw new Error(`Unknown argument: ${argument}`)
  }
  if (!options.workspace || !options.project) {
    throw new Error("Usage: sync-metas-workspace --workspace DIR --project DIR [--json]")
  }
  return options
}

async function readCatalogs(workspace) {
  const root = path.join(workspace, "01-meta-catalog")
  const catalogs = new Map()
  try {
    for (const name of (await readdir(root)).sort()) {
      if (!/\.ya?ml$/i.test(name)) continue
      const content = yaml.load(await readFile(path.join(root, name), "utf8"), { schema: yaml.JSON_SCHEMA })
      if (content?.id && content?.meta_path) catalogs.set(content.meta_path, content)
    }
  } catch (error) {
    if (error?.code !== "ENOENT") throw error
  }
  return catalogs
}

async function writeJsonAtomic(filePath, value) {
  await mkdir(path.dirname(filePath), { recursive: true })
  const temporary = `${filePath}.tmp-${process.pid}`
  await writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`)
  await rename(temporary, filePath)
}

export async function syncMetasWorkspace({ workspace, project }) {
  const workspaceRoot = path.resolve(workspace)
  const projectRoot = path.resolve(project)
  const catalogs = await readCatalogs(workspaceRoot)
  const directories = (await readdir(workspaceRoot, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory() && entry.name.endsWith("-meta"))
    .sort((left, right) => left.name.localeCompare(right.name))
  const metas = []
  const summaries = []
  for (const directory of directories) {
    const catalogEntry = catalogs.get(directory.name)
    const metaId = String(catalogEntry?.id ?? directory.name.slice(0, -"-meta".length))
    const sourceCatalog = catalogEntry?.source_catalog
      ? path.join(workspaceRoot, catalogEntry.source_catalog)
      : undefined
    const localBindings = catalogEntry?.local_binding_pattern
      ? path.join(workspaceRoot, catalogEntry.local_binding_pattern)
      : undefined
    const summary = await syncMetasSources({
      source: path.join(workspaceRoot, directory.name),
      project: projectRoot,
      metaId,
      catalog: sourceCatalog,
      localBindings,
    })
    metas.push({
      id: metaId,
      name: String(catalogEntry?.name ?? metaId),
      sourceDirectory: directory.name,
      cataloged: Boolean(catalogEntry),
      hasSourceCatalog: Boolean(sourceCatalog),
    })
    summaries.push(summary)
  }
  await writeJsonAtomic(path.join(projectRoot, ".llm-wiki/metas-sync/metas.json"), {
    version: 1,
    metas,
  })
  return { metas, summaries }
}

async function main() {
  const options = parseArgs(process.argv.slice(2))
  const result = await syncMetasWorkspace(options)
  if (options.json) process.stdout.write(`${JSON.stringify(result)}\n`)
  else process.stdout.write(`Metas workspace sync: metas=${result.metas.length} records=${result.summaries.reduce((sum, item) => sum + item.records, 0)}\n`)
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(new URL(import.meta.url).pathname)) {
  main().catch((error) => {
    process.stderr.write(`Metas workspace sync failed: ${error instanceof Error ? error.message : String(error)}\n`)
    process.exitCode = 1
  })
}
