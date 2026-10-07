use crate::{
    engine::{allows, Config},
    provider::{chunks, digest, Provider, CHUNK_CHARS},
    text,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub identity: String,
    pub title: String,
    pub content: String,
    pub source: Value,
    pub source_date: Option<String>,
    pub evidence_category: String,
    pub requirements: Vec<String>,
}
#[derive(Clone)]
pub struct Store {
    pub db: Arc<Mutex<Connection>>,
    pub config: Config,
    pub provider: Provider,
}
#[derive(Clone, Serialize, Deserialize)]
struct Revision {
    evidence: Evidence,
    analysis: String,
    chunks: Vec<String>,
    vectors: Vec<Vec<f32>>,
    profile: String,
    indexed_at: String,
}
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
fn storage(_: rusqlite::Error) -> String {
    "storage_failed".into()
}
impl Store {
    pub fn open(config: Config, provider: Provider) -> Result<Self, String> {
        let parent = Path::new(&config.database)
            .parent()
            .ok_or("database_invalid")?;
        fs::create_dir_all(parent).map_err(|_| "database_invalid")?;
        if fs::symlink_metadata(&config.database).is_ok_and(|m| !m.is_file()) {
            return Err("database_path_invalid".into());
        }
        let db = Connection::open(&config.database).map_err(storage)?;
        db.busy_timeout(Duration::from_secs(5)).map_err(storage)?;
        db.execute_batch("PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL;
  CREATE TABLE IF NOT EXISTS wiki_schema(version INTEGER NOT NULL);
  INSERT INTO wiki_schema SELECT 1 WHERE NOT EXISTS(SELECT 1 FROM wiki_schema);
  CREATE TABLE IF NOT EXISTS jobs(id INTEGER PRIMARY KEY,project TEXT NOT NULL,identity TEXT NOT NULL,revision TEXT NOT NULL,profile TEXT NOT NULL,payload TEXT NOT NULL,status TEXT NOT NULL,phase TEXT NOT NULL DEFAULT 'analysis',analysis TEXT NOT NULL DEFAULT '[]',vectors TEXT NOT NULL DEFAULT '[]',attempts INTEGER NOT NULL DEFAULT 0,next_at INTEGER NOT NULL DEFAULT 0,error TEXT,updated_at TEXT NOT NULL, UNIQUE(project,identity,revision,profile));
  CREATE TABLE IF NOT EXISTS revisions(project TEXT NOT NULL,identity TEXT NOT NULL,revision TEXT NOT NULL,profile TEXT NOT NULL,payload TEXT NOT NULL,active INTEGER NOT NULL,revoked INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(project,identity,revision,profile));
  CREATE TABLE IF NOT EXISTS checkpoints(project TEXT NOT NULL,identity TEXT NOT NULL,revision TEXT NOT NULL,PRIMARY KEY(project,identity));
  CREATE TABLE IF NOT EXISTS revocations(project TEXT NOT NULL,identity TEXT NOT NULL,revoked_at TEXT NOT NULL,PRIMARY KEY(project,identity));
  UPDATE jobs SET status='pending' WHERE status='processing';").map_err(storage)?;
        let schema: i64 = db
            .query_row("SELECT version FROM wiki_schema", [], |r| r.get(0))
            .map_err(storage)?;
        if schema != 1 {
            return Err("wiki_schema_unsupported".into());
        }
        db.execute("UPDATE jobs SET status='failed',error='index_profile_changed' WHERE profile<>? AND status IN ('pending','paused','processing')",[&provider.profile]).map_err(storage)?;
        fs::set_permissions(&config.database, fs::Permissions::from_mode(0o600))
            .map_err(|_| "database_permissions")?;
        for p in &config.projects {
            for dir in [
                Path::new(&p.root).to_path_buf(),
                Path::new(&p.root).join("wiki"),
                Path::new(&p.root).join("wiki/server-pages"),
            ] {
                if fs::symlink_metadata(&dir).is_ok_and(|m| !m.is_dir()) {
                    return Err("project_path_invalid".into());
                }
                fs::create_dir_all(&dir).map_err(|_| "project_path_invalid")?;
            }
        }
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            config,
            provider,
        })
    }
    pub fn validate_evidence(&self, e: &Evidence) -> Result<(), String> {
        if e.identity.is_empty()
            || e.identity.len() > 512
            || e.title.len() > 512
            || e.content.is_empty()
            || e.content.chars().count() > 64000
            || e.requirements.is_empty()
            || e.source.to_string().len() > 8192
            || self
                .provider
                .rejects_secret(&serde_json::to_string(e).unwrap())
        {
            return Err("evidence_invalid".into());
        }
        if e.requirements
            .iter()
            .any(|x| x.is_empty() || x.contains('*'))
        {
            return Err("evidence_scope_invalid".into());
        }
        if e.evidence_category.len() > 128
            || e.source_date.as_ref().is_some_and(|d| {
                chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err()
                    && chrono::DateTime::parse_from_rfc3339(d).is_err()
            })
        {
            return Err("evidence_date_invalid".into());
        }
        Ok(())
    }
    pub fn submit(&self, project: &str, e: Evidence) -> Result<Value, String> {
        self.validate_evidence(&e)?;
        if !self.config.projects.iter().any(|p| p.id == project) {
            return Err("project_unknown".into());
        }
        let payload = serde_json::to_string(&e).unwrap();
        let mut stable: Value = serde_json::from_str(&payload).map_err(|_| "evidence_invalid")?;
        if let Some(source) = stable["source"].as_object_mut() {
            source.remove("observed_at");
        }
        let revision = digest(&format!("{}:{}", stable, self.provider.profile));
        let mut db = self.db.lock().unwrap();
        let revoked: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM revocations WHERE project=? AND identity=?)",
                params![project, e.identity],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if revoked {
            return Err("source_revoked".into());
        }
        db.execute("INSERT OR IGNORE INTO jobs(project,identity,revision,profile,payload,status,updated_at) VALUES(?,?,?,?,?,'pending',?)",params![project,e.identity,revision,self.provider.profile,payload,chrono::Utc::now().to_rfc3339()]).map_err(storage)?;
        let(id,status):(i64,String)=db.query_row("SELECT id,status FROM jobs WHERE project=? AND identity=? AND revision=? AND profile=?",params![project,e.identity,revision,self.provider.profile],|r|Ok((r.get(0)?,r.get(1)?))).map_err(storage)?;
        if status == "done" {
            let current: Option<String> = db
                .query_row(
                    "SELECT revision FROM checkpoints WHERE project=? AND identity=?",
                    params![project, e.identity],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            let pending: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE project=? AND identity=? AND id<>? AND status IN ('pending','processing','paused'))",params![project,e.identity,id],|r|r.get(0)).map_err(storage)?;
            if current.as_deref() != Some(&revision) || pending {
                // A source can revert to an already organized version. Publish
                // its cached revision atomically and stop superseded work rather
                // than retaining newer content or paying to organize it again.
                let tx = db.transaction().map_err(storage)?;
                tx.execute("UPDATE jobs SET status='cancelled',error='superseded_revision',updated_at=? WHERE project=? AND identity=? AND id<>? AND status IN ('pending','processing','paused')",params![chrono::Utc::now().to_rfc3339(),project,e.identity,id]).map_err(storage)?;
                tx.execute(
                    "UPDATE revisions SET active=0 WHERE project=? AND identity=?",
                    params![project, e.identity],
                )
                .map_err(storage)?;
                tx.execute("UPDATE revisions SET active=1 WHERE project=? AND identity=? AND revision=? AND profile=? AND revoked=0",params![project,e.identity,revision,self.provider.profile]).map_err(storage)?;
                tx.execute("INSERT INTO checkpoints VALUES(?,?,?) ON CONFLICT(project,identity) DO UPDATE SET revision=excluded.revision",params![project,e.identity,revision]).map_err(storage)?;
                tx.execute(
                    "UPDATE jobs SET updated_at=? WHERE id=?",
                    params![chrono::Utc::now().to_rfc3339(), id],
                )
                .map_err(storage)?;
                tx.commit().map_err(storage)?;
            }
        }
        Ok(json!({"job_id":id,"status":status,"revision":revision}))
    }
    fn load_active(
        &self,
        project: &str,
        grants: &[String],
    ) -> Result<Vec<(String, Revision)>, String> {
        let db = self.db.lock().unwrap();
        let mut stmt = db
            .prepare(
                "SELECT revision,payload FROM revisions WHERE project=? AND active=1 AND revoked=0",
            )
            .map_err(storage)?;
        let mut out = Vec::new();
        for row in stmt
            .query_map([project], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(storage)?
        {
            let (rev, p) = row.map_err(storage)?;
            let r: Revision = serde_json::from_str(&p).map_err(|_| "stored_revision_invalid")?;
            if allows(&r.evidence.requirements, grants) {
                out.push((rev, r))
            }
        }
        Ok(out)
    }
    fn public_revision(rev: &str, r: &Revision) -> Value {
        json!({"page_id":rev,"title":r.evidence.title,"identity":r.evidence.identity,"source":r.evidence.source,"source_date":r.evidence.source_date,"evidence_category":r.evidence.evidence_category,"indexed_at":r.indexed_at,"analysis_category":"ai_analysis","profile":r.profile})
    }
    pub fn handle(&self, input: &Value) -> Result<Value, String> {
        let grants: Vec<String> =
            serde_json::from_value(input["grants"].clone()).map_err(|_| "scope_invalid")?;
        let op = input["operation"].as_str().ok_or("operation_invalid")?;
        if op == "projects" {
            return Ok(
                json!({"projects":self.config.projects.iter().filter(|p|allows(&p.requirements,&grants)).map(|p| {
                    // Presentation is explicitly configured, never inferred from a
                    // project name. Only these public fields cross the boundary;
                    // import policies and private source paths remain server-side.
                    let presentation = &self.config.extensions["presentation"]["projects"][&p.id];
                    let kind = match presentation["kind"].as_str() {
                        Some("synthetic") => "synthetic",
                        _ => "knowledge",
                    };
                    let questions: Vec<_> = presentation["suggested_questions"].as_array()
                        .into_iter().flatten().filter_map(|q|q.as_str())
                        .filter(|q|q.chars().count() <= 1000).take(4).collect();
                    json!({"id":p.id,"name":p.name,"kind":kind,
                        "description":presentation["description"].as_str().unwrap_or(""),
                        "suggested_questions":questions})
                }).collect::<Vec<_>>()}),
            );
        }
        let project = input["project"].as_str().ok_or("project_missing")?;
        let spec = self
            .config
            .projects
            .iter()
            .find(|p| p.id == project)
            .ok_or("project_unknown")?;
        if !allows(&spec.requirements, &grants) {
            return Err("forbidden".into());
        }
        match op {
            "submit" => {
                let e = Evidence {
                    identity: input["identity"].as_str().ok_or("identity_missing")?.into(),
                    title: input["title"].as_str().ok_or("title_missing")?.into(),
                    content: input["content"].as_str().ok_or("content_missing")?.into(),
                    source: json!({"kind":"manual_document","path":input["source_path"].as_str().unwrap_or("unknown"),"record_id":input["identity"]}),
                    source_date: input["source_date"].as_str().map(String::from),
                    evidence_category: "unverified_uploaded_material".into(),
                    requirements: spec.requirements.clone(),
                };
                self.submit(project, e)
            }
            "pages" => Ok(
                json!({"pages":self.load_active(project,&grants)?.iter().map(|(id,r)|Self::public_revision(id,r)).collect::<Vec<_>>()}),
            ),
            "read" => {
                let id = input["page_id"].as_str().ok_or("page_missing")?;
                if id.len() != 64 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err("page_path_invalid".into());
                }
                let (payload, active): (String, bool) = {
                    let db = self.db.lock().unwrap();
                    db.query_row("SELECT payload,active FROM revisions WHERE project=? AND revision=? AND revoked=0 ORDER BY active DESC LIMIT 1",params![project,id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(|_|"page_not_found")?
                };
                let r: Revision =
                    serde_json::from_str(&payload).map_err(|_| "stored_revision_invalid")?;
                if !allows(&r.evidence.requirements, &grants) {
                    return Err("page_not_found".into());
                }
                let mut value = Self::public_revision(id, &r);
                value["revision_status"] = json!(if active { "current" } else { "superseded" });
                value["markdown"] = json!(format!(
                    "# {}\n\n## AI 整理（待结合原文判断）\n\n{}\n\n## 原始证据\n\n{}",
                    r.evidence.title, r.analysis, r.evidence.content
                ));
                value["original"] = json!(r.evidence.content);
                value["analysis"] = json!(r.analysis);
                Ok(value)
            }
            "search" => self.search(
                project,
                input["query"].as_str().ok_or("query_missing")?,
                &grants,
            ),
            "status" => {
                let rows = self.load_active(project, &grants)?;
                let stale = rows
                    .iter()
                    .filter(|(_, r)| r.profile != self.provider.profile)
                    .count();
                Ok(
                    json!({"pages":rows.len(),"stale_pages":stale,"index_state":if stale>0{"reindex_required"}else{"current"},"model":self.provider.llm,"embedding_model":self.provider.embedding,"dimensions":1024,"profile":self.provider.profile,"worker_concurrency":1,"max_attempts":3,"scope":"approved evidence only; not all Reader facts"}),
                )
            }
            "jobs" => self.jobs(project, &grants),
            "control" => self.control(project, input, &grants),
            "revoke" => {
                let id = input["identity"].as_str().ok_or("identity_missing")?;
                let visible = self
                    .load_active(project, &grants)?
                    .iter()
                    .any(|(_, r)| r.evidence.identity == id);
                if !visible {
                    return Err("evidence_not_found".into());
                }
                let mut db = self.db.lock().unwrap();
                let tx = db.transaction().map_err(storage)?;
                tx.execute(
                    "UPDATE revisions SET active=0,revoked=1 WHERE project=? AND identity=?",
                    params![project, id],
                )
                .map_err(storage)?;
                tx.execute("UPDATE jobs SET status='cancelled',updated_at=? WHERE project=? AND identity=? AND status IN ('pending','processing','paused','failed')",params![chrono::Utc::now().to_rfc3339(),project,id]).map_err(storage)?;
                tx.execute("INSERT INTO revocations VALUES(?,?,?) ON CONFLICT(project,identity) DO UPDATE SET revoked_at=excluded.revoked_at",params![project,id,chrono::Utc::now().to_rfc3339()]).map_err(storage)?;
                tx.execute(
                    "DELETE FROM checkpoints WHERE project=? AND identity=?",
                    params![project, id],
                )
                .map_err(storage)?;
                tx.commit().map_err(storage)?;
                Ok(json!({"status":"revoked","identity":id}))
            }
            "reindex" => {
                let rows = self.load_active(project, &grants)?;
                let mut result = Vec::new();
                for (_, r) in rows {
                    result.push(self.submit(project, r.evidence)?)
                }
                Ok(json!({"jobs":result}))
            }
            _ => Err("operation_denied".into()),
        }
    }
    fn search(&self, project: &str, q: &str, grants: &[String]) -> Result<Value, String> {
        if q.trim().is_empty() || q.chars().count() > 1000 {
            return Err("query_invalid".into());
        }
        let rows = self.load_active(project, grants)?;
        let stale = rows.iter().any(|(_, r)| r.profile != self.provider.profile);
        let query_vector = if rows.iter().any(|(_, r)| r.profile == self.provider.profile) {
            self.provider
                .embed(&[q.to_string()])
                .map(|mut v| v.remove(0))
        } else {
            Err("index_empty".into())
        };
        let mut keywords = Vec::new();
        let mut vectors = Vec::new();
        let tokens = text::tokenize_query(q);
        for (id, r) in &rows {
            let body = format!(
                "{}\n{}\n{}",
                r.evidence.title, r.evidence.content, r.analysis
            )
            .to_lowercase();
            let kw = tokens.iter().filter(|s| body.contains(s.as_str())).count();
            if kw > 0 {
                keywords.push((id.clone(), kw as f64))
            }
            if r.profile == self.provider.profile {
                if let Ok(ref v) = query_vector {
                    let score = r
                        .vectors
                        .iter()
                        .map(|x| cosine(v, x))
                        .fold(-1.0f64, f64::max);
                    if score >= 0.45 {
                        vectors.push((id.clone(), score))
                    }
                }
            }
        }
        keywords.sort_by(|a, b| b.1.total_cmp(&a.1));
        vectors.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut scores = std::collections::BTreeMap::<String, f64>::new();
        for list in [&keywords, &vectors] {
            for (i, (id, _)) in list.iter().enumerate() {
                *scores.entry(id.clone()).or_default() += 1. / (60. + i as f64 + 1.)
            }
        }
        let mut ranked: Vec<_> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        let result: Vec<_> = ranked
            .into_iter()
            .take(20)
            .map(|(id, score)| {
                let r = &rows.iter().find(|(i, _)| *i == id).unwrap().1;
                let mut v = Self::public_revision(&id, r);
                v["score"] = json!(score);
                v["snippet"] = json!(text::build_snippet(&r.evidence.content, q));
                v
            })
            .collect();
        Ok(
            json!({"results":result,"mode":if query_vector.is_ok(){"hybrid"}else{"keyword"},"degraded":query_vector.err(),"index_stale":stale,"graph":"not_enabled_for_envelope_corpus"}),
        )
    }
    fn jobs(&self, project: &str, grants: &[String]) -> Result<Value, String> {
        let db = self.db.lock().unwrap();
        let mut stmt=db.prepare("SELECT id,payload,status,phase,attempts,error,updated_at,json_array_length(vectors) FROM jobs WHERE project=? ORDER BY id DESC LIMIT 100").map_err(storage)?;
        let mut out = Vec::new();
        for row in stmt
            .query_map([project], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, i64>(7)?,
                ))
            })
            .map_err(storage)?
        {
            let (id, payload, status, phase, attempts, error, at, vectors) =
                row.map_err(storage)?;
            let e: Evidence = serde_json::from_str(&payload).map_err(|_| "job_invalid")?;
            if allows(&e.requirements, grants) {
                out.push(json!({"id":id,"identity":e.identity,"title":e.title,"status":status,"phase":phase,"attempts":attempts,"error":error,"updated_at":at,"vectors_checkpointed":vectors}))
            }
        }
        Ok(json!({"jobs":out}))
    }
    fn control(&self, project: &str, input: &Value, grants: &[String]) -> Result<Value, String> {
        let id = input["job_id"].as_i64().ok_or("job_missing")?;
        let action = input["action"].as_str().ok_or("action_missing")?;
        let db = self.db.lock().unwrap();
        let (payload, status): (String, String) = db
            .query_row(
                "SELECT payload,status FROM jobs WHERE id=? AND project=?",
                params![id, project],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| "job_not_found")?;
        let e: Evidence = serde_json::from_str(&payload).map_err(|_| "job_invalid")?;
        if !allows(&e.requirements, grants) {
            return Err("forbidden".into());
        }
        let next = match (action, status.as_str()) {
            ("pause", "pending" | "processing") => "paused",
            ("cancel", "pending" | "processing" | "paused" | "failed") => "cancelled",
            ("resume", "paused") | ("retry", "failed") => "pending",
            _ => return Err("job_transition_invalid".into()),
        };
        db.execute("UPDATE jobs SET status=?,error=NULL,next_at=0,attempts=CASE WHEN ?='retry' THEN 0 ELSE attempts END,updated_at=? WHERE id=?",params![next,action,chrono::Utc::now().to_rfc3339(),id]).map_err(storage)?;
        Ok(
            json!({"job_id":id,"status":next,"control":"takes effect before next checkpoint or commit"}),
        )
    }
    pub fn spawn_worker(&self) {
        let store = self.clone();
        thread::spawn(move || loop {
            match store.work_once() {
                Ok(true) => {}
                Ok(false) => thread::sleep(Duration::from_millis(300)),
                Err(_) => {
                    eprintln!("wiki worker storage operation failed");
                    thread::sleep(Duration::from_secs(1))
                }
            }
        });
    }
    pub fn work_once(&self) -> Result<bool, String> {
        let job = {
            let db = self.db.lock().unwrap();
            let row:Option<(i64,String,String,String,String,String,String)>=db.query_row("SELECT id,project,revision,payload,phase,analysis,vectors FROM jobs WHERE status='pending' AND next_at<=? ORDER BY id LIMIT 1",[now()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional().map_err(storage)?;
            if let Some(ref row) = row {
                db.execute("UPDATE jobs SET status='processing',attempts=attempts+1,updated_at=? WHERE id=?",params![chrono::Utc::now().to_rfc3339(),row.0]).map_err(storage)?;
            };
            row
        };
        let Some((id, project, revision, payload, _phase, analysis, vectors)) = job else {
            return Ok(false);
        };
        let e: Evidence = serde_json::from_str(&payload).map_err(|_| "job_invalid")?;
        let result = self.process(id, &project, &revision, e, analysis, vectors);
        if let Err(error) = result {
            let db = self.db.lock().unwrap();
            let attempts: i64 = db
                .query_row("SELECT attempts FROM jobs WHERE id=?", [id], |r| r.get(0))
                .map_err(storage)?;
            let status = if attempts >= 3 || error == "provider_authentication" {
                "failed"
            } else {
                "pending"
            };
            db.execute("UPDATE jobs SET status=?,error=?,next_at=?,updated_at=? WHERE id=? AND status='processing'",params![status,error,now()+2i64.pow(attempts as u32),chrono::Utc::now().to_rfc3339(),id]).map_err(storage)?;
        }
        Ok(true)
    }
    fn processing(&self, id: i64) -> Result<bool, String> {
        let db = self.db.lock().unwrap();
        db.query_row(
            "SELECT status='processing' FROM jobs WHERE id=?",
            [id],
            |r| r.get(0),
        )
        .map_err(storage)
    }
    fn process(
        &self,
        id: i64,
        project: &str,
        revision: &str,
        e: Evidence,
        analysis: String,
        vectors: String,
    ) -> Result<(), String> {
        let parts = chunks(&e.content, 7000);
        let mut summaries: Vec<String> =
            serde_json::from_str(&analysis).map_err(|_| "checkpoint_invalid")?;
        for part in parts.iter().skip(summaries.len()) {
            if !self.processing(id)? {
                return Ok(());
            }
            let input = format!(
                "Evidence category: {}\nSource date: {}\nTitle: {}\n\n{}",
                e.evidence_category,
                e.source_date.as_deref().unwrap_or("unknown"),
                e.title,
                part
            );
            // Chunk size leaves space for provenance within the generation request limit.
            let summary = self.provider.summarize(&input)?;
            summaries.push(summary);
            let db = self.db.lock().unwrap();
            db.execute(
                "UPDATE jobs SET analysis=?,updated_at=? WHERE id=? AND status='processing'",
                params![
                    serde_json::to_string(&summaries).unwrap(),
                    chrono::Utc::now().to_rfc3339(),
                    id
                ],
            )
            .map_err(storage)?;
        }
        let analysis = summaries.join("\n\n");
        let texts = chunks(
            &format!("{}\n{}\n{}", e.title, e.content, analysis),
            CHUNK_CHARS,
        );
        let mut vectors: Vec<Vec<f32>> =
            serde_json::from_str(&vectors).map_err(|_| "checkpoint_invalid")?;
        if vectors.len() > texts.len() {
            return Err("checkpoint_invalid".into());
        }
        {
            let db = self.db.lock().unwrap();
            db.execute(
                "UPDATE jobs SET phase='embedding' WHERE id=? AND status='processing'",
                [id],
            )
            .map_err(storage)?;
        }
        while vectors.len() < texts.len() {
            if !self.processing(id)? {
                return Ok(());
            }
            let end = (vectors.len() + 10).min(texts.len());
            vectors.extend(self.provider.embed(&texts[vectors.len()..end])?);
            let db = self.db.lock().unwrap();
            db.execute(
                "UPDATE jobs SET vectors=?,updated_at=? WHERE id=? AND status='processing'",
                params![
                    serde_json::to_string(&vectors).unwrap(),
                    chrono::Utc::now().to_rfc3339(),
                    id
                ],
            )
            .map_err(storage)?;
        }
        let r = Revision {
            evidence: e.clone(),
            analysis,
            chunks: texts,
            vectors,
            profile: self.provider.profile.clone(),
            indexed_at: chrono::Utc::now().to_rfc3339(),
        };
        {
            let mut db = self.db.lock().unwrap();
            let tx = db.transaction().map_err(storage)?;
            let status: String = tx
                .query_row("SELECT status FROM jobs WHERE id=?", [id], |r| r.get(0))
                .map_err(storage)?;
            if status != "processing" {
                return Ok(());
            }
            let newer:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE project=? AND identity=? AND id>? AND status='done')",params![project,e.identity,id],|r|r.get(0)).map_err(storage)?;
            if newer {
                tx.execute(
                    "UPDATE jobs SET status='cancelled',error='superseded_revision' WHERE id=?",
                    [id],
                )
                .map_err(storage)?;
                tx.commit().map_err(storage)?;
                return Ok(());
            }
            tx.execute(
                "UPDATE revisions SET active=0 WHERE project=? AND identity=?",
                params![project, e.identity],
            )
            .map_err(storage)?;
            tx.execute("INSERT INTO revisions(project,identity,revision,profile,payload,active) VALUES(?,?,?,?,?,1) ON CONFLICT(project,identity,revision,profile) DO UPDATE SET payload=excluded.payload,active=1,revoked=0",params![project,e.identity,revision,self.provider.profile,serde_json::to_string(&r).unwrap()]).map_err(storage)?;
            tx.execute("INSERT INTO checkpoints VALUES(?,?,?) ON CONFLICT(project,identity) DO UPDATE SET revision=excluded.revision",params![project,e.identity,revision]).map_err(storage)?;
            tx.execute("UPDATE jobs SET status='done',phase='committed',error=NULL,updated_at=? WHERE id=?",params![chrono::Utc::now().to_rfc3339(),id]).map_err(storage)?;
            tx.commit().map_err(storage)?;
        }
        self.materialize(project, revision, &r);
        Ok(())
    }
    fn materialize(&self, project: &str, revision: &str, r: &Revision) {
        let root = Path::new(
            &self
                .config
                .projects
                .iter()
                .find(|p| p.id == project)
                .unwrap()
                .root,
        );
        let dir = root.join("wiki/server-pages");
        let target = dir.join(format!("{revision}.md"));
        let pending = dir.join(format!("{revision}.pending"));
        let body=format!("# {}\n\n## AI analysis\n\n{}\n\n## Original evidence\n\n{}\n\n## Provenance\n\n```json\n{}\n```\n",r.evidence.title,r.analysis,r.evidence.content,Self::public_revision(revision,r));
        let result = (|| {
            use std::io::Write;
            if pending.exists() || fs::symlink_metadata(&pending).is_ok() {
                fs::remove_file(&pending)?;
            }
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&pending)?;
            f.set_permissions(fs::Permissions::from_mode(0o600))?;
            f.write_all(body.as_bytes())?;
            f.sync_all()?;
            fs::rename(&pending, &target)?;
            fs::File::open(&dir)?.sync_all()
        })();
        if result.is_err() {
            eprintln!("wiki Markdown projection unavailable; committed database retained")
        }
    }
    pub fn repair_projections(&self) -> Result<(), String> {
        let db = self.db.lock().unwrap();
        let mut stmt = db
            .prepare("SELECT project,revision,payload FROM revisions")
            .map_err(storage)?;
        for row in stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(storage)?
        {
            let (p, id, payload) = row.map_err(storage)?;
            if self.config.projects.iter().any(|s| s.id == p) {
                let r: Revision = serde_json::from_str(&payload).map_err(|_| "revision_invalid")?;
                self.materialize(&p, &id, &r)
            }
        }
        Ok(())
    }
}
fn cosine(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() {
        return -1.;
    }
    let (dot, aa, bb) = a.iter().zip(b).fold((0., 0., 0.), |(dot, aa, bb), (x, y)| {
        (
            dot + (*x as f64) * (*y as f64),
            aa + (*x as f64).powi(2),
            bb + (*y as f64).powi(2),
        )
    });
    if aa == 0. || bb == 0. {
        -1.
    } else {
        dot / (aa * bb).sqrt()
    }
}
