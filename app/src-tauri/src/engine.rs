//! Supervises the headless CloudStream engine (a JVM sidecar) and forwards API calls to it.

use std::{collections::HashMap, path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::{Child, Command},
    sync::Mutex,
};

const READY_PREFIX: &str = "ENGINE_READY port=";

/// Small-footprint JVM: serial GC that gives memory back (the engine also forces a GC when idle),
/// C1-only JIT with a small code cache, capped heap/metaspace, and no perf-data mapping.
const JVM_ARGS: [&str; 11] = [
    "-XX:+UseSerialGC",
    "-Xms16m",
    "-Xmx256m",
    "-XX:MinHeapFreeRatio=10",
    "-XX:MaxHeapFreeRatio=25",
    "-XX:MaxMetaspaceSize=192m",
    "-XX:TieredStopAtLevel=1",
    "-XX:ReservedCodeCacheSize=48m",
    "-Xss1m",
    "-XX:-UsePerfData",
    "-Xshare:auto",
];
const STARTUP_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum EngineStatus {
    Starting,
    Ready { port: u16 },
    Failed { message: String },
    Stopped,
}

pub struct Engine {
    token: String,
    http: reqwest::Client,
    /// Serializes start/restart; `inner` stays free for status queries meanwhile.
    start_lock: Mutex<()>,
    inner: Mutex<Inner>,
}

struct Inner {
    child: Option<Child>,
    status: EngineStatus,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            token: uuid::Uuid::new_v4().simple().to_string(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("http client"),
            start_lock: Mutex::new(()),
            inner: Mutex::new(Inner { child: None, status: EngineStatus::Stopped }),
        }
    }

    pub async fn status(&self) -> EngineStatus {
        self.inner.lock().await.status.clone()
    }

    /// (Re)starts the engine and waits until it reports its port.
    pub async fn start(self: &Arc<Self>, app: &AppHandle) {
        let _starting = self.start_lock.lock().await;
        {
            let mut inner = self.inner.lock().await;
            if let Some(mut child) = inner.child.take() {
                let _ = child.kill().await;
            }
            inner.status = EngineStatus::Starting;
            let _ = app.emit("engine-status", &inner.status);
        }

        let result = self.spawn(app).await;
        let mut inner = self.inner.lock().await;
        inner.status = match result {
            Ok((child, port)) => {
                inner.child = Some(child);
                EngineStatus::Ready { port }
            }
            Err(message) => EngineStatus::Failed { message },
        };
        let _ = app.emit("engine-status", &inner.status);
    }

    pub async fn stop(&self) {
        let mut inner = self.inner.lock().await;
        if let Some(mut child) = inner.child.take() {
            let _ = child.kill().await;
        }
        inner.status = EngineStatus::Stopped;
    }

    async fn spawn(self: &Arc<Self>, app: &AppHandle) -> Result<(Child, u16), String> {
        let jar = find_jar(app).ok_or_else(|| {
            "engine.jar not found. Build it with `./gradlew shadowJar` in engine/ or set CS_ENGINE_JAR.".to_string()
        })?;
        let java = find_java(app);
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("engine");

        let mut cmd = Command::new(&java);
        cmd.args(JVM_ARGS)
            .arg("-jar")
            .arg(&jar)
            .env("CS_ENGINE_DATA", &data_dir)
            .env("CS_ENGINE_TOKEN", &self.token)
            .env("CS_ENGINE_WATCH_STDIN", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Could not start Java ({}): {e}. Is a Java 17+ runtime installed?", java.display()))?;

        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        tokio::spawn(drain("engine:err", stderr));

        let mut lines = BufReader::new(stdout).lines();
        let wait_ready = async {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(port) = line.strip_prefix(READY_PREFIX) {
                    return port.trim().parse::<u16>().map_err(|e| e.to_string());
                }
                println!("[engine] {line}");
            }
            Err("engine exited during startup (see log output)".to_string())
        };
        let port = match tokio::time::timeout(STARTUP_TIMEOUT, wait_ready).await {
            Ok(Ok(port)) => port,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("engine did not become ready in time".into()),
        };

        // Keep draining stdout; flag the engine as failed if it dies while we think it is ready.
        let this = Arc::clone(self);
        let app = app.clone();
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                println!("[engine] {line}");
            }
            let mut inner = this.inner.lock().await;
            if matches!(inner.status, EngineStatus::Ready { port: p } if p == port) {
                inner.child = None;
                inner.status = EngineStatus::Failed { message: "engine process exited".into() };
                let _ = app.emit("engine-status", &inner.status);
            }
        });

        Ok((child, port))
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        query: Option<HashMap<String, String>>,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let port = match self.status().await {
            EngineStatus::Ready { port } => port,
            EngineStatus::Starting => return Err("Engine is still starting".into()),
            EngineStatus::Failed { message } => return Err(format!("Engine unavailable: {message}")),
            EngineStatus::Stopped => return Err("Engine is stopped".into()),
        };
        if !path.starts_with('/') {
            return Err("path must start with '/'".into());
        }
        let method = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
        let mut req = self
            .http
            .request(method, format!("http://127.0.0.1:{port}{path}"))
            .header("X-Engine-Token", &self.token);
        if let Some(query) = query {
            req = req.query(&query);
        }
        if let Some(body) = body {
            req = req.json(&body);
        }
        let res = req.send().await.map_err(|e| e.to_string())?;
        let status = res.status();
        let value: Value = res.json().await.map_err(|e| format!("Bad engine response ({status}): {e}"))?;
        if status.is_success() {
            Ok(value)
        } else {
            Err(value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| status.to_string()))
        }
    }
}

async fn drain(tag: &'static str, reader: impl AsyncRead + Unpin) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        eprintln!("[{tag}] {line}");
    }
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Bundled JRE first, then JAVA_HOME, then whatever `java` is on PATH.
fn find_java(app: &AppHandle) -> PathBuf {
    let java = exe("java");
    if let Ok(res) = app.path().resource_dir() {
        let bundled = res.join("jre").join("bin").join(&java);
        if bundled.exists() {
            return bundled;
        }
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let p = PathBuf::from(home).join("bin").join(&java);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(java)
}

fn find_jar(app: &AppHandle) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CS_ENGINE_JAR") {
        return Some(PathBuf::from(p));
    }
    if let Ok(res) = app.path().resource_dir() {
        let bundled = res.join("engine").join("engine.jar");
        if bundled.exists() {
            return Some(bundled);
        }
    }
    if cfg!(debug_assertions) {
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../engine/build/libs/engine.jar");
        if dev.exists() {
            return Some(dev);
        }
    }
    None
}
