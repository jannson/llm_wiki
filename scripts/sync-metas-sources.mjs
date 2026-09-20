#!/usr/bin/env node

import { createHash } from "node:crypto"
import {
  copyFile,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises"
import path from "node:path"
import process from "node:process"
import yaml from "js-yaml"

const INCLUDED_EXTENSIONS = new Set([".md", ".mdx", ".yaml", ".yml"])
const EXCLUDED_DIRECTORIES = new Set([
  ".git",
  ".codex",
  ".llm-wiki",
  "node_modules",
  "dist",
  "target",
])

function parseArgs(argv) {
  const options = { json: false }
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]
    if (argument === "--json") {
      options.json = true
      continue
    }
    if (["--source", "--project", "--meta-id", "--catalog", "--local-bindings"].includes(argument)) {
      const value = argv[index + 1]
      if (!value || value.startsWith("--")) throw new Error(`Missing value for ${argument}`)
      const key = argument.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase())
      options[key] = value
      index += 1
      continue
    }
    throw new Error(`Unknown argument: ${argument}`)
  }
  if (!options.source || !options.project || !options.metaId) {
    throw new Error("Usage: sync-metas-sources --source DIR --project DIR --meta-id ID [--catalog FILE --local-bindings FILE] [--json]")
  }
  if (!/^[a-z0-9][a-z0-9_-]*$/i.test(options.metaId)) {
    throw new Error(`Invalid meta id: ${options.metaId}`)
  }
  return options
}

function normalizeRelative(relativePath) {
  return relativePath.split(path.sep).join("/")
}

function shouldInclude(relativePath, directoryEntry) {
  if (directoryEntry.isDirectory()) return !EXCLUDED_DIRECTORIES.has(directoryEntry.name)
  if (!directoryEntry.isFile()) return false
  const lowerName = directoryEntry.name.toLowerCase()
  if (lowerName.startsWith("local_") && /\.sources\.ya?ml$/.test(lowerName)) return false
  if (lowerName === "template.md" || lowerName.endsWith("-template.md")) return false
  return INCLUDED_EXTENSIONS.has(path.extname(lowerName))
}

async function collectFiles(root, relativeDirectory = "", includeRelativePath = () => true) {
  const absoluteDirectory = path.join(root, relativeDirectory)
  const entries = await readdir(absoluteDirectory, { withFileTypes: true })
  const files = []
  for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
    const relativePath = path.join(relativeDirectory, entry.name)
    if (!shouldInclude(relativePath, entry)) continue
    if (entry.isDirectory()) files.push(...(await collectFiles(root, relativePath, includeRelativePath)))
    else {
      const normalized = normalizeRelative(relativePath)
      if (includeRelativePath(normalized)) files.push(normalized)
    }
  }
  return files
}

function sha256(content) {
  return createHash("sha256").update(content).digest("hex")
}

function scalar(value) {
  if (value === undefined || value === null || value === "") return null
  return String(value)
}

function list(value) {
  if (value === undefined || value === null || value === "") return []
  return (Array.isArray(value) ? value : [value])
    .map((item) => String(item).trim())
    .filter(Boolean)
}

function relationSpecs(frontmatter, recordType) {
  const specs = []
  const add = (field, type, direction = "forward") => {
    for (const targetId of list(frontmatter[field])) specs.push({ type, targetId, sourceField: field, direction })
  }
  add("derived_from", "derived_from")
  add("resulted_in", "motivates")
  add("tests", "tests")
  add("tested_by", "tests", "reverse")
  add("experiment", "produces", "reverse")
  add("decisions", "motivates")
  add("based_on", "derived_from")
  add("produces", recordType === "decision" ? "codified_as" : "produces")
  add("measures", "impacts")
  add("metrics", "impacts")
  add("expected_outcomes", "impacts")
  return specs
}

function parseFrontmatterRecord(metaId, sourcePath, content) {
  if (!sourcePath.toLowerCase().endsWith(".md")) return null
  const match = /^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/.exec(content)
  if (!match) return null
  const frontmatter = yaml.load(match[1], { schema: yaml.JSON_SCHEMA })
  if (!frontmatter || typeof frontmatter !== "object" || Array.isArray(frontmatter)) return null
  if (!frontmatter.id || !frontmatter.type) return null
  const createdAt = scalar(frontmatter.created_at ?? frontmatter.created)
  const heading = /^#\s+(.+)$/m.exec(content.slice(match[0].length))?.[1]?.trim()
  const recordType = String(frontmatter.type)
  return {
    metaId,
    id: String(frontmatter.id),
    type: recordType,
    title: scalar(frontmatter.title) ?? heading ?? String(frontmatter.id),
    status: scalar(frontmatter.status),
    owner: scalar(frontmatter.owner),
    priority: scalar(frontmatter.priority),
    businessDate: scalar(
      frontmatter.business_date ?? frontmatter.date ?? frontmatter.occurred_at ?? createdAt,
    ),
    createdAt,
    lastVerified: scalar(frontmatter.last_verified),
    sourcePath,
    relatedSources: list(frontmatter.related_sources),
    _relationSpecs: relationSpecs(frontmatter, recordType),
  }
}

function tableCells(line) {
  if (!line.trim().startsWith("|")) return []
  return line.trim().replace(/^\||\|$/g, "").split("|").map((cell) => cell.trim())
}

function parseFeedbackBoard(metaId, sourcePath, content) {
  if (path.basename(sourcePath).toLowerCase() !== "task-board.md") return []
  const records = []
  for (const line of content.split(/\r?\n/)) {
    const cells = tableCells(line)
    if (!/^FB-[A-Z0-9-]+$/i.test(cells[0] ?? "")) continue
    records.push({
      metaId,
      id: cells[0],
      type: "feedback",
      title: cells[1] || cells[0],
      status: scalar(cells[2]),
      owner: scalar(cells[6]),
      priority: scalar(cells[7]),
      businessDate: scalar(cells[3]),
      createdAt: null,
      lastVerified: null,
      sourcePath,
      relatedSources: [],
      _relationSpecs: [],
    })
  }
  return records
}

function materializeRelations(records) {
  const byId = new Map(records.map((record) => [record.id, record]))
  for (const record of records) record.relations = []
  for (const record of records) {
    for (const spec of record._relationSpecs) {
      const source = spec.direction === "reverse" ? byId.get(spec.targetId) : record
      const sourceId = spec.direction === "reverse" ? spec.targetId : record.id
      const targetId = spec.direction === "reverse" ? record.id : spec.targetId
      const relation = {
        type: spec.type,
        targetMetaId: metaIdFor(record),
        targetId,
        sourceField: spec.sourceField,
      }
      if (source && !source.relations.some((item) => item.type === relation.type && item.targetId === targetId)) {
        source.relations.push(relation)
      } else if (!source && sourceId === record.id) {
        record.relations.push(relation)
      }
    }
  }
  for (const record of records) {
    delete record._relationSpecs
    record.relations.sort((left, right) => `${left.type}\0${left.targetId}`.localeCompare(`${right.type}\0${right.targetId}`))
  }
}

function metaIdFor(record) {
  return record.metaId
}

async function readJson(filePath, fallback) {
  try {
    return JSON.parse(await readFile(filePath, "utf8"))
  } catch (error) {
    if (error?.code === "ENOENT") return fallback
    throw error
  }
}

async function writeJsonAtomic(filePath, value) {
  await mkdir(path.dirname(filePath), { recursive: true })
  const temporary = `${filePath}.tmp-${process.pid}`
  await writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`)
  await rename(temporary, filePath)
}

async function pathIsDirectory(candidate) {
  try {
    return (await stat(candidate)).isDirectory()
  } catch (error) {
    if (error?.code === "ENOENT") return false
    throw error
  }
}

async function syncSourceCatalog({ catalog, localBindings, stateRoot, projectRoot, metaId }) {
  if (!catalog) return 0
  const publicCatalog = yaml.load(await readFile(path.resolve(catalog), "utf8"), { schema: yaml.JSON_SCHEMA })
  if (!publicCatalog || publicCatalog.meta_id !== metaId || !Array.isArray(publicCatalog.sources)) {
    throw new Error(`Source catalog meta_id must be ${metaId} and sources must be a list`)
  }
  let bindings = {}
  if (localBindings) {
    const local = yaml.load(await readFile(path.resolve(localBindings), "utf8"), { schema: yaml.JSON_SCHEMA })
    if (!local || local.meta_id !== metaId || !local.sources || typeof local.sources !== "object") {
      throw new Error(`Local source bindings meta_id must be ${metaId}`)
    }
    bindings = local.sources
  }
  const sources = []
  const privateSources = {}
  for (const source of publicCatalog.sources) {
    if (!source?.id) throw new Error("Every source catalog entry requires id")
    const bindingPath = scalar(bindings[source.id]?.path)
    const localAvailable = bindingPath ? await pathIsDirectory(bindingPath) : false
    sources.push({
      id: String(source.id),
      repository: scalar(source.repository),
      role: scalar(source.role),
      lifecycle: scalar(source.lifecycle),
      kind: scalar(source.kind),
      verifies: list(source.verifies),
      localAvailable,
    })
    if (bindingPath) privateSources[source.id] = { path: path.resolve(bindingPath) }
  }
  await writeJsonAtomic(path.join(stateRoot, `${metaId}.sources.json`), {
    version: 1,
    metaId,
    sources,
  })
  if (localBindings) {
    await writeJsonAtomic(path.join(projectRoot, ".llm-wiki", "metas-local-sources", `${metaId}.json`), {
      version: 1,
      metaId,
      sources: privateSources,
    })
  }
  return sources.length
}

export async function syncMetasSources({ source, project, metaId, catalog, localBindings, includeRelativePath }) {
  const sourceRoot = path.resolve(source)
  const projectRoot = path.resolve(project)
  const sourceInfo = await stat(sourceRoot)
  if (!sourceInfo.isDirectory()) throw new Error(`Source is not a directory: ${sourceRoot}`)
  if (projectRoot === sourceRoot || projectRoot.startsWith(`${sourceRoot}${path.sep}`)) {
    throw new Error("Project directory must not be inside the authoritative source directory")
  }

  const mirrorRoot = path.join(projectRoot, "raw", "sources", metaId)
  const stateRoot = path.join(projectRoot, ".llm-wiki", "metas-sync")
  const manifestPath = path.join(stateRoot, `${metaId}.manifest.json`)
  const recordsPath = path.join(stateRoot, `${metaId}.records.json`)
  const previous = await readJson(manifestPath, { files: {} })
  const relativeFiles = await collectFiles(sourceRoot, "", includeRelativePath)
  const currentFiles = {}
  const records = []
  const recordKeys = new Set()
  let added = 0
  let updated = 0
  let unchanged = 0

  await mkdir(mirrorRoot, { recursive: true })
  for (const relativePath of relativeFiles) {
    const sourcePath = path.join(sourceRoot, relativePath)
    const destinationPath = path.join(mirrorRoot, relativePath)
    const content = await readFile(sourcePath)
    const hash = sha256(content)
    currentFiles[relativePath] = { sha256: hash, size: content.byteLength }
    const previousHash = previous.files?.[relativePath]?.sha256
    let destinationHash = null
    try {
      destinationHash = sha256(await readFile(destinationPath))
    } catch (error) {
      if (error?.code !== "ENOENT") throw error
    }
    if (previousHash === hash && destinationHash === hash) {
      unchanged += 1
    } else {
      await mkdir(path.dirname(destinationPath), { recursive: true })
      await copyFile(sourcePath, destinationPath)
      if (previousHash) updated += 1
      else added += 1
    }

    const text = content.toString("utf8")
    const parsedRecords = [parseFrontmatterRecord(metaId, relativePath, text), ...parseFeedbackBoard(metaId, relativePath, text)].filter(Boolean)
    for (const record of parsedRecords) {
      const key = `${record.metaId}\u0000${record.id}`
      if (recordKeys.has(key)) throw new Error(`Duplicate record identity: (${metaId}, ${record.id})`)
      recordKeys.add(key)
      records.push(record)
    }
  }

  let deleted = 0
  for (const previousPath of Object.keys(previous.files ?? {})) {
    if (currentFiles[previousPath]) continue
    await rm(path.join(mirrorRoot, previousPath), { force: true })
    deleted += 1
  }

  materializeRelations(records)
  records.sort((left, right) =>
    left.sourcePath.localeCompare(right.sourcePath) || left.id.localeCompare(right.id),
  )
  await writeJsonAtomic(manifestPath, {
    version: 1,
    metaId,
    sourceLabel: metaId,
    syncedAt: new Date().toISOString(),
    files: currentFiles,
  })
  await writeJsonAtomic(recordsPath, { version: 1, metaId, records })
  const catalogSources = await syncSourceCatalog({
    catalog,
    localBindings,
    stateRoot,
    projectRoot,
    metaId,
  })

  return {
    metaId,
    sourceRoot,
    mirrorRoot,
    added,
    updated,
    deleted,
    unchanged,
    records: records.length,
    catalogSources,
    failures: 0,
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2))
  const summary = await syncMetasSources({
    source: options.source,
    project: options.project,
    metaId: options.metaId,
    catalog: options.catalog,
    localBindings: options.localBindings,
  })
  if (options.json) process.stdout.write(`${JSON.stringify(summary)}\n`)
  else {
    process.stdout.write(
      `Metas sync ${summary.metaId}: added=${summary.added} updated=${summary.updated} ` +
        `deleted=${summary.deleted} unchanged=${summary.unchanged} records=${summary.records}\n`,
    )
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(new URL(import.meta.url).pathname)) {
  main().catch((error) => {
    process.stderr.write(`Metas sync failed: ${error instanceof Error ? error.message : String(error)}\n`)
    process.exitCode = 1
  })
}
