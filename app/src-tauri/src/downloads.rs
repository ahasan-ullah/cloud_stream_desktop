//! Background downloads streamed straight to disk, so memory stays constant however big the file.
//!
//! - Direct files (MP4/MKV/…): resumable with HTTP Range into `<name>.part`, renamed when complete.
//! - HLS: every segment is saved (AES-128 decrypted) into `<name>/`, with a local `index.m3u8` the
//!   player (hls.js) or any external player can open. Separate audio renditions are kept too.
//!
//! State lives in `downloads.json`; progress is pushed to the UI as `download` events.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::{stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use url::Url;

const MAX_ACTIVE: usize = 2;
const SEGMENT_CONCURRENCY: usize = 3;
const EMIT_EVERY: Duration = Duration::from_millis(500);
/// Returned by a task that stopped because the user paused it.
const PAUSED: &str = "\u{0}paused";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleSource {
    pub lang: String,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
    pub title: String,
    pub episode: Option<String>,
    pub poster: Option<String>,
    pub provider: String,
    /// Details page URL, to reopen the title.
    pub page_url: String,
    /// Episode / movie data the provider uses for links; identifies what was downloaded.
    pub data: String,
    pub link_url: String,
    pub link_name: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub hls: bool,
    #[serde(default)]
    pub subtitles: Vec<SubtitleSource>,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Queued,
    Downloading,
    Paused,
    Done,
    Failed,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSubtitle {
    pub lang: String,
    /// Path relative to the downloads folder.
    pub file: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub id: String,
    #[serde(flatten)]
    pub request: DownloadRequest,
    pub state: State,
    /// File stem (direct) or folder name (HLS) inside the downloads folder.
    pub name: String,
    /// What to play, relative to the downloads folder, once known.
    pub file: Option<String>,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub segments_done: u32,
    pub segments_total: u32,
    /// Bytes per second while downloading.
    pub speed: u64,
    pub error: Option<String>,
    pub created_at: u64,
    #[serde(default)]
    pub subtitle_files: Vec<LocalSubtitle>,
    /// Folder this item was saved into (the downloads folder can change later).
    #[serde(default)]
    pub dir: String,
}

impl DownloadItem {
    pub fn folder(&self) -> &Path {
        Path::new(&self.dir)
    }
}

pub struct Downloads {
    /// Where new downloads go; chosen in Settings, persisted in `config`.
    dir: Mutex<PathBuf>,
    config: PathBuf,
    db: PathBuf,
    http: reqwest::Client,
    items: Mutex<Vec<DownloadItem>>,
    /// Cancel flags of running tasks.
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Downloads {
    /// `default_dir` is used until the user picks a folder (saved in `config`).
    pub fn load(default_dir: PathBuf, db: PathBuf, config: PathBuf) -> Arc<Self> {
        let dir = std::fs::read_to_string(&config)
            .ok()
            .map(|s| PathBuf::from(s.trim()))
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(default_dir);
        let _ = std::fs::create_dir_all(&dir);
        let mut items: Vec<DownloadItem> = std::fs::read(&db)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        for item in &mut items {
            // Anything that was running when the app closed resumes on demand.
            if matches!(item.state, State::Downloading | State::Queued) {
                item.state = State::Paused;
            }
            item.speed = 0;
            if item.dir.is_empty() {
                item.dir = dir.display().to_string();
            }
        }
        Arc::new(Self {
            dir: Mutex::new(dir),
            config,
            db,
            http: reqwest::Client::builder()
                .user_agent(crate::proxy::DEFAULT_UA)
                .connect_timeout(Duration::from_secs(20))
                .read_timeout(Duration::from_secs(60))
                .build()
                .expect("http client"),
            items: Mutex::new(items),
            running: Mutex::default(),
        })
    }

    pub fn dir(&self) -> PathBuf {
        self.dir.lock().unwrap().clone()
    }

    /// Changes where new downloads are saved. Existing downloads stay where they are.
    pub fn set_dir(&self, dir: PathBuf) -> Result<(), String> {
        std::fs::create_dir_all(&dir).map_err(|e| format!("Can't use that folder: {e}"))?;
        let probe = dir.join(".cloudstream-write-test");
        std::fs::write(&probe, b"ok").map_err(|_| "That folder isn't writable".to_string())?;
        let _ = std::fs::remove_file(probe);
        std::fs::write(&self.config, dir.display().to_string()).map_err(|e| e.to_string())?;
        *self.dir.lock().unwrap() = dir;
        Ok(())
    }

    pub fn list(&self) -> Vec<DownloadItem> {
        self.items.lock().unwrap().clone()
    }

    pub fn get(&self, id: &str) -> Option<DownloadItem> {
        self.items.lock().unwrap().iter().find(|i| i.id == id).cloned()
    }

    pub fn start(self: &Arc<Self>, app: &AppHandle, request: DownloadRequest) -> DownloadItem {
        let dir = self.dir();
        let item = {
            let mut items = self.items.lock().unwrap();
            let base = file_stem(&request);
            let mut name = base.clone();
            let mut n = 2;
            let taken = |name: &str| {
                items.iter().any(|i| i.name == name && i.folder() == dir)
                    || dir.join(name).exists()
                    || dir.join(format!("{name}.part")).exists()
            };
            while taken(&name) {
                name = format!("{base} ({n})");
                n += 1;
            }
            let item = DownloadItem {
                id: uuid::Uuid::new_v4().simple().to_string(),
                request,
                state: State::Queued,
                name,
                file: None,
                bytes_done: 0,
                bytes_total: None,
                segments_done: 0,
                segments_total: 0,
                speed: 0,
                error: None,
                created_at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
                subtitle_files: Vec::new(),
                dir: dir.display().to_string(),
            };
            items.push(item.clone());
            item
        };
        let _ = app.emit("download", &item);
        self.save();
        self.pump(app);
        item
    }

    pub fn pause(self: &Arc<Self>, app: &AppHandle, id: &str) {
        if let Some(flag) = self.running.lock().unwrap().get(id) {
            flag.store(true, Ordering::Relaxed); // the task marks itself Paused when it stops
        } else {
            self.update(app, id, |i| {
                if i.state == State::Queued {
                    i.state = State::Paused;
                }
            });
            self.save();
        }
    }

    pub fn resume(self: &Arc<Self>, app: &AppHandle, id: &str) {
        self.update(app, id, |i| {
            if matches!(i.state, State::Paused | State::Failed) {
                i.state = State::Queued;
                i.error = None;
            }
        });
        self.save();
        self.pump(app);
    }

    pub fn delete(self: &Arc<Self>, app: &AppHandle, id: &str, delete_files: bool) {
        if let Some(flag) = self.running.lock().unwrap().get(id) {
            flag.store(true, Ordering::Relaxed);
        }
        let removed = {
            let mut items = self.items.lock().unwrap();
            items.iter().position(|i| i.id == id).map(|pos| items.remove(pos))
        };
        if let (Some(item), true) = (removed, delete_files) {
            let dir = item.folder();
            if item.request.hls {
                let _ = std::fs::remove_dir_all(dir.join(&item.name));
            } else {
                let _ = std::fs::remove_file(dir.join(format!("{}.part", item.name)));
                if let Some(file) = &item.file {
                    let _ = std::fs::remove_file(dir.join(file));
                }
                for sub in &item.subtitle_files {
                    let _ = std::fs::remove_file(dir.join(&sub.file));
                }
            }
        }
        let _ = app.emit("download-removed", id);
        self.save();
        self.pump(app);
    }

    /// Starts queued downloads while there are free slots.
    fn pump(self: &Arc<Self>, app: &AppHandle) {
        let mut running = self.running.lock().unwrap();
        let free = MAX_ACTIVE.saturating_sub(running.len());
        let next: Vec<String> = self
            .items
            .lock()
            .unwrap()
            .iter()
            .filter(|i| i.state == State::Queued && !running.contains_key(&i.id))
            .take(free)
            .map(|i| i.id.clone())
            .collect();
        for id in next {
            let cancel = Arc::new(AtomicBool::new(false));
            running.insert(id.clone(), Arc::clone(&cancel));
            self.update(app, &id, |i| i.state = State::Downloading);
            let this = Arc::clone(self);
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let result = this.run(&app, &id, &cancel).await;
                this.running.lock().unwrap().remove(&id);
                this.update(&app, &id, |i| {
                    i.speed = 0;
                    match &result {
                        Ok(()) => i.state = State::Done,
                        Err(e) if e == PAUSED => i.state = State::Paused,
                        Err(e) => {
                            i.state = State::Failed;
                            i.error = Some(e.clone());
                        }
                    }
                });
                this.save();
                this.pump(&app);
            });
        }
    }

    async fn run(&self, app: &AppHandle, id: &str, cancel: &AtomicBool) -> Result<(), String> {
        let item = self.get(id).ok_or("download was removed")?;
        self.fetch_subtitles(app, &item).await;
        if item.request.hls {
            self.run_hls(app, &item, cancel).await
        } else {
            self.run_direct(app, &item, cancel).await
        }
    }

    // ---- direct files

    async fn run_direct(&self, app: &AppHandle, item: &DownloadItem, cancel: &AtomicBool) -> Result<(), String> {
        let dir = item.folder();
        tokio::fs::create_dir_all(dir).await.map_err(|e| format!("Can't write to {}: {e}", dir.display()))?;
        let part = dir.join(format!("{}.part", item.name));
        let mut done = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);

        let mut req = self.http.get(&item.request.link_url);
        for (k, v) in &item.request.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        if done > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={done}-"));
        }
        let res = req.send().await.map_err(|e| e.to_string())?;
        let status = res.status();
        if !status.is_success() {
            return Err(format!("Server returned {status}"));
        }
        if done > 0 && status != reqwest::StatusCode::PARTIAL_CONTENT {
            done = 0; // server ignored the Range header: start over
        }
        let total = res.content_length().map(|len| len + done);
        let content_type = res
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let file_name = format!("{}.{}", item.name, extension(&item.request.link_url, &content_type));
        self.update(app, &item.id, |i| {
            i.bytes_done = done;
            i.bytes_total = total;
        });

        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(done > 0)
            .truncate(done == 0)
            .open(&part)
            .await
            .map_err(|e| e.to_string())?;
        let mut out = tokio::io::BufWriter::with_capacity(256 * 1024, file);
        let mut meter = Meter::new(done);
        let mut body = res.bytes_stream();
        while let Some(chunk) = body.next().await {
            if cancel.load(Ordering::Relaxed) {
                out.flush().await.map_err(|e| e.to_string())?;
                self.update(app, &item.id, |i| i.bytes_done = done);
                return Err(PAUSED.into());
            }
            let chunk = chunk.map_err(|e| format!("Connection lost ({e}); resume to continue"))?;
            out.write_all(&chunk).await.map_err(|e| e.to_string())?;
            done += chunk.len() as u64;
            if let Some(speed) = meter.tick(done) {
                self.update(app, &item.id, |i| {
                    i.bytes_done = done;
                    i.speed = speed;
                });
            }
        }
        out.flush().await.map_err(|e| e.to_string())?;
        drop(out);
        if total.is_some_and(|t| done < t) {
            self.update(app, &item.id, |i| i.bytes_done = done);
            return Err("Connection closed early; resume to continue".into());
        }
        tokio::fs::rename(&part, dir.join(&file_name)).await.map_err(|e| e.to_string())?;
        self.update(app, &item.id, |i| {
            i.bytes_done = done;
            i.bytes_total = Some(done);
            i.file = Some(file_name);
        });
        Ok(())
    }

    // ---- HLS

    async fn run_hls(&self, app: &AppHandle, item: &DownloadItem, cancel: &AtomicBool) -> Result<(), String> {
        let folder = item.folder().join(&item.name);
        tokio::fs::create_dir_all(&folder).await.map_err(|e| e.to_string())?;
        let headers = &item.request.headers;

        let mut url = Url::parse(&item.request.link_url).map_err(|e| e.to_string())?;
        let mut text = self.text(&url, headers).await?;
        let mut audio = None;
        if text.contains("#EXT-X-STREAM-INF") {
            let variant = best_variant(&text, &url).ok_or("No playable stream in the playlist")?;
            audio = variant.audio;
            url = variant.uri;
            text = self.text(&url, headers).await?;
        }
        let video = parse_media_playlist(&text, &url)?;
        let audio = match audio {
            Some(audio_url) => {
                let audio_text = self.text(&audio_url, headers).await?;
                Some(parse_media_playlist(&audio_text, &audio_url)?)
            }
            None => None,
        };

        let total = (video.segments.len() + audio.as_ref().map_or(0, |a| a.segments.len())) as u32;
        self.update(app, &item.id, |i| i.segments_total = total);

        // Counts every segment, including ones already on disk from an earlier run.
        let progress = Mutex::new((0u32, 0u64, Meter::new(0)));
        let on_segment = |bytes: u64| {
            let mut p = progress.lock().unwrap();
            p.0 += 1;
            p.1 += bytes;
            let (count, done) = (p.0, p.1);
            let speed = p.2.tick(done);
            drop(p);
            if let Some(speed) = speed {
                self.update(app, &item.id, |i| {
                    i.segments_done = count;
                    i.bytes_done = done;
                    i.speed = speed;
                });
            }
        };
        let video_dir = if audio.is_some() { folder.join("video") } else { folder.clone() };
        self.download_playlist(&video, &video_dir, headers, cancel, &on_segment).await?;
        if let Some(audio) = &audio {
            self.download_playlist(audio, &folder.join("audio"), headers, cancel, &on_segment).await?;
            let master = "#EXTM3U\n\
                #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"Audio\",DEFAULT=YES,AUTOSELECT=YES,URI=\"audio/index.m3u8\"\n\
                #EXT-X-STREAM-INF:BANDWIDTH=2000000,AUDIO=\"aud\"\nvideo/index.m3u8\n";
            tokio::fs::write(folder.join("index.m3u8"), master).await.map_err(|e| e.to_string())?;
        }

        let (count, done, _) = *progress.lock().unwrap();
        self.update(app, &item.id, |i| {
            i.segments_done = count;
            i.bytes_done = done;
            i.bytes_total = Some(done);
            i.file = Some(format!("{}/index.m3u8", i.name));
        });
        Ok(())
    }

    /// Saves every segment of one media playlist into `dir` and writes `dir/index.m3u8`.
    async fn download_playlist(
        &self,
        playlist: &MediaPlaylist,
        dir: &Path,
        headers: &HashMap<String, String>,
        cancel: &AtomicBool,
        on_segment: &(impl Fn(u64) + Sync),
    ) -> Result<(), String> {
        tokio::fs::create_dir_all(dir).await.map_err(|e| e.to_string())?;
        if let Some(map) = &playlist.map {
            let init = dir.join("init.mp4");
            if !init.exists() {
                let data = self.bytes(&map.uri, headers, map.range).await?;
                tokio::fs::write(&init, data).await.map_err(|e| e.to_string())?;
            }
        }
        let ext = if playlist.map.is_some() { "m4s" } else { "ts" };
        let keys: tokio::sync::Mutex<HashMap<Url, Vec<u8>>> = Default::default();

        // Indices rather than `&Segment` items: a closure taking a borrowed argument makes the
        // spawned download future fail the `Send` check ("FnOnce is not general enough").
        stream::iter(0..playlist.segments.len())
            .map(|index| {
                let path = dir.join(format!("{index:05}.{ext}"));
                self.save_segment(&playlist.segments[index], path, headers, cancel, &keys)
            })
            .buffer_unordered(SEGMENT_CONCURRENCY)
            .try_for_each(|bytes| {
                on_segment(bytes);
                futures_util::future::ready(Ok(()))
            })
            .await?;

        tokio::fs::write(dir.join("index.m3u8"), local_playlist(playlist, ext))
            .await
            .map_err(|e| e.to_string())
    }

    /// Downloads (and decrypts) one segment to `path`; returns its size. Skips segments already saved.
    async fn save_segment(
        &self,
        segment: &Segment,
        path: PathBuf,
        headers: &HashMap<String, String>,
        cancel: &AtomicBool,
        keys: &tokio::sync::Mutex<HashMap<Url, Vec<u8>>>,
    ) -> Result<u64, String> {
        if cancel.load(Ordering::Relaxed) {
            return Err(PAUSED.to_string());
        }
        if let Ok(meta) = tokio::fs::metadata(&path).await {
            return Ok(meta.len());
        }
        let mut data = self.bytes(&segment.uri, headers, segment.range).await?;
        if let Some(key) = &segment.key {
            let key_bytes = {
                let mut cache = keys.lock().await;
                match cache.get(&key.uri) {
                    Some(k) => k.clone(),
                    None => {
                        let k = self.bytes(&key.uri, headers, None).await?;
                        cache.insert(key.uri.clone(), k.clone());
                        k
                    }
                }
            };
            data = decrypt_aes128(&data, &key_bytes, &key.iv)?;
        }
        let tmp = path.with_extension("tmp");
        tokio::fs::write(&tmp, &data).await.map_err(|e| e.to_string())?;
        tokio::fs::rename(&tmp, &path).await.map_err(|e| e.to_string())?;
        Ok(data.len() as u64)
    }

    async fn fetch_subtitles(&self, app: &AppHandle, item: &DownloadItem) {
        if !item.subtitle_files.is_empty() || item.request.subtitles.is_empty() {
            return;
        }
        let mut files = Vec::new();
        for (n, sub) in item.request.subtitles.iter().enumerate().take(8) {
            let Ok(url) = Url::parse(&sub.url) else { continue };
            let Ok(data) = self.bytes(&url, &sub.headers, None).await else { continue };
            let text = crate::proxy::to_webvtt(&String::from_utf8_lossy(&data));
            let lang = sanitize(&sub.lang);
            let file = if item.request.hls {
                format!("{}/sub-{n}-{lang}.vtt", item.name)
            } else {
                format!("{}.{n}.{lang}.vtt", item.name)
            };
            let path = item.folder().join(&file);
            if let Some(parent) = path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if tokio::fs::write(&path, text).await.is_ok() {
                files.push(LocalSubtitle { lang: sub.lang.clone(), file });
            }
        }
        self.update(app, &item.id, |i| i.subtitle_files = files);
    }

    async fn text(&self, url: &Url, headers: &HashMap<String, String>) -> Result<String, String> {
        String::from_utf8(self.bytes(url, headers, None).await?).map_err(|_| "Playlist is not text".into())
    }

    async fn bytes(&self, url: &Url, headers: &HashMap<String, String>, range: Option<(u64, u64)>) -> Result<Vec<u8>, String> {
        let mut last = String::new();
        for _ in 0..3 {
            let mut req = self.http.get(url.clone());
            for (k, v) in headers {
                req = req.header(k.as_str(), v.as_str());
            }
            if let Some((len, offset)) = range {
                req = req.header(reqwest::header::RANGE, format!("bytes={}-{}", offset, offset + len - 1));
            }
            match req.send().await {
                Ok(res) if res.status().is_success() => match res.bytes().await {
                    Ok(b) => return Ok(b.to_vec()),
                    Err(e) => last = e.to_string(),
                },
                Ok(res) => return Err(format!("Server returned {} for {}", res.status(), url)),
                Err(e) => last = e.to_string(),
            }
        }
        Err(format!("Network error: {last}"))
    }

    fn update(&self, app: &AppHandle, id: &str, f: impl FnOnce(&mut DownloadItem)) {
        let snapshot = {
            let mut items = self.items.lock().unwrap();
            let Some(item) = items.iter_mut().find(|i| i.id == id) else { return };
            f(item);
            item.clone()
        };
        let _ = app.emit("download", &snapshot);
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_vec_pretty(&*self.items.lock().unwrap()) {
            let _ = std::fs::write(&self.db, json);
        }
    }
}

/// Throttles progress events and computes speed.
struct Meter {
    last: Instant,
    last_bytes: u64,
}

impl Meter {
    fn new(bytes: u64) -> Self {
        Self { last: Instant::now(), last_bytes: bytes }
    }

    fn tick(&mut self, bytes: u64) -> Option<u64> {
        let elapsed = self.last.elapsed();
        if elapsed < EMIT_EVERY {
            return None;
        }
        let speed = ((bytes.saturating_sub(self.last_bytes)) as f64 / elapsed.as_secs_f64()) as u64;
        self.last = Instant::now();
        self.last_bytes = bytes;
        Some(speed)
    }
}

fn file_stem(r: &DownloadRequest) -> String {
    let name = match &r.episode {
        Some(ep) if !ep.is_empty() && ep != &r.title => format!("{} - {}", r.title, ep),
        _ => r.title.clone(),
    };
    let clean = sanitize(&name);
    if clean.is_empty() {
        "Download".into()
    } else {
        clean
    }
}

/// Safe file name on Windows and macOS.
fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { ' ' } else { c })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.trim_matches(|c: char| c == '.' || c == ' ').chars().take(120).collect()
}

fn extension(url: &str, content_type: &str) -> &'static str {
    let path = url.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
    for ext in ["mp4", "mkv", "webm", "avi", "mov", "m4v", "ts"] {
        if path.ends_with(&format!(".{ext}")) {
            return ext;
        }
    }
    match content_type {
        t if t.contains("matroska") => "mkv",
        t if t.contains("webm") => "webm",
        t if t.contains("mp2t") => "ts",
        _ => "mp4",
    }
}

// ---- HLS playlist parsing

struct Variant {
    uri: Url,
    audio: Option<Url>,
}

#[derive(Clone)]
struct Key {
    uri: Url,
    iv: [u8; 16],
}

struct Map {
    uri: Url,
    range: Option<(u64, u64)>,
}

struct Segment {
    uri: Url,
    duration: f64,
    /// (length, offset) for EXT-X-BYTERANGE.
    range: Option<(u64, u64)>,
    key: Option<Key>,
}

struct MediaPlaylist {
    segments: Vec<Segment>,
    map: Option<Map>,
}

/// Highest-bandwidth variant of a master playlist, plus its separate audio rendition if any.
fn best_variant(text: &str, base: &Url) -> Option<Variant> {
    let mut audio_groups: HashMap<String, Url> = HashMap::new();
    let mut best: Option<(u64, Url, Option<String>)> = None;
    let mut pending: Option<(u64, Option<String>)> = None;
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("#EXT-X-MEDIA:") {
            let a = attributes(rest);
            if a.get("TYPE").map(String::as_str) == Some("AUDIO") {
                if let (Some(group), Some(uri)) = (a.get("GROUP-ID"), a.get("URI")) {
                    let is_default = a.get("DEFAULT").map(String::as_str) == Some("YES");
                    if let Ok(u) = base.join(uri) {
                        if is_default || !audio_groups.contains_key(group) {
                            audio_groups.insert(group.clone(), u);
                        }
                    }
                }
            }
        } else if let Some(rest) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let a = attributes(rest);
            let bandwidth = a.get("BANDWIDTH").and_then(|b| b.parse().ok()).unwrap_or(0);
            pending = Some((bandwidth, a.get("AUDIO").cloned()));
        } else if !line.is_empty() && !line.starts_with('#') {
            if let (Some((bandwidth, audio)), Ok(uri)) = (pending.take(), base.join(line)) {
                if best.as_ref().is_none_or(|(b, _, _)| bandwidth > *b) {
                    best = Some((bandwidth, uri, audio));
                }
            }
        }
    }
    best.map(|(_, uri, audio)| Variant { uri, audio: audio.and_then(|g| audio_groups.get(&g).cloned()) })
}

fn parse_media_playlist(text: &str, base: &Url) -> Result<MediaPlaylist, String> {
    let mut segments = Vec::new();
    let mut map = None;
    let mut sequence: u64 = 0;
    let mut key: Option<(Url, Option<[u8; 16]>)> = None;
    let mut duration = 0.0;
    let mut range: Option<(u64, u64)> = None;
    let mut next_offset = 0u64;

    for line in text.lines().map(str::trim) {
        if let Some(v) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            sequence = v.trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("#EXT-X-KEY:") {
            let a = attributes(rest);
            match a.get("METHOD").map(String::as_str) {
                Some("NONE") | None => key = None,
                Some("AES-128") => {
                    let uri = a.get("URI").ok_or("Encryption key without URI")?;
                    let uri = base.join(uri).map_err(|e| e.to_string())?;
                    key = Some((uri, a.get("IV").and_then(|iv| parse_iv(iv))));
                }
                Some(other) => return Err(format!("This stream is DRM-protected ({other}) and can't be downloaded")),
            }
        } else if let Some(rest) = line.strip_prefix("#EXT-X-MAP:") {
            let a = attributes(rest);
            let uri = base.join(a.get("URI").ok_or("EXT-X-MAP without URI")?).map_err(|e| e.to_string())?;
            map = Some(Map { uri, range: a.get("BYTERANGE").and_then(|r| parse_range(r, 0)) });
        } else if let Some(rest) = line.strip_prefix("#EXTINF:") {
            duration = rest.split(',').next().and_then(|d| d.trim().parse().ok()).unwrap_or(0.0);
        } else if let Some(rest) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            range = parse_range(rest, next_offset);
            if let Some((len, offset)) = range {
                next_offset = offset + len;
            }
        } else if !line.is_empty() && !line.starts_with('#') {
            let uri = base.join(line).map_err(|e| e.to_string())?;
            let segment_key = key.as_ref().map(|(uri, iv)| Key {
                uri: uri.clone(),
                iv: iv.unwrap_or_else(|| (sequence as u128).to_be_bytes()),
            });
            segments.push(Segment { uri, duration, range: range.take(), key: segment_key });
            sequence += 1;
            duration = 0.0;
        }
    }
    if segments.is_empty() {
        return Err("The playlist has no segments".into());
    }
    Ok(MediaPlaylist { segments, map })
}

fn local_playlist(playlist: &MediaPlaylist, ext: &str) -> String {
    let target = playlist.segments.iter().map(|s| s.duration).fold(1.0, f64::max).ceil() as u64;
    let mut out = format!(
        "#EXTM3U\n#EXT-X-VERSION:{}\n#EXT-X-TARGETDURATION:{target}\n#EXT-X-MEDIA-SEQUENCE:0\n#EXT-X-PLAYLIST-TYPE:VOD\n",
        if playlist.map.is_some() { 7 } else { 3 }
    );
    if playlist.map.is_some() {
        out.push_str("#EXT-X-MAP:URI=\"init.mp4\"\n");
    }
    for (i, s) in playlist.segments.iter().enumerate() {
        out.push_str(&format!("#EXTINF:{:.3},\n{i:05}.{ext}\n", s.duration));
    }
    out.push_str("#EXT-X-ENDLIST\n");
    out
}

/// `KEY=value,KEY="quoted, value"` attribute lists.
fn attributes(s: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut rest = s;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].trim().trim_start_matches(',').trim().to_string();
        rest = &rest[eq + 1..];
        let value;
        if let Some(stripped) = rest.strip_prefix('"') {
            let end = stripped.find('"').unwrap_or(stripped.len());
            value = stripped[..end].to_string();
            rest = stripped.get(end + 1..).unwrap_or("");
        } else {
            let end = rest.find(',').unwrap_or(rest.len());
            value = rest[..end].trim().to_string();
            rest = &rest[end..];
        }
        rest = rest.trim_start_matches(',');
        out.insert(key, value);
    }
    out
}

fn parse_iv(iv: &str) -> Option<[u8; 16]> {
    let hex = iv.trim_start_matches("0x").trim_start_matches("0X");
    let value = u128::from_str_radix(hex, 16).ok()?;
    Some(value.to_be_bytes())
}

/// `length[@offset]`; the offset defaults to the end of the previous range.
fn parse_range(s: &str, default_offset: u64) -> Option<(u64, u64)> {
    let mut parts = s.trim().split('@');
    let len = parts.next()?.trim().parse().ok()?;
    let offset = parts.next().and_then(|o| o.trim().parse().ok()).unwrap_or(default_offset);
    Some((len, offset))
}

fn decrypt_aes128(data: &[u8], key: &[u8], iv: &[u8; 16]) -> Result<Vec<u8>, String> {
    use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
    cbc::Decryptor::<aes::Aes128>::new_from_slices(key, iv)
        .map_err(|_| "Invalid encryption key".to_string())?
        .decrypt_padded_vec_mut::<Pkcs7>(data)
        .map_err(|_| "Could not decrypt a segment".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_highest_bandwidth_with_audio_group() {
        let base = Url::parse("https://cdn.example/v/master.m3u8").unwrap();
        let text = "#EXTM3U\n\
            #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"English\",DEFAULT=YES,URI=\"audio/en.m3u8\"\n\
            #EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360,AUDIO=\"a\"\nlow.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080,CODECS=\"avc1.640028,mp4a.40.2\",AUDIO=\"a\"\nhigh.m3u8\n";
        let v = best_variant(text, &base).unwrap();
        assert_eq!(v.uri.as_str(), "https://cdn.example/v/high.m3u8");
        assert_eq!(v.audio.unwrap().as_str(), "https://cdn.example/v/audio/en.m3u8");
    }

    #[test]
    fn parses_keys_ranges_and_sequence_ivs() {
        let base = Url::parse("https://cdn.example/v/index.m3u8").unwrap();
        let text = "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:7\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\"\n\
            #EXTINF:4.0,\na.ts\n#EXT-X-BYTERANGE:100@0\n#EXTINF:4.5,\nall.ts\n#EXT-X-BYTERANGE:50\n#EXTINF:2,\nall.ts\n\
            #EXT-X-KEY:METHOD=NONE\n#EXTINF:1,\nplain.ts\n#EXT-X-ENDLIST\n";
        let p = parse_media_playlist(text, &base).unwrap();
        assert_eq!(p.segments.len(), 4);
        let k = p.segments[0].key.as_ref().unwrap();
        assert_eq!(k.uri.as_str(), "https://cdn.example/v/k.bin");
        assert_eq!(k.iv, 7u128.to_be_bytes());
        assert_eq!(p.segments[1].range, Some((100, 0)));
        assert_eq!(p.segments[2].range, Some((50, 100)));
        assert!(p.segments[3].key.is_none());
        let local = local_playlist(&p, "ts");
        assert!(local.contains("#EXTINF:4.500,\n00001.ts"));
        assert!(local.ends_with("#EXT-X-ENDLIST\n"));
    }

    #[test]
    fn rejects_drm() {
        let base = Url::parse("https://cdn.example/v/index.m3u8").unwrap();
        let text = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://x\"\n#EXTINF:4,\na.ts\n";
        let Err(error) = parse_media_playlist(text, &base) else { panic!("DRM stream should be rejected") };
        assert!(error.contains("DRM"));
    }

    #[test]
    fn decrypts_aes128_cbc() {
        use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
        let (key, iv) = ([7u8; 16], [9u8; 16]);
        let plain = b"segment payload that spans more than one block".to_vec();
        let enc = cbc::Encryptor::<aes::Aes128>::new_from_slices(&key, &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(&plain);
        assert_eq!(decrypt_aes128(&enc, &key, &iv).unwrap(), plain);
    }

    #[test]
    fn sanitizes_file_names() {
        assert_eq!(sanitize("Batman: The \"Dark\" Knight / Part 1?"), "Batman The Dark Knight Part 1");
        assert_eq!(sanitize("..."), "");
    }
}
