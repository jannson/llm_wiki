use serde::{Deserialize, Serialize};
use std::fs;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MetasRelation {
    #[serde(rename = "type")]
    pub relation_type: String,
    pub target_meta_id: String,
    pub target_id: String,
    pub source_field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MetasRecord {
    pub meta_id: String,
    pub id: String,
    #[serde(rename = "type")]
    pub record_type: String,
    #[serde(default)]
    pub title: String,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub priority: Option<String>,
    pub business_date: Option<String>,
    pub created_at: Option<String>,
    pub last_verified: Option<String>,
    pub source_path: String,
    #[serde(default)]
    pub mirror_path: Option<String>,
    #[serde(default)]
    pub relations: Vec<MetasRelation>,
    #[serde(default)]
    pub related_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceCatalogEntry {
    pub id: String,
    pub repository: Option<String>,
    pub role: Option<String>,
    pub lifecycle: Option<String>,
    pub kind: Option<String>,
    #[serde(default)]
    pub verifies: Vec<String>,
    pub local_available: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceCatalogInventory {
    meta_id: String,
    sources: Vec<SourceCatalogEntry>,
}

#[derive(Debug, Deserialize)]
struct LocalSourceBinding {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalSourceInventory {
    meta_id: String,
    sources: HashMap<String, LocalSourceBinding>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceCodeMatch {
    pub source_id: String,
    pub relative_path: String,
    pub line: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaRegistryEntry {
    pub id: String,
    pub name: String,
    pub cataloged: bool,
    pub has_source_catalog: bool,
}

#[derive(Debug, Deserialize)]
struct MetaRegistry {
    metas: Vec<MetaRegistryEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BusinessSummary {
    pub id: String,
    pub name: String,
    pub cataloged: bool,
    pub has_source_catalog: bool,
    pub record_count: usize,
    pub type_counts: BTreeMap<String, usize>,
    pub latest_business_date: Option<String>,
    pub knowledge_gap_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MetasEdge {
    pub meta_id: String,
    pub source_id: String,
    pub target_id: String,
    #[serde(rename = "type")]
    pub relation_type: String,
    pub source_field: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeGap {
    pub kind: String,
    pub meta_id: String,
    pub record_id: Option<String>,
    pub target_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TraceResult {
    pub nodes: Vec<MetasRecord>,
    pub edges: Vec<MetasEdge>,
    pub knowledge_gaps: Vec<KnowledgeGap>,
}

#[derive(Debug, Deserialize)]
struct RecordInventory {
    records: Vec<MetasRecord>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FocusScore {
    pub priority: i64,
    pub status: i64,
    pub business_date: String,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScoredRecord {
    #[serde(flatten)]
    pub record: MetasRecord,
    pub score: FocusScore,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecordQuery {
    pub meta_id: Option<String>,
    pub record_id: Option<String>,
    pub record_type: Option<String>,
    pub owner: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: usize,
}

fn equal_filter(value: &str, expected: &Option<String>) -> bool {
    expected
        .as_ref()
        .map(|expected| value.eq_ignore_ascii_case(expected.trim()))
        .unwrap_or(true)
}

fn optional_filter(value: Option<&str>, expected: &Option<String>) -> bool {
    expected
        .as_ref()
        .map(|expected| {
            value
                .map(|value| value.eq_ignore_ascii_case(expected.trim()))
                .unwrap_or(false)
        })
        .unwrap_or(true)
}

fn effective_date(record: &MetasRecord) -> &str {
    record
        .business_date
        .as_deref()
        .or(record.last_verified.as_deref())
        .unwrap_or("")
}

pub fn query_records(records: Vec<MetasRecord>, query: &RecordQuery) -> Vec<MetasRecord> {
    let mut found: Vec<MetasRecord> = records
        .into_iter()
        .filter(|record| equal_filter(&record.meta_id, &query.meta_id))
        .filter(|record| equal_filter(&record.id, &query.record_id))
        .filter(|record| equal_filter(&record.record_type, &query.record_type))
        .filter(|record| optional_filter(record.owner.as_deref(), &query.owner))
        .filter(|record| optional_filter(record.status.as_deref(), &query.status))
        .filter(|record| optional_filter(record.priority.as_deref(), &query.priority))
        .filter(|record| {
            query
                .date_from
                .as_ref()
                .map(|date_from| effective_date(record) >= date_from.trim())
                .unwrap_or(true)
        })
        .filter(|record| {
            query
                .date_to
                .as_ref()
                .map(|date_to| effective_date(record) <= date_to.trim())
                .unwrap_or(true)
        })
        .collect();

    found.sort_by(|left, right| {
        effective_date(right)
            .cmp(effective_date(left))
            .then_with(|| left.meta_id.cmp(&right.meta_id))
            .then_with(|| left.id.cmp(&right.id))
    });
    found.truncate(query.limit.max(1).min(1_000));
    found
}

pub fn load_records(project_path: &str) -> Result<Vec<MetasRecord>, String> {
    let state_root = Path::new(project_path).join(".llm-wiki/metas-sync");
    let entries = match fs::read_dir(&state_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Failed to list Metas record inventories: {error}")),
    };
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("Failed to read Metas inventory entry: {error}"))?;
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(|error| format!("Failed to inspect Metas inventory: {error}"))?
            .is_file()
            || !path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.ends_with(".records.json"))
                .unwrap_or(false)
        {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("Failed to inspect Metas inventory size: {error}"))?;
        if metadata.len() > 10 * 1024 * 1024 {
            return Err(format!("Metas record inventory is too large: {}", path.display()));
        }
        let raw = fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read Metas inventory {}: {error}", path.display()))?;
        let inventory: RecordInventory = serde_json::from_str(&raw)
            .map_err(|error| format!("Invalid Metas inventory {}: {error}", path.display()))?;
        for mut record in inventory.records {
            if record.title.trim().is_empty() {
                record.title = record.id.clone();
            }
            record.mirror_path = Some(format!(
                "raw/sources/{}/{}",
                record.meta_id,
                record.source_path.trim_start_matches('/')
            ));
            records.push(record);
        }
    }
    records.sort_by(|left, right| {
        left.meta_id
            .cmp(&right.meta_id)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(records)
}

pub fn load_source_catalog(project_path: &str, meta_id: &str) -> Result<Vec<SourceCatalogEntry>, String> {
    let path = Path::new(project_path)
        .join(".llm-wiki/metas-sync")
        .join(format!("{meta_id}.sources.json"));
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Source catalog is unavailable for {meta_id}: {error}"))?;
    let inventory: SourceCatalogInventory = serde_json::from_str(&raw)
        .map_err(|error| format!("Invalid source catalog for {meta_id}: {error}"))?;
    if !inventory.meta_id.eq_ignore_ascii_case(meta_id) {
        return Err(format!("Source catalog identity mismatch for {meta_id}"));
    }
    Ok(inventory.sources)
}

pub fn business_summaries(project_path: &str, records: &[MetasRecord]) -> Result<Vec<BusinessSummary>, String> {
    let path = Path::new(project_path).join(".llm-wiki/metas-sync/metas.json");
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Metas registry is unavailable: {error}"))?;
    let registry: MetaRegistry = serde_json::from_str(&raw)
        .map_err(|error| format!("Invalid Metas registry: {error}"))?;
    let mut summaries = Vec::new();
    for meta in registry.metas {
        let scoped: Vec<&MetasRecord> = records
            .iter()
            .filter(|record| record.meta_id.eq_ignore_ascii_case(&meta.id))
            .collect();
        let mut type_counts = BTreeMap::new();
        let mut latest_business_date = None;
        for record in &scoped {
            *type_counts.entry(record.record_type.clone()).or_insert(0) += 1;
            if let Some(date) = record.business_date.as_ref() {
                if latest_business_date.as_ref().map(|latest| date > latest).unwrap_or(true) {
                    latest_business_date = Some(date.clone());
                }
            }
        }
        summaries.push(BusinessSummary {
            id: meta.id.clone(),
            name: meta.name,
            cataloged: meta.cataloged,
            has_source_catalog: meta.has_source_catalog,
            record_count: scoped.len(),
            type_counts,
            latest_business_date,
            knowledge_gap_count: knowledge_gaps(records, Some(&meta.id)).len()
                + usize::from(scoped.is_empty()),
        });
    }
    summaries.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(summaries)
}

fn resolve_local_source(project_path: &str, meta_id: &str, source_id: &str) -> Result<std::path::PathBuf, String> {
    let catalog = load_source_catalog(project_path, meta_id)?;
    let Some(source) = catalog.iter().find(|source| source.id.eq_ignore_ascii_case(source_id)) else {
        return Err(format!("Unknown source id {source_id} for {meta_id}"));
    };
    if !source.local_available {
        return Err(format!("Source {source_id} has no available local binding; remote fallback is disabled"));
    }
    let path = Path::new(project_path)
        .join(".llm-wiki/metas-local-sources")
        .join(format!("{meta_id}.json"));
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Local source bindings are unavailable for {meta_id}: {error}"))?;
    let inventory: LocalSourceInventory = serde_json::from_str(&raw)
        .map_err(|error| format!("Invalid local source bindings for {meta_id}: {error}"))?;
    if !inventory.meta_id.eq_ignore_ascii_case(meta_id) {
        return Err(format!("Local source binding identity mismatch for {meta_id}"));
    }
    let binding = inventory
        .sources
        .iter()
        .find(|(id, _)| id.eq_ignore_ascii_case(source_id))
        .map(|(_, binding)| binding)
        .ok_or_else(|| format!("Source {source_id} has no local binding; remote fallback is disabled"))?;
    let root = std::path::PathBuf::from(&binding.path);
    if !root.is_dir() {
        return Err(format!("Local binding for {source_id} is unavailable; remote fallback is disabled"));
    }
    Ok(root)
}

fn searchable_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()).unwrap_or("").to_ascii_lowercase().as_str(),
        "rs" | "ts" | "tsx" | "js" | "jsx" | "go" | "zig" | "c" | "h" | "cpp" | "hpp" | "java" | "kt" | "py" | "sh" | "md" | "toml" | "yaml" | "yml" | "json" | "html" | "css" | "scss" | "vue" | "svelte"
    )
}

pub fn search_local_source(
    project_path: &str,
    meta_id: &str,
    source_id: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SourceCodeMatch>, String> {
    let query = query.trim();
    if query.len() < 2 {
        return Err("Source query must contain at least 2 characters".into());
    }
    let root = resolve_local_source(project_path, meta_id, source_id)?;
    let needle = query.to_lowercase();
    let mut matches = Vec::new();
    let walker = walkdir::WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            !entry.file_type().is_dir()
                || !matches!(entry.file_name().to_str().unwrap_or(""), ".git" | "node_modules" | "dist" | "target" | "vendor")
        });
    for entry in walker.filter_map(Result::ok).take(20_000) {
        if !entry.file_type().is_file() || !searchable_source_file(entry.path()) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else { continue };
        if metadata.len() > 2 * 1024 * 1024 {
            continue;
        }
        let Ok(content) = fs::read_to_string(entry.path()) else { continue };
        for (index, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&needle) {
                matches.push(SourceCodeMatch {
                    source_id: source_id.into(),
                    relative_path: entry
                        .path()
                        .strip_prefix(&root)
                        .unwrap_or(entry.path())
                        .to_string_lossy()
                        .replace('\\', "/"),
                    line: index + 1,
                    snippet: line.trim().chars().take(300).collect(),
                });
                if matches.len() >= limit.clamp(1, 100) {
                    return Ok(matches);
                }
            }
        }
    }
    Ok(matches)
}

fn priority_score(priority: Option<&str>) -> i64 {
    match priority.unwrap_or("").to_ascii_lowercase().as_str() {
        "critical" | "urgent" | "p0" => 4,
        "high" | "p1" => 3,
        "medium" | "normal" | "p2" => 2,
        "low" | "p3" => 1,
        _ => 0,
    }
}

fn status_score(status: Option<&str>) -> i64 {
    match status.unwrap_or("").to_ascii_lowercase().as_str() {
        "active" | "running" | "testing" | "in-progress" | "in_progress" => 3,
        "planned" | "proposed" | "awaiting-results" => 2,
        "blocked" | "on-hold" | "on_hold" => 1,
        _ => 0,
    }
}

fn date_score(date: &str) -> i64 {
    date.chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>()
        .parse::<i64>()
        .unwrap_or(0)
}

pub fn score_business_focus(
    records: Vec<MetasRecord>,
    meta_id: Option<String>,
    limit: usize,
) -> Vec<ScoredRecord> {
    let query = RecordQuery {
        meta_id,
        record_type: Some("task".into()),
        limit: 1_000,
        ..RecordQuery::default()
    };
    let mut scored: Vec<ScoredRecord> = query_records(records, &query)
        .into_iter()
        .map(|record| {
            let priority = priority_score(record.priority.as_deref());
            let status = status_score(record.status.as_deref());
            let business_date = effective_date(&record).to_string();
            let total = priority * 1_000_000_000 + status * 100_000_000 + date_score(&business_date);
            ScoredRecord {
                record,
                score: FocusScore {
                    priority,
                    status,
                    business_date,
                    total,
                },
            }
        })
        .collect();
    scored.sort_by(|left, right| {
        right
            .score
            .total
            .cmp(&left.score.total)
            .then_with(|| left.record.meta_id.cmp(&right.record.meta_id))
            .then_with(|| left.record.id.cmp(&right.record.id))
    });
    scored.truncate(limit.max(1).min(1_000));
    scored
}

const ALLOWED_RELATIONS: &[&str] = &[
    "derived_from",
    "motivates",
    "tests",
    "produces",
    "supports",
    "contradicts",
    "decides",
    "codified_as",
    "impacts",
    "owned_by",
    "part_of",
    "supersedes",
];

pub fn knowledge_gaps(records: &[MetasRecord], meta_id: Option<&str>) -> Vec<KnowledgeGap> {
    use std::collections::{HashMap, HashSet};

    let scoped: Vec<&MetasRecord> = records
        .iter()
        .filter(|record| meta_id.map(|id| record.meta_id.eq_ignore_ascii_case(id)).unwrap_or(true))
        .collect();
    let ids: HashSet<(String, String)> = scoped
        .iter()
        .map(|record| (record.meta_id.to_ascii_lowercase(), record.id.to_ascii_lowercase()))
        .collect();
    let mut counts = HashMap::new();
    for record in &scoped {
        *counts
            .entry((record.meta_id.to_ascii_lowercase(), record.id.to_ascii_lowercase()))
            .or_insert(0usize) += 1;
    }
    let mut gaps = Vec::new();
    for record in scoped {
        let key = (record.meta_id.to_ascii_lowercase(), record.id.to_ascii_lowercase());
        if counts.get(&key).copied().unwrap_or(0) > 1 {
            gaps.push(KnowledgeGap {
                kind: "duplicate-id".into(),
                meta_id: record.meta_id.clone(),
                record_id: Some(record.id.clone()),
                target_id: None,
                message: format!("Duplicate record identity ({}, {}).", record.meta_id, record.id),
            });
        }
        if record.business_date.is_none() {
            gaps.push(KnowledgeGap {
                kind: "missing-business-date".into(),
                meta_id: record.meta_id.clone(),
                record_id: Some(record.id.clone()),
                target_id: None,
                message: format!("{} has no explicit business date.", record.id),
            });
        }
        if record.last_verified.is_none() {
            gaps.push(KnowledgeGap {
                kind: "missing-last-verified".into(),
                meta_id: record.meta_id.clone(),
                record_id: Some(record.id.clone()),
                target_id: None,
                message: format!("{} has no last_verified date.", record.id),
            });
        }
        for relation in &record.relations {
            if !ALLOWED_RELATIONS.contains(&relation.relation_type.as_str()) {
                gaps.push(KnowledgeGap {
                    kind: "unknown-relation-type".into(),
                    meta_id: record.meta_id.clone(),
                    record_id: Some(record.id.clone()),
                    target_id: Some(relation.target_id.clone()),
                    message: format!("{} uses unknown relation type {}.", record.id, relation.relation_type),
                });
            }
            let target_key = (
                relation.target_meta_id.to_ascii_lowercase(),
                relation.target_id.to_ascii_lowercase(),
            );
            if !ids.contains(&target_key) {
                gaps.push(KnowledgeGap {
                    kind: "unknown-relation-target".into(),
                    meta_id: record.meta_id.clone(),
                    record_id: Some(record.id.clone()),
                    target_id: Some(relation.target_id.clone()),
                    message: format!("{} points to missing record {}.", record.id, relation.target_id),
                });
            }
        }
    }
    gaps.sort_by(|left, right| {
        (&left.meta_id, &left.kind, &left.record_id, &left.target_id)
            .cmp(&(&right.meta_id, &right.kind, &right.record_id, &right.target_id))
    });
    gaps.dedup();
    gaps
}

pub fn trace_records(
    records: Vec<MetasRecord>,
    meta_id: &str,
    record_id: &str,
    max_depth: usize,
) -> TraceResult {
    use std::collections::{HashMap, HashSet, VecDeque};

    let scoped: Vec<MetasRecord> = records
        .into_iter()
        .filter(|record| record.meta_id.eq_ignore_ascii_case(meta_id))
        .collect();
    let by_id: HashMap<String, &MetasRecord> = scoped
        .iter()
        .map(|record| (record.id.to_ascii_lowercase(), record))
        .collect();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([(record_id.to_ascii_lowercase(), 0usize)]);
    while let Some((current, depth)) = queue.pop_front() {
        if depth > max_depth || !by_id.contains_key(&current) || !visited.insert(current.clone()) {
            continue;
        }
        if depth == max_depth {
            continue;
        }
        for record in &scoped {
            for relation in &record.relations {
                let source = record.id.to_ascii_lowercase();
                let target = relation.target_id.to_ascii_lowercase();
                if source == current && by_id.contains_key(&target) {
                    queue.push_back((target.clone(), depth + 1));
                }
                if target == current && by_id.contains_key(&source) {
                    queue.push_back((source, depth + 1));
                }
            }
        }
    }
    let mut nodes: Vec<MetasRecord> = scoped
        .iter()
        .filter(|record| visited.contains(&record.id.to_ascii_lowercase()))
        .cloned()
        .collect();
    nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let mut edges = Vec::new();
    for record in &scoped {
        if !visited.contains(&record.id.to_ascii_lowercase()) {
            continue;
        }
        for relation in &record.relations {
            if visited.contains(&relation.target_id.to_ascii_lowercase())
                && by_id.contains_key(&relation.target_id.to_ascii_lowercase())
            {
                edges.push(MetasEdge {
                    meta_id: record.meta_id.clone(),
                    source_id: record.id.clone(),
                    target_id: relation.target_id.clone(),
                    relation_type: relation.relation_type.clone(),
                    source_field: relation.source_field.clone(),
                });
            }
        }
    }
    edges.sort_by(|left, right| {
        (&left.source_id, &left.relation_type, &left.target_id)
            .cmp(&(&right.source_id, &right.relation_type, &right.target_id))
    });
    let mut gaps = knowledge_gaps(&nodes, Some(meta_id));
    if !by_id.contains_key(&record_id.to_ascii_lowercase()) {
        gaps.push(KnowledgeGap {
            kind: "missing-anchor".into(),
            meta_id: meta_id.into(),
            record_id: Some(record_id.into()),
            target_id: None,
            message: format!("Anchor record {record_id} does not exist."),
        });
    }
    TraceResult { nodes, edges, knowledge_gaps: gaps }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, record_type: &str, owner: &str, date: &str) -> MetasRecord {
        MetasRecord {
            meta_id: "ddnsto".into(),
            id: id.into(),
            record_type: record_type.into(),
            title: id.into(),
            status: Some("active".into()),
            owner: Some(owner.into()),
            priority: Some("high".into()),
            business_date: Some(date.into()),
            created_at: Some(date.into()),
            last_verified: Some("2026-09-20".into()),
            source_path: format!("tasks/{id}.md"),
            mirror_path: None,
            relations: Vec::new(),
            related_sources: Vec::new(),
        }
    }

    #[test]
    fn trace_walks_only_explicit_edges_and_reports_unknown_targets() {
        let mut feedback = record("FB-1", "feedback", "Alice", "2026-09-01");
        let mut insight = record("INS-1", "insight", "Alice", "2026-09-02");
        let idea = record("IDEA-1", "idea", "Alice", "2026-09-03");
        feedback.relations.push(MetasRelation {
            relation_type: "motivates".into(),
            target_meta_id: "ddnsto".into(),
            target_id: "INS-1".into(),
            source_field: "resulted_in".into(),
        });
        insight.relations.push(MetasRelation {
            relation_type: "motivates".into(),
            target_meta_id: "ddnsto".into(),
            target_id: "IDEA-1".into(),
            source_field: "resulted_in".into(),
        });
        insight.relations.push(MetasRelation {
            relation_type: "derived_from".into(),
            target_meta_id: "ddnsto".into(),
            target_id: "MISSING-1".into(),
            source_field: "derived_from".into(),
        });

        let traced = trace_records(vec![feedback, insight, idea], "ddnsto", "INS-1", 10);

        assert_eq!(traced.nodes.iter().map(|record| record.id.as_str()).collect::<Vec<_>>(), vec!["FB-1", "IDEA-1", "INS-1"]);
        assert_eq!(traced.edges.len(), 2);
        assert!(traced.knowledge_gaps.iter().any(|gap| gap.kind == "unknown-relation-target" && gap.target_id.as_deref() == Some("MISSING-1")));
        assert!(!traced.nodes.iter().any(|record| record.id == "MISSING-1"));
    }

    #[test]
    fn local_source_search_returns_only_relative_paths() {
        let root = std::env::temp_dir().join(format!("llm-wiki-source-search-{}", std::process::id()));
        let project = root.join("project");
        let source = root.join("private-source");
        fs::create_dir_all(project.join(".llm-wiki/metas-sync")).unwrap();
        fs::create_dir_all(project.join(".llm-wiki/metas-local-sources")).unwrap();
        fs::create_dir_all(source.join("src")).unwrap();
        fs::write(source.join("src/app.ts"), "const autoScan = true;\n").unwrap();
        fs::write(
            project.join(".llm-wiki/metas-sync/ddnsto.sources.json"),
            r#"{"metaId":"ddnsto","sources":[{"id":"ddns-fe","repository":null,"role":null,"lifecycle":null,"kind":null,"verifies":[],"localAvailable":true}]}"#,
        ).unwrap();
        fs::write(
            project.join(".llm-wiki/metas-local-sources/ddnsto.json"),
            serde_json::json!({"metaId":"ddnsto","sources":{"ddns-fe":{"path":source}}}).to_string(),
        ).unwrap();

        let found = search_local_source(project.to_str().unwrap(), "ddnsto", "ddns-fe", "autoScan", 5).unwrap();

        assert_eq!(found[0].relative_path, "src/app.ts");
        assert!(!serde_json::to_string(&found).unwrap().contains(root.to_str().unwrap()));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn query_keeps_identical_record_ids_isolated_by_meta() {
        let mut alpha = record("TASK-1", "task", "Alice", "2026-09-01");
        alpha.meta_id = "alpha".into();
        let mut beta = record("TASK-1", "task", "Alice", "2026-09-02");
        beta.meta_id = "beta".into();

        let found = query_records(vec![alpha, beta], &RecordQuery {
            meta_id: Some("beta".into()),
            record_id: Some("TASK-1".into()),
            limit: 10,
            ..RecordQuery::default()
        });

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].meta_id, "beta");
    }

    #[test]
    fn filters_tasks_by_owner_and_sorts_by_explicit_business_date() {
        let records = vec![
            record("TASK-OLD", "task", "CarolChen", "2026-09-01"),
            record("TASK-NEW", "task", "CarolChen", "2026-09-18"),
            record("TASK-OTHER", "task", "Alice", "2026-09-19"),
            record("OP-NEW", "operation", "CarolChen", "2026-09-20"),
        ];
        let query = RecordQuery {
            record_type: Some("task".into()),
            owner: Some("carolchen".into()),
            limit: 10,
            ..RecordQuery::default()
        };

        let found = query_records(records, &query);

        assert_eq!(
            found.iter().map(|record| record.id.as_str()).collect::<Vec<_>>(),
            vec!["TASK-NEW", "TASK-OLD"]
        );
    }

    #[test]
    fn business_focus_uses_a_public_priority_status_and_date_score() {
        let mut critical = record("TASK-CRITICAL", "task", "CarolChen", "2026-09-01");
        critical.priority = Some("critical".into());
        let mut recent = record("TASK-RECENT", "task", "CarolChen", "2026-09-20");
        recent.priority = Some("high".into());
        let operation = record("OP-1", "operation", "CarolChen", "2026-09-21");

        let found = score_business_focus(vec![recent, operation, critical], None, 10);

        assert_eq!(
            found.iter().map(|item| item.record.id.as_str()).collect::<Vec<_>>(),
            vec!["TASK-CRITICAL", "TASK-RECENT"]
        );
        assert_eq!(found[0].score.priority, 4);
        assert_eq!(found[0].score.status, 3);
        assert_eq!(found[0].score.business_date, "2026-09-01");
        assert!(found[0].score.total > found[1].score.total);
    }
}
