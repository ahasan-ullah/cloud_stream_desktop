//! Local HTTP proxy that lets the webview play streams which need custom headers.
//!
//! Video hosts often require a specific Referer/User-Agent, which a `<video>` element
//! cannot send. Each stream is registered with its headers under a random id; the
//! proxy replays requests upstream with those headers, forwards Range requests, and
//! rewrites HLS playlists so that every segment, key and variant also goes through it.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use url::Url;

type Headers = HashMap<String, String>;

#[derive(Clone)]
pub struct Proxy {
    port: u16,
    streams: Arc<RwLock<HashMap<String, Headers>>>,
    /// Download folders the player may read from, addressed by index in `/files/{index}/…`.
    roots: Arc<RwLock<Vec<PathBuf>>>,
}

#[derive(Clone)]
struct ProxyState {
    proxy: Proxy,
    http: reqwest::Client,
}

/// How the proxy should treat a response body.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// HLS playlist: rewrite URIs.
    M3u8,
    /// Subtitle file: convert SRT to WebVTT if needed.
    Sub,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::M3u8 => "m3u8",
            Kind::Sub => "sub",
        }
    }
}

#[derive(Deserialize)]
struct Params {
    u: String,
    k: Option<Kind>,
}

impl Proxy {
    pub async fn start() -> std::io::Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let proxy = Proxy {
            port: listener.local_addr()?.port(),
            streams: Default::default(),
            roots: Default::default(),
        };
        let http = reqwest::Client::builder()
            .user_agent(DEFAULT_UA)
            .connect_timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(std::io::Error::other)?;
        let router = Router::new()
            .route("/p/{id}", get(handle))
            .route("/files/{root}/{*path}", get(serve_file))
            .with_state(ProxyState { proxy: proxy.clone(), http })
            .layer(axum::middleware::map_response(|res: Response| async { cors(res) }));
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                eprintln!("[proxy] stopped: {e}");
            }
        });
        Ok(proxy)
    }

    /// URL of `relative` (with `/` separators) inside the download folder `root`.
    pub fn file_url(&self, root: &std::path::Path, relative: &str) -> String {
        let index = {
            let mut roots = self.roots.write().unwrap();
            match roots.iter().position(|r| r == root) {
                Some(i) => i,
                None => {
                    roots.push(root.to_path_buf());
                    roots.len() - 1
                }
            }
        };
        let mut url = Url::parse(&format!("http://127.0.0.1:{}/files/{index}/", self.port)).expect("valid base");
        if let Ok(mut segments) = url.path_segments_mut() {
            segments.pop_if_empty();
            for part in relative.split('/') {
                segments.push(part);
            }
        }
        url.to_string()
    }

    /// Registers headers for a stream and returns the local URL that serves it.
    pub fn register(&self, url: &str, headers: Headers, kind: Option<Kind>) -> String {
        let id = uuid::Uuid::new_v4().simple().to_string();
        self.streams.write().unwrap().insert(id.clone(), headers);
        self.url_for(&id, url, kind)
    }

    fn url_for(&self, id: &str, target: &str, kind: Option<Kind>) -> String {
        let encoded: String = url::form_urlencoded::byte_serialize(target.as_bytes()).collect();
        let mut out = format!("http://127.0.0.1:{}/p/{id}?u={encoded}", self.port);
        if let Some(kind) = kind {
            out.push_str("&k=");
            out.push_str(kind.as_str());
        }
        out
    }
}

pub(crate) const DEFAULT_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

/// Response headers worth passing through for media playback.
const PASS_HEADERS: [header::HeaderName; 5] = [
    header::CONTENT_TYPE,
    header::CONTENT_LENGTH,
    header::CONTENT_RANGE,
    header::ACCEPT_RANGES,
    header::LAST_MODIFIED,
];

async fn handle(
    State(st): State<ProxyState>,
    Path(id): Path<String>,
    Query(params): Query<Params>,
    req_headers: HeaderMap,
) -> Response {
    let Some(headers) = st.proxy.streams.read().unwrap().get(&id).cloned() else {
        return (StatusCode::NOT_FOUND, "unknown stream").into_response();
    };

    let mut req = st.http.get(&params.u);
    for (k, v) in &headers {
        req = req.header(k.as_str(), v.as_str());
    }
    if let Some(range) = req_headers.get(header::RANGE) {
        req = req.header(header::RANGE, range.clone());
    }
    let upstream = match req.send().await {
        Ok(r) => r,
        Err(e) => return cors((StatusCode::BAD_GATEWAY, e.to_string()).into_response()),
    };

    let status = upstream.status();
    let content_type = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let kind = params.k.or_else(|| {
        let path = upstream.url().path().to_ascii_lowercase();
        (content_type.contains("mpegurl") || path.ends_with(".m3u8")).then_some(Kind::M3u8)
    });

    let Some(kind) = kind.filter(|_| status.is_success()) else {
        let mut res = Response::builder().status(status);
        for name in PASS_HEADERS {
            if let Some(v) = upstream.headers().get(&name) {
                res = res.header(name, v.clone());
            }
        }
        let body = Body::from_stream(upstream.bytes_stream());
        return cors(res.body(body).unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response()));
    };

    // Relative URIs resolve against the final URL after redirects.
    let base = upstream.url().clone();
    let text = match upstream.text().await {
        Ok(t) => t,
        Err(e) => return cors((StatusCode::BAD_GATEWAY, e.to_string()).into_response()),
    };
    let (body, mime) = match kind {
        Kind::M3u8 => (rewrite_playlist(&text, &base, &st.proxy, &id), "application/vnd.apple.mpegurl"),
        Kind::Sub => (to_webvtt(&text), "text/vtt; charset=utf-8"),
    };
    let mut res = (StatusCode::OK, body).into_response();
    res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    cors(res)
}

/// `GET /files/{root}/{path}`: a downloaded file, with Range support for seeking.
/// Only plain relative paths inside a registered download folder are served.
async fn serve_file(
    State(st): State<ProxyState>,
    Path((root, path)): Path<(usize, String)>,
    request: axum::extract::Request,
) -> Response {
    use std::path::Component;
    use tower::ServiceExt;

    let Some(base) = st.proxy.roots.read().unwrap().get(root).cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let relative = std::path::Path::new(&path);
    if relative.components().any(|c| !matches!(c, Component::Normal(_))) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    match tower_http::services::ServeFile::new(base.join(relative)).oneshot(request).await {
        Ok(res) => res.map(Body::new),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn cors(mut res: Response) -> Response {
    res.headers_mut()
        .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    res
}

fn rewrite_playlist(text: &str, base: &Url, proxy: &Proxy, id: &str) -> String {
    let proxied = |uri: &str, kind: Option<Kind>| match base.join(uri) {
        Ok(abs) => proxy.url_for(id, abs.as_str(), kind),
        Err(_) => uri.to_string(),
    };

    let mut out = String::with_capacity(text.len() * 2);
    let mut next_is_playlist = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            if line.starts_with("#EXT-X-STREAM-INF") {
                next_is_playlist = true;
            }
            out.push_str(&rewrite_uri_attr(line, |uri| {
                // Alternate renditions and I-frame lists are playlists; keys and init maps are not.
                let is_playlist = line.starts_with("#EXT-X-MEDIA") || line.starts_with("#EXT-X-I-FRAME-STREAM-INF");
                proxied(uri, is_playlist.then_some(Kind::M3u8))
            }));
        } else if !line.is_empty() {
            out.push_str(&proxied(line, next_is_playlist.then_some(Kind::M3u8)));
            next_is_playlist = false;
        }
        out.push('\n');
    }
    out
}

fn rewrite_uri_attr(line: &str, map: impl Fn(&str) -> String) -> String {
    const ATTR: &str = "URI=\"";
    let Some(start) = line.find(ATTR).map(|i| i + ATTR.len()) else {
        return line.to_string();
    };
    let Some(len) = line[start..].find('"') else {
        return line.to_string();
    };
    format!("{}{}{}", &line[..start], map(&line[start..start + len]), &line[start + len..])
}

/// Browsers only render WebVTT tracks; most providers serve SRT.
pub(crate) fn to_webvtt(text: &str) -> String {
    let text = text.trim_start_matches('\u{feff}');
    if text.trim_start().starts_with("WEBVTT") {
        return text.to_string();
    }
    let mut out = String::from("WEBVTT\n\n");
    for line in text.lines() {
        if line.contains("-->") {
            out.push_str(&line.replace(',', "."));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy() -> Proxy {
        Proxy { port: 9, streams: Default::default(), roots: Default::default() }
    }

    #[test]
    fn rewrites_variants_segments_and_keys() {
        let base = Url::parse("https://cdn.example/hls/master.m3u8").unwrap();
        let input = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n#EXT-X-STREAM-INF:BANDWIDTH=1\n720/index.m3u8\nseg1.ts\n";
        let out = rewrite_playlist(input, &base, &proxy(), "abc");
        let lines: Vec<_> = out.lines().collect();
        assert_eq!(lines[0], "#EXTM3U");
        assert!(lines[1].contains("u=https%3A%2F%2Fcdn.example%2Fhls%2Fkey.bin\""));
        assert!(lines[3].ends_with("720%2Findex.m3u8&k=m3u8"));
        assert!(lines[4].ends_with("seg1.ts"));
    }

    #[test]
    fn converts_srt() {
        let vtt = to_webvtt("1\r\n00:00:01,000 --> 00:00:02,500\r\nHello, world\r\n");
        assert!(vtt.starts_with("WEBVTT"));
        assert!(vtt.contains("00:00:01.000 --> 00:00:02.500"));
        assert!(vtt.contains("Hello, world"));
    }
}
