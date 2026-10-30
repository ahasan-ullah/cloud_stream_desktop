//! Poster thumbnails for the webview, served over the `thumb:` URI scheme.
//!
//! Chromium keeps every `<img>` decoded at its original size (a 1000×1500 poster is ~6 MB of
//! bitmap), so posters are fetched here and shrunk to the size the card shows. A custom scheme
//! rather than the localhost proxy: WebView2 hands these requests straight to us, without the
//! six-connections-per-host limit that would queue a screenful of posters behind each other.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use tauri::{
    http::{header, Request, Response, StatusCode},
    AppHandle, Manager, Runtime, UriSchemeResponder,
};
use url::Url;

/// Encoded thumbnails kept in memory so revisiting a page doesn't download and decode again.
const CACHE_BYTES: usize = 8 * 1024 * 1024;
/// Originals we couldn't shrink are passed through but not cached above this size.
const MAX_CACHED_ITEM: usize = 512 * 1024;

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

pub struct Thumbnails {
    http: reqwest::Client,
    /// Bounds concurrent decodes so a page full of posters can't spike CPU and memory.
    slots: tokio::sync::Semaphore,
    cache: Mutex<Cache>,
}

pub struct Thumb {
    data: Vec<u8>,
    mime: String,
}

#[derive(Default)]
struct Cache {
    items: HashMap<String, Arc<Thumb>>,
    order: VecDeque<String>,
    bytes: usize,
}

impl Cache {
    fn insert(&mut self, key: String, thumb: Arc<Thumb>) {
        if thumb.data.len() > MAX_CACHED_ITEM || self.items.contains_key(&key) {
            return;
        }
        self.bytes += thumb.data.len();
        self.items.insert(key.clone(), thumb);
        self.order.push_back(key);
        while self.bytes > CACHE_BYTES {
            let Some(old) = self.order.pop_front() else { break };
            if let Some(t) = self.items.remove(&old) {
                self.bytes -= t.data.len();
            }
        }
    }
}

impl Thumbnails {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .connect_timeout(std::time::Duration::from_secs(15))
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("http client"),
            slots: tokio::sync::Semaphore::new(
                std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(2, 6)),
            ),
            cache: Mutex::default(),
        }
    }

    /// `url` scaled down to `width` pixels wide (or the original if it can't be decoded).
    pub async fn get(&self, url: &str, width: u32) -> Result<Arc<Thumb>, (StatusCode, String)> {
        let key = format!("{width}|{url}");
        if let Some(hit) = self.cache.lock().unwrap().items.get(&key) {
            return Ok(Arc::clone(hit));
        }
        let parsed = match Url::parse(url) {
            Ok(u) if matches!(u.scheme(), "http" | "https") => u,
            _ => return Err((StatusCode::BAD_REQUEST, "bad image url".into())),
        };

        let (bytes, original_mime) = self.download(&parsed).await?;

        // Downloads run freely; only the CPU-heavy decode is limited.
        let (shrunk, bytes) = {
            let _slot = self.slots.acquire().await;
            tokio::task::spawn_blocking(move || (shrink(&bytes, width), bytes))
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        };
        let thumb = Arc::new(match shrunk {
            Some((data, mime)) => Thumb { data, mime: mime.to_string() },
            None => Thumb { data: bytes, mime: original_mime },
        });
        self.cache.lock().unwrap().insert(key, Arc::clone(&thumb));
        Ok(thumb)
    }

    /// Image bytes and content type. Retries once: poster CDNs drop connections now and then.
    async fn download(&self, url: &Url) -> Result<(Vec<u8>, String), (StatusCode, String)> {
        let mut last_error = String::new();
        for _ in 0..2 {
            let res = match self.http.get(url.clone()).header(header::ACCEPT, "image/*,*/*;q=0.8").send().await {
                Ok(r) => r,
                Err(e) => {
                    last_error = e.to_string();
                    continue;
                }
            };
            if !res.status().is_success() {
                return Err((res.status(), format!("upstream returned {}", res.status())));
            }
            let mime = res
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();
            match res.bytes().await {
                Ok(bytes) => return Ok((bytes.to_vec(), mime)),
                Err(e) => last_error = e.to_string(),
            }
        }
        Err((StatusCode::BAD_GATEWAY, last_error))
    }
}

/// Handler for `thumb://localhost/?w=<pixels>&u=<image url>`
/// (`http://thumb.localhost/?…` on Windows).
pub fn handle<R: Runtime>(app: &AppHandle<R>, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let mut width = 0u32;
    let mut url = String::new();
    for (k, v) in url::form_urlencoded::parse(request.uri().query().unwrap_or("").as_bytes()) {
        match &*k {
            "w" => width = v.parse().unwrap_or(0),
            "u" => url = v.into_owned(),
            _ => {}
        }
    }
    let thumbs = Arc::clone(&app.state::<Arc<Thumbnails>>());
    tauri::async_runtime::spawn(async move {
        let response = match thumbs.get(&url, width.clamp(32, 1600)).await {
            Ok(t) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, &t.mime)
                .header(header::CACHE_CONTROL, "public, max-age=86400")
                .body(t.data.clone()),
            Err((status, message)) => {
                eprintln!("[thumb] {status} {url}: {message}");
                Response::builder().status(status).body(message.into_bytes())
            }
        };
        responder.respond(response.expect("valid response"));
    });
}

/// Decodes, downsizes and re-encodes an image (JPEG, or PNG when it has transparency).
/// Returns None when it is already narrow enough or can't be decoded.
fn shrink(data: &[u8], width: u32) -> Option<(Vec<u8>, &'static str)> {
    use image::{codecs::jpeg::JpegEncoder, ImageFormat, ImageReader, Limits};
    use std::io::Cursor;

    let mut reader = ImageReader::new(Cursor::new(data)).with_guessed_format().ok()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(96 * 1024 * 1024); // refuse absurdly large images
    reader.limits(limits);
    let img = reader.decode().ok()?;
    if img.width() <= width {
        return None;
    }
    let height = ((img.height() as u64 * width as u64) / img.width() as u64).max(1) as u32;
    let small = img.thumbnail_exact(width, height);

    let mut out = Vec::new();
    if small.color().has_alpha() {
        small.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).ok()?;
        Some((out, "image/png"))
    } else {
        JpegEncoder::new_with_quality(&mut out, 82).encode_image(&small.to_rgb8()).ok()?;
        Some((out, "image/jpeg"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinks_large_images_only() {
        let big = image::DynamicImage::new_rgb8(1000, 1500);
        let mut png = Vec::new();
        big.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();

        let (jpeg, mime) = shrink(&png, 160).expect("should shrink");
        assert_eq!(mime, "image/jpeg");
        let thumb = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (160, 240));

        assert!(shrink(&png, 2000).is_none(), "already narrow enough");
        assert!(shrink(b"not an image", 160).is_none());
    }

    #[test]
    fn cache_evicts_oldest_beyond_budget() {
        let mut cache = Cache::default();
        let item = |n: usize| Arc::new(Thumb { data: vec![0; n], mime: "image/jpeg".into() });
        for i in 0..20 {
            cache.insert(format!("k{i}"), item(500 * 1024));
        }
        assert!(cache.bytes <= CACHE_BYTES);
        assert!(!cache.items.contains_key("k0"), "oldest evicted");
        assert!(cache.items.contains_key("k19"), "newest kept");
        cache.insert("huge".into(), item(MAX_CACHED_ITEM + 1));
        assert!(!cache.items.contains_key("huge"));
    }
}
