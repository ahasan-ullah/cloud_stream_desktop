//! Supervises the headless CloudStream engine (a JVM sidecar) and forwards API calls to it.

use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, BufReader},
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
        let data_dir = match std::env::var_os("CS_ENGINE_DATA") {
            Some(dir) => PathBuf::from(dir),
            None => app.path().app_data_dir().map_err(|e| e.to_string())?.join("engine"),
        };
        std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
        let log = Arc::new(EngineLog::create(data_dir.join("engine.log")));

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
        let stderr_done = tokio::spawn(drain("engine:err", stderr, Arc::clone(&log)));

        let mut lines = BufReader::new(stdout);
        let wait_ready = async {
            while let Some(line) = read_line(&mut lines).await {
                log.write(&line);
                if let Some(port) = line.strip_prefix(READY_PREFIX) {
                    return port.trim().parse::<u16>().map_err(|e| e.to_string());
                }
                println!("[engine] {line}");
            }
            let _ = stderr_done.await;
            Err(log.startup_failure())
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
            while let Some(line) = read_line(&mut lines).await {
                log.write(&line);
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

async fn drain(tag: &'static str, reader: impl AsyncRead + Unpin, log: Arc<EngineLog>) {
    let mut lines = BufReader::new(reader);
    while let Some(line) = read_line(&mut lines).await {
        log.write(&line);
        eprintln!("[{tag}] {line}");
    }
}

/// Reads a line, replacing invalid UTF-8 rather than stopping: the output isn't guaranteed to be
/// UTF-8, and a stream that stopped being read would look like an engine exit and could block it.
async fn read_line(reader: &mut (impl AsyncBufRead + Unpin)) -> Option<String> {
    let mut bytes = Vec::new();
    match reader.read_until(b'\n', &mut bytes).await {
        Ok(n) if n > 0 => Some(String::from_utf8_lossy(&bytes).trim_end().to_string()),
        _ => None,
    }
}

/// The engine's output for this run. Installed apps have no console, so this is the only record.
struct EngineLog {
    path: PathBuf,
    file: std::sync::Mutex<Option<File>>,
    last_error: std::sync::Mutex<Option<String>>,
}

impl EngineLog {
    fn create(path: PathBuf) -> Self {
        let file = std::sync::Mutex::new(File::create(&path).ok());
        Self { path, file, last_error: std::sync::Mutex::new(None) }
    }

    fn write(&self, line: &str) {
        if let Some(file) = self.file.lock().unwrap().as_mut() {
            let _ = writeln!(file, "{line}");
        }
        if line.contains("Error") || line.contains("Exception") {
            *self.last_error.lock().unwrap() = Some(line.trim_matches(['#', ' ']).to_string());
        }
    }

    fn startup_failure(&self) -> String {
        let reason = match self.last_error.lock().unwrap().as_deref() {
            Some(error) => format!(": {error}"),
            None => String::new(),
        };
        format!("engine exited during startup{reason} (log: {})", self.path.display())
    }
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Tauri reports its folders as `\\?\C:\...` on Windows, and Java can't start from such a path.
fn plain_path(path: &Path) -> PathBuf {
    dunce::simplified(path).to_path_buf()
}

/// Bundled JRE first, then JAVA_HOME, then whatever `java` is on PATH.
fn find_java(app: &AppHandle) -> PathBuf {
    let java = exe("java");
    if let Ok(res) = app.path().resource_dir() {
        let bundled = plain_path(&res).join("jre").join("bin").join(&java);
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
        let bundled = plain_path(&res).join("engine").join("engine.jar");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn plain_path_removes_the_prefix_java_rejects() {
        let path = plain_path(Path::new(r"\\?\C:\Users\me\AppData\Local\CloudStream Desktop"));
        assert_eq!(path, PathBuf::from(r"C:\Users\me\AppData\Local\CloudStream Desktop"));
    }

    #[tokio::test]
    async fn read_line_keeps_going_after_invalid_utf8() {
        let mut output: &[u8] = b"caf\xe9\r\nENGINE_READY port=1\n";
        assert_eq!(read_line(&mut output).await.as_deref(), Some("caf\u{fffd}"));
        assert_eq!(read_line(&mut output).await.as_deref(), Some("ENGINE_READY port=1"));
        assert_eq!(read_line(&mut output).await, None);
    }

    #[test]
    fn startup_failure_names_the_error_and_the_log() {
        let dir = std::env::temp_dir().join(format!("engine-log-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("engine.log");
        let log = EngineLog::create(path.clone());
        log.write("[main] INFO PluginHost - Loaded 0 plugin(s)");
        log.write("Error: Unable to access jarfile engine.jar");
        log.write("at the end of the output");
        let message = log.startup_failure();
        assert!(message.contains(": Error: Unable to access jarfile engine.jar"), "{message}");
        assert!(message.contains(path.to_str().unwrap()), "{message}");
        drop(log);
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 3);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
