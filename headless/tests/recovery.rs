use llm_wiki_core::{
    engine::Config,
    provider::{Provider, DIMENSIONS},
    store::{Evidence, Store},
};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
};
use tempfile::TempDir;

struct Mock {
    calls: Arc<Mutex<Vec<usize>>>,
    generation: Arc<AtomicUsize>,
    fail_at: Arc<AtomicUsize>,
    address: String,
}
fn mock() -> Mock {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let address = format!("http://{}", server.server_addr());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let generation = Arc::new(AtomicUsize::new(0));
    let fail_at = Arc::new(AtomicUsize::new(usize::MAX));
    let (c, g, f) = (calls.clone(), generation.clone(), fail_at.clone());
    thread::spawn(move || {
        for mut r in server.incoming_requests() {
            let mut text = String::new();
            r.as_reader().read_to_string(&mut text).unwrap();
            let v: Value = serde_json::from_str(&text).unwrap();
            let mut status = 200;
            let body = if r.url().ends_with("/embeddings") {
                let n = v["input"].as_array().unwrap().len();
                assert!(n <= 10);
                assert_eq!(v["dimensions"], 1024);
                let mut calls = c.lock().unwrap();
                calls.push(n);
                if calls.len() == f.load(Ordering::SeqCst) {
                    status = 503;
                    json!({"error":"synthetic provider error must not be propagated"})
                } else {
                    json!({"data":(0..n).map(|i|json!({"index":i,"embedding":vec![1.;DIMENSIONS]})).collect::<Vec<_>>()})
                }
            } else {
                g.fetch_add(1, Ordering::SeqCst);
                json!({"choices":[{"message":{"content":"Synthetic summary. 测试环境端口 8088，保留未知验收状态。"},"finish_reason":"stop"}]})
            };
            r.respond(tiny_http::Response::from_string(body.to_string()).with_status_code(status))
                .unwrap();
        }
    });
    Mock {
        calls,
        generation,
        fail_at,
        address,
    }
}
fn fixture(mock: &Mock) -> (TempDir, Config, Provider) {
    let d = TempDir::new().unwrap();
    let model = d.path().join("model.env");
    fs::write(&model,format!("SERVER={}\nKEY=synthetic-private-key\nLLM_MODEL=qwen-plus\nEMBEDDING_MODEL=text-embedding-v4\n",mock.address)).unwrap();
    fs::set_permissions(&model, fs::Permissions::from_mode(0o600)).unwrap();
    let c:Config=serde_json::from_value(json!({"socket":d.path().join("wiki.sock"),"database":d.path().join("wiki.sqlite3"),"model_file":model,"allow_local_provider":true,"projects":[{"id":"demo","name":"Demo","root":d.path().join("project"),"requirements":["project:demo"]}]})).unwrap();
    let p = Provider::load(&c.model_file, true).unwrap();
    (d, c, p)
}
fn evidence(content: String) -> Evidence {
    Evidence {
        identity: "doc-1".into(),
        title: "Acceptance example".into(),
        content,
        source: json!({"path":"docs/demo.md","record_id":"doc-1"}),
        source_date: None,
        evidence_category: "executor_self_report".into(),
        requirements: vec!["project:demo".into()],
    }
}
fn input(operation: &str) -> Value {
    json!({"operation":operation,"project":"demo","grants":["project:demo"]})
}

#[test]
fn managed_work_selects_one_job_and_preserves_attempts_after_revocation() {
    let mock = mock();
    let (_d, c, p) = fixture(&mock);
    let store = Store::open(c.clone(), p.clone())
        .unwrap()
        .preserve_job_attempts();
    let old = store
        .submit("demo", evidence("Historical backlog".into()))
        .unwrap()["job_id"]
        .as_i64()
        .unwrap();
    let mut current = evidence("Explicitly authorized document".into());
    current.identity = "authorized".into();
    let selected = store.submit("demo", current).unwrap()["job_id"]
        .as_i64()
        .unwrap();
    // Revoke after the provider request, before its result can be checkpointed.
    let requested = Arc::new(AtomicUsize::new(0));
    let seen = requested.clone();
    let guarded = store.with_work_guard(Arc::new(move |phase| {
        if phase.starts_with("model:") {
            seen.fetch_add(1, Ordering::SeqCst);
        } else if seen.load(Ordering::SeqCst) > 0 {
            return Err("authorization_revoked".into());
        }
        Ok(())
    }));
    let result = guarded.work_job(selected).unwrap();
    assert_eq!(result["state"], "pending");
    assert_eq!(result["attempts"], 1);
    assert_eq!(result["error"], "authorization_revoked");
    assert_eq!(mock.generation.load(Ordering::SeqCst), 1);
    assert!(mock.calls.lock().unwrap().is_empty());
    {
        let db = store.db.lock().unwrap();
        let old_attempts: i64 = db
            .query_row("SELECT attempts FROM jobs WHERE id=?", [old], |r| r.get(0))
            .unwrap();
        let revisions: i64 = db
            .query_row("SELECT COUNT(*) FROM revisions", [], |r| r.get(0))
            .unwrap();
        assert_eq!((old_attempts, revisions), (0, 0));
        db.execute("UPDATE jobs SET status='failed' WHERE id=?", [selected])
            .unwrap();
    }
    drop(guarded);
    drop(store);
    let recovered = Store::open(c, p).unwrap().preserve_job_attempts();
    let mut retry = input("control");
    retry["job_id"] = json!(selected);
    retry["action"] = json!("retry");
    recovered.handle(&retry).unwrap();
    let completed = recovered.work_job(selected).unwrap();
    assert_eq!(completed["state"], "done");
    assert_eq!(completed["attempts"], 2);
    assert_eq!(
        recovered.handle(&input("pages")).unwrap()["pages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let old_attempts: i64 = recovered
        .db
        .lock()
        .unwrap()
        .query_row("SELECT attempts FROM jobs WHERE id=?", [old], |r| r.get(0))
        .unwrap();
    assert_eq!(old_attempts, 0);
}

#[test]
fn managed_repair_is_bounded_idempotent_and_guarded() {
    let mock = mock();
    let (_d, c, p) = fixture(&mock);
    let store = Store::open(c.clone(), p).unwrap();
    for n in 0..2 {
        let mut e = evidence(format!("Document {n}"));
        e.identity = format!("document-{n}");
        let id = store.submit("demo", e).unwrap()["job_id"].as_i64().unwrap();
        assert_eq!(store.work_job(id).unwrap()["state"], "done");
    }
    let directory = std::path::Path::new(&c.projects[0].root).join("wiki/server-pages");
    for path in fs::read_dir(&directory).unwrap() {
        fs::remove_file(path.unwrap().path()).unwrap();
    }
    let denied = store.with_work_guard(Arc::new(|_| Err("authorization_revoked".into())));
    assert_eq!(
        denied.repair_project_page("demo", "", 1).unwrap_err(),
        "authorization_revoked"
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    let first = store.repair_project_page("demo", "", 1).unwrap();
    assert_eq!(
        (first["checked"].as_u64(), first["written"].as_u64()),
        (Some(1), Some(1))
    );
    let second = store
        .repair_project_page("demo", first["cursor"].as_str().unwrap(), 1)
        .unwrap();
    assert_eq!(second["written"], 1);
    let again = store.repair_project_page("demo", "", 2).unwrap();
    assert_eq!(again["written"], 0);
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
}

#[test]
fn externally_scheduled_reindex_requires_and_obeys_a_batch_limit() {
    let mock = mock();
    let (_d, c, p) = fixture(&mock);
    let store = Store::open(c, p).unwrap();
    for n in 0..2 {
        let mut e = evidence(format!("Evidence {n}"));
        e.identity = format!("doc-{n}");
        let id = store.submit("demo", e).unwrap()["job_id"].as_i64().unwrap();
        store.work_job(id).unwrap();
    }
    let mut managed = store.externally_scheduled();
    managed.provider.profile = "synthetic-new-profile".into();
    let mut request = input("reindex");
    assert_eq!(
        managed.handle(&request).unwrap_err(),
        "background_batch_required"
    );
    request["limit"] = json!(1);
    let output = managed.handle(&request).unwrap();
    assert_eq!(output["jobs"].as_array().unwrap().len(), 1);
    assert_eq!(output["deferred_items"], 1);
    assert_eq!(mock.generation.load(Ordering::SeqCst), 2);
    let count: i64 = managed
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM jobs WHERE status='pending'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn externally_scheduled_attempt_limit_is_effective_and_not_reset_on_retry() {
    let mock = mock();
    mock.fail_at.store(1, Ordering::SeqCst);
    let (_d, c, p) = fixture(&mock);
    let store = Store::open(c.clone(), p.clone())
        .unwrap()
        .externally_scheduled();
    let id = store
        .submit("demo", evidence("Bounded retry evidence".into()))
        .unwrap()["job_id"]
        .as_i64()
        .unwrap();
    let failed = store.work_job_with_limit(id, 1).unwrap();
    assert_eq!(failed["state"], "failed");
    assert_eq!(failed["attempts"], 1);
    drop(store);
    let recovered = Store::open(c, p).unwrap().externally_scheduled();
    let mut retry = input("control");
    retry["job_id"] = json!(id);
    retry["action"] = json!("retry");
    recovered.handle(&retry).unwrap();
    let exhausted = recovered.work_job_with_limit(id, 1).unwrap();
    assert_eq!(exhausted["state"], "failed");
    assert_eq!(exhausted["attempts"], 1);
    assert_eq!(exhausted["error"], "attempts_exhausted");
    assert_eq!(mock.calls.lock().unwrap().len(), 1);
}

#[test]
fn prepared_metadata_summary_replaces_generated_inference_without_losing_history() {
    let mock = mock();
    let (_d, c, p) = fixture(&mock);
    let s = Store::open(c.clone(), p.clone()).unwrap();
    let mut e = evidence("Only an artifact name is known; description missing".into());
    let first = s.submit("demo", e.clone()).unwrap();
    s.work_once().unwrap();
    assert_eq!(mock.generation.load(Ordering::SeqCst), 1);
    let summary = "原始说明为空，用途未知；不依据名称推断。";
    e.source["processing_policy"] = json!({"mode":"metadata_only","summary":summary});
    let revised = s.submit("demo", e.clone()).unwrap();
    assert_ne!(first["job_id"], revised["job_id"]);
    s.work_once().unwrap();
    assert_eq!(mock.generation.load(Ordering::SeqCst), 1);
    assert_eq!(mock.calls.lock().unwrap().len(), 2);
    let pages = s.handle(&input("pages")).unwrap();
    let mut read = input("read");
    read["page_id"] = pages["pages"][0]["page_id"].clone();
    let current = s.handle(&read).unwrap();
    assert_eq!(current["analysis"], summary);
    assert_eq!(current["analysis_category"], "source_metadata_summary");
    assert_eq!(current["original"], e.content);
    assert_eq!(
        s.db.lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM revisions", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    drop(s);
    let recovered = Store::open(c, p).unwrap();
    assert_eq!(
        recovered.submit("demo", e.clone()).unwrap()["job_id"],
        revised["job_id"]
    );
    assert_eq!(
        recovered.handle(&read).unwrap()["analysis_category"],
        "source_metadata_summary"
    );
    e.source["processing_policy"]["summary"] = json!("");
    assert!(recovered.submit("demo", e).is_err());
}

#[test]
fn batches_recovery_and_atomic_checkpoint() {
    let mock = mock();
    let (_d, c, p) = fixture(&mock);
    let s = Store::open(c.clone(), p.clone()).unwrap();
    let e = evidence("测试环境 8088. Deployment evidence unknown.\n".repeat(310));
    let first = s.submit("demo", e.clone()).unwrap();
    let same = s.submit("demo", e).unwrap();
    assert_eq!(first["job_id"], same["job_id"]);
    mock.fail_at.store(2, Ordering::SeqCst);
    s.work_once().unwrap();
    assert_eq!(
        s.handle(&input("pages")).unwrap()["pages"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        s.db.lock()
            .unwrap()
            .query_row("SELECT count(*) FROM checkpoints", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let saved = s.handle(&input("jobs")).unwrap();
    assert_eq!(saved["jobs"][0]["vectors_checkpointed"], 10);
    let generated = mock.generation.load(Ordering::SeqCst);
    s.db.lock()
        .unwrap()
        .execute("UPDATE jobs SET status='processing',next_at=0", [])
        .unwrap();
    drop(s);
    let recovered = Store::open(c, p).unwrap();
    recovered.work_once().unwrap();
    assert_eq!(mock.generation.load(Ordering::SeqCst), generated);
    assert_eq!(
        recovered.handle(&input("jobs")).unwrap()["jobs"][0]["status"],
        "done"
    );
    assert_eq!(
        recovered.handle(&input("pages")).unwrap()["pages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        recovered
            .db
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM checkpoints", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(mock.calls.lock().unwrap().iter().all(|n| *n <= 10));
    assert!(mock.calls.lock().unwrap().len() >= 3);
}
#[test]
fn scoped_read_revision_revoke_and_path_boundary() {
    let mock = mock();
    let (_d, c, p) = fixture(&mock);
    let s = Store::open(c, p).unwrap();
    s.submit("demo", evidence("Evidence v1".into())).unwrap();
    s.work_once().unwrap();
    let mut hidden = input("pages");
    hidden["grants"] = json!([]);
    assert!(s.handle(&hidden).is_err());
    let mut read = input("read");
    read["page_id"] = json!("../../model.env");
    assert!(s.handle(&read).is_err());
    let old = s.handle(&input("pages")).unwrap()["pages"][0]["page_id"].clone();
    s.submit("demo", evidence("Evidence v2".into())).unwrap();
    s.work_once().unwrap();
    assert_ne!(
        s.handle(&input("pages")).unwrap()["pages"][0]["page_id"],
        old
    );
    read["page_id"] = old;
    assert_eq!(s.handle(&read).unwrap()["revision_status"], "superseded");
    assert_eq!(
        s.db.lock()
            .unwrap()
            .query_row("SELECT count(*) FROM revisions", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    let mut revoke = input("revoke");
    revoke["identity"] = json!("doc-1");
    s.handle(&revoke).unwrap();
    assert_eq!(s.handle(&input("pages")).unwrap()["pages"], json!([]));
    assert!(s.handle(&read).is_err());
    assert!(s.submit("demo", evidence("Evidence v3".into())).is_err());
}
#[test]
fn persistent_pause_cancel_and_bounded_failure() {
    let mock = mock();
    let (_d, c, p) = fixture(&mock);
    let s = Store::open(c, p).unwrap();
    let id = s.submit("demo", evidence("Synthetic task".into())).unwrap()["job_id"].clone();
    let mut control = input("control");
    control["job_id"] = id;
    control["action"] = json!("pause");
    s.handle(&control).unwrap();
    assert!(!s.work_once().unwrap());
    control["action"] = json!("resume");
    s.handle(&control).unwrap();
    control["action"] = json!("cancel");
    s.handle(&control).unwrap();
    assert!(!s.work_once().unwrap());
    assert_eq!(
        s.handle(&input("jobs")).unwrap()["jobs"][0]["status"],
        "cancelled"
    );
    s.submit("demo", evidence("Synthetic task revised".into()))
        .unwrap();
    for _ in 0..3 {
        mock.fail_at
            .store(mock.calls.lock().unwrap().len() + 1, Ordering::SeqCst);
        s.db.lock()
            .unwrap()
            .execute("UPDATE jobs SET next_at=0", [])
            .unwrap();
        s.work_once().unwrap();
    }
    let failed = s.handle(&input("jobs")).unwrap();
    assert_eq!(failed["jobs"][0]["status"], "failed");
    assert_eq!(failed["jobs"][0]["attempts"], 3);
    assert!(!failed.to_string().contains("synthetic provider error"));
    assert!(!s.work_once().unwrap());
}

#[test]
fn source_reversion_republishes_cached_evidence_without_model_calls() {
    let mock = mock();
    let (_dir, c, p) = fixture(&mock);
    let store = Store::open(c.clone(), p.clone()).unwrap();
    let old = store
        .submit("demo", evidence("Source version one".into()))
        .unwrap();
    store.work_once().unwrap();
    store
        .submit("demo", evidence("Source version two".into()))
        .unwrap();
    store.work_once().unwrap();
    store
        .submit("demo", evidence("Pending version three".into()))
        .unwrap();
    let generation = mock.generation.load(Ordering::SeqCst);
    let embeddings = mock.calls.lock().unwrap().len();
    let reverted = store
        .submit("demo", evidence("Source version one".into()))
        .unwrap();
    assert_eq!(reverted["status"], "done");
    assert_eq!(reverted["revision"], old["revision"]);
    assert_eq!(
        store.handle(&input("pages")).unwrap()["pages"][0]["page_id"],
        old["revision"]
    );
    assert!(!store.work_once().unwrap());
    drop(store);
    let recovered = Store::open(c, p).unwrap();
    assert_eq!(
        recovered.handle(&input("pages")).unwrap()["pages"][0]["page_id"],
        old["revision"]
    );
    assert_eq!(mock.generation.load(Ordering::SeqCst), generation);
    assert_eq!(mock.calls.lock().unwrap().len(), embeddings);
}

#[test]
fn source_scope_precedes_ranking_counts_and_observation_is_not_a_revision() {
    let mock = mock();
    let (_dir, mut c, p) = fixture(&mock);
    c.extensions = json!({"presentation":{"projects":{"demo":{
        "kind":"synthetic","description":"Synthetic coverage only",
        "suggested_questions":["What was actually verified?"],
        "private_import_path":"/private/do-not-publish"
    }}}});
    let store = Store::open(c, p).unwrap();
    let projects = store.handle(&input("projects")).unwrap();
    assert_eq!(projects["projects"][0]["kind"], "synthetic");
    assert_eq!(
        projects["projects"][0]["description"],
        "Synthetic coverage only"
    );
    assert!(!projects.to_string().contains("do-not-publish"));
    let mut denied = input("projects");
    denied["grants"] = json!([]);
    assert_eq!(store.handle(&denied).unwrap()["projects"], json!([]));
    let mut e = evidence("Restricted synthetic evidence".into());
    e.requirements.push("meta:secret-meta".into());
    e.source["observed_at"] = json!("2026-10-07T01:00:00Z");
    let first = store.submit("demo", e.clone()).unwrap();
    e.source["observed_at"] = json!("2026-10-07T02:00:00Z");
    assert_eq!(first["job_id"], store.submit("demo", e).unwrap()["job_id"]);
    store.work_once().unwrap();
    assert_eq!(store.handle(&input("pages")).unwrap()["pages"], json!([]));
    assert_eq!(store.handle(&input("jobs")).unwrap()["jobs"], json!([]));
    assert_eq!(store.handle(&input("status")).unwrap()["pages"], 0);
    let mut search = input("search");
    search["query"] = json!("Restricted");
    assert_eq!(store.handle(&search).unwrap()["results"], json!([]));
    let mut allowed = input("pages");
    allowed["grants"] = json!(["project:demo", "meta:secret-meta"]);
    let id = store.handle(&allowed).unwrap()["pages"][0]["page_id"].clone();
    let mut read = input("read");
    read["page_id"] = id;
    assert!(store.handle(&read).is_err());
}
