use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::{
    downloads::{DownloadItem, DownloadRequest, Downloads},
    engine::{Engine, EngineStatus},
    mpv,
    proxy::{Kind, Proxy},
};

#[tauri::command]
pub async fn engine_status(engine: State<'_, Arc<Engine>>) -> Result<EngineStatus, String> {
    Ok(engine.status().await)
}

#[tauri::command]
pub async fn engine_restart(app: AppHandle, engine: State<'_, Arc<Engine>>) -> Result<EngineStatus, String> {
    let engine = Arc::clone(&engine);
    engine.start(&app).await;
    Ok(engine.status().await)
}

/// Generic passthrough to the engine's REST API; the frontend owns the typed wrappers.
#[tauri::command]
pub async fn engine_request(
    engine: State<'_, Arc<Engine>>,
    method: String,
    path: String,
    query: Option<HashMap<String, String>>,
    body: Option<Value>,
) -> Result<Value, String> {
    engine.request(&method, &path, query, body).await
}

#[tauri::command]
pub fn proxy_url(proxy: State<'_, Proxy>, url: String, headers: HashMap<String, String>, kind: Option<Kind>) -> String {
    proxy.register(&url, headers, kind)
}

/// Opens an http(s) link in the default browser.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|e| e.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only http(s) links can be opened".into());
    }
    #[cfg(windows)]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", parsed.as_str()]).creation_flags(0x0800_0000);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(parsed.as_str());
        c
    };
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(parsed.as_str());
        c
    };
    cmd.spawn().map(drop).map_err(|e| e.to_string())
}

// ---- downloads

#[tauri::command]
pub fn downloads_list(downloads: State<'_, Arc<Downloads>>) -> Vec<DownloadItem> {
    downloads.list()
}

#[tauri::command]
pub fn download_start(app: AppHandle, downloads: State<'_, Arc<Downloads>>, request: DownloadRequest) -> DownloadItem {
    downloads.start(&app, request)
}

#[tauri::command]
pub fn download_pause(app: AppHandle, downloads: State<'_, Arc<Downloads>>, id: String) {
    downloads.pause(&app, &id)
}

#[tauri::command]
pub fn download_resume(app: AppHandle, downloads: State<'_, Arc<Downloads>>, id: String) {
    downloads.resume(&app, &id)
}

#[tauri::command]
pub fn download_delete(app: AppHandle, downloads: State<'_, Arc<Downloads>>, id: String, delete_files: bool) {
    downloads.delete(&app, &id, delete_files)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSubtitleUrl {
    lang: String,
    url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPlayback {
    url: String,
    hls: bool,
    subtitles: Vec<LocalSubtitleUrl>,
}

/// Local URLs (served by the proxy) to play a finished download.
#[tauri::command]
pub fn download_playback(
    downloads: State<'_, Arc<Downloads>>,
    proxy: State<'_, Proxy>,
    id: String,
) -> Result<LocalPlayback, String> {
    let item = downloads.get(&id).ok_or("Download not found")?;
    let file = item.file.as_deref().ok_or("This download hasn't finished yet")?;
    let folder = item.folder();
    if !folder.join(file).exists() {
        return Err("The downloaded file was moved or deleted".into());
    }
    Ok(LocalPlayback {
        url: proxy.file_url(folder, file),
        hls: item.request.hls,
        subtitles: item
            .subtitle_files
            .iter()
            .map(|s| LocalSubtitleUrl { lang: s.lang.clone(), url: proxy.file_url(folder, &s.file) })
            .collect(),
    })
}

#[tauri::command]
pub fn download_dir(downloads: State<'_, Arc<Downloads>>) -> String {
    downloads.dir().display().to_string()
}

/// Lets the user pick where new downloads are saved. Returns the new folder, or None if cancelled.
#[tauri::command]
pub async fn choose_download_dir(app: AppHandle, downloads: State<'_, Arc<Downloads>>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let current = downloads.dir();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().set_title("Choose downloads folder").set_directory(current).blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(picked) = picked else { return Ok(None) };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    downloads.set_dir(path.clone())?;
    Ok(Some(path.display().to_string()))
}

/// Shows a download (or the downloads folder) in Explorer / Finder.
#[tauri::command]
pub fn reveal_download(downloads: State<'_, Arc<Downloads>>, id: Option<String>) -> Result<(), String> {
    let target = id
        .and_then(|id| downloads.get(&id))
        .map(|item| {
            let rel = if item.request.hls { item.name.clone() } else { item.file.clone().unwrap_or(format!("{}.part", item.name)) };
            item.folder().join(rel)
        })
        .filter(|p| p.exists());
    match target {
        Some(path) => reveal(&path, true),
        None => reveal(&downloads.dir(), false),
    }
}

fn backup_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().document_dir().or_else(|_| app.path().app_data_dir()).map_err(|e| e.to_string())?;
    Ok(base.join("CloudStream"))
}

/// Writes a backup JSON (built by the UI) to Documents/CloudStream and returns its path.
#[tauri::command]
pub fn save_backup(app: AppHandle, file_name: String, json: String) -> Result<String, String> {
    let safe: String = file_name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    if !safe.ends_with(".json") || safe.len() < 6 {
        return Err("Invalid backup file name".into());
    }
    let dir = backup_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(safe);
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn reveal_backups(app: AppHandle) -> Result<(), String> {
    let dir = backup_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    reveal(&dir, false)
}

fn reveal(path: &Path, select: bool) -> Result<(), String> {
    #[cfg(windows)]
    let result = {
        let mut c = std::process::Command::new("explorer");
        if select {
            c.arg(format!("/select,{}", path.display()));
        } else {
            c.arg(path);
        }
        c.spawn()
    };
    #[cfg(target_os = "macos")]
    let result = {
        let mut c = std::process::Command::new("open");
        if select {
            c.arg("-R");
        }
        c.arg(path).spawn()
    };
    #[cfg(not(any(windows, target_os = "macos")))]
    let result = {
        let _ = select;
        std::process::Command::new("xdg-open").arg(path).spawn()
    };
    result.map(drop).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_in_mpv(
    mpv_path: Option<String>,
    url: String,
    title: String,
    headers: HashMap<String, String>,
    subtitles: Vec<String>,
) -> Result<(), String> {
    mpv::open(mpv_path.as_deref(), &url, &title, &headers, &subtitles)
}
