use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root: String,
    pub requirements: Vec<String>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub socket: String,
    pub database: String,
    pub model_file: String,
    pub projects: Vec<Project>,
    #[serde(default)]
    pub allow_local_provider: bool,
    #[serde(default)]
    pub extensions: Value,
}
pub type Extension = fn(&crate::store::Store, &Value) -> Result<Value, String>;
pub fn allows(required: &[String], grants: &[String]) -> bool {
    required.iter().all(|r| {
        grants.iter().any(|g| {
            g == r
                || g.strip_suffix('*')
                    .is_some_and(|prefix| r.starts_with(prefix))
        })
    })
}
pub fn run_from_args(extension: Option<Extension>) -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 || !matches!(args[1].as_str(), "--config" | "--check-config") {
        return Err("usage: llm-wiki-server --config PRIVATE_JSON".into());
    }
    let raw = fs::read(&args[2]).map_err(|_| "config_unavailable")?;
    let metadata = fs::symlink_metadata(&args[2]).map_err(|_| "config_unavailable")?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
        return Err("config_permissions".into());
    }
    let config: Config = serde_json::from_slice(&raw).map_err(|_| "config_invalid")?;
    if args[1] == "--check-config" {
        crate::provider::Provider::load(&config.model_file, config.allow_local_provider)?;
        println!("wiki_config_valid");
        return Ok(());
    }
    serve(config, extension)
}
pub fn serve(config: Config, extension: Option<Extension>) -> Result<(), String> {
    if !Path::new(&config.database).is_absolute()
        || !Path::new(&config.socket).is_absolute()
        || config.projects.is_empty()
        || config.projects.iter().any(|p| {
            !Path::new(&p.root).is_absolute()
                || p.requirements.is_empty()
                || p.id.is_empty()
                || !p
                    .id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
    {
        return Err("config_paths_invalid".into());
    }
    let parent = Path::new(&config.database)
        .parent()
        .ok_or("database_invalid")?;
    fs::create_dir_all(parent).map_err(|_| "database_invalid")?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join("wiki.lock"))
        .map_err(|_| "database_lock")?;
    use fs2::FileExt;
    lock.try_lock_exclusive()
        .map_err(|_| "wiki_already_running")?;
    let provider =
        crate::provider::Provider::load(&config.model_file, config.allow_local_provider)?;
    let store = crate::store::Store::open(config.clone(), provider)?;
    store.repair_projections()?;
    store.spawn_worker();
    let socket = Path::new(&config.socket);
    fs::create_dir_all(socket.parent().ok_or("socket_invalid")?).map_err(|_| "socket_invalid")?;
    if socket.exists() {
        if std::os::unix::net::UnixStream::connect(socket).is_ok() {
            return Err("socket_in_use".into());
        }
        fs::remove_file(socket).map_err(|_| "socket_in_use")?;
    }
    let server = tiny_http::Server::http_unix(socket).map_err(|_| "socket_bind_failed")?;
    fs::set_permissions(socket, fs::Permissions::from_mode(0o600))
        .map_err(|_| "socket_permissions")?;
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for mut request in server.incoming_requests() {
                    let mut input = String::new();
                    use std::io::Read;
                    let result = if request.method() != &tiny_http::Method::Post
                        || request.url() != "/rpc"
                    {
                        Err("route_denied".to_string())
                    } else if request
                        .as_reader()
                        .take(1024 * 1024 + 1)
                        .read_to_string(&mut input)
                        .is_err()
                        || input.len() > 1024 * 1024
                    {
                        Err("input_too_large".to_string())
                    } else {
                        serde_json::from_str::<Value>(&input)
                            .map_err(|_| "invalid_request".to_string())
                            .and_then(|v| {
                                if matches!(v["operation"].as_str(), Some("import" | "records")) {
                                    if let Some(handler) = extension {
                                        handler(&store, &v)
                                    } else {
                                        Err("operation_denied".into())
                                    }
                                } else {
                                    store.handle(&v)
                                }
                            })
                    };
                    let (status, body) = match result {
                        Ok(v) => (200, v),
                        Err(e) => (400, json!({"error":e})),
                    };
                    let response = tiny_http::Response::from_string(body.to_string())
                        .with_status_code(status)
                        .with_header(
                            tiny_http::Header::from_bytes("Content-Type", "application/json")
                                .unwrap(),
                        );
                    let _ = request.respond(response);
                }
            });
        }
    });
    Ok(())
}
