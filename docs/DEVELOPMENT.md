# Development

How CloudStream Desktop works and how to work on it. For installing and using the app, see the [README](../README.md).

- **App:** [Tauri 2](https://tauri.app): a Rust core plus the OS's own webview (WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux). It has no bundled browser and no .NET runtime.
- **UI:** Svelte 5, built to match the CloudStream Android app's tablet layout (see below). The first-load JS is about 46 KB gzipped; hls.js loads only when an HLS stream plays.
- **Engine:** a headless Kotlin/JVM sidecar that runs unmodified CloudStream `.cs3` plugins and the built-in extractors from CloudStream's `library` module.

```
┌──────────── Tauri app (Rust) ────────────┐        ┌──── engine.jar (JVM sidecar) ────┐
│ Svelte UI ──invoke──▶ commands.rs        │  HTTP  │ Ktor on 127.0.0.1:<random>       │
│                       engine.rs ─────────┼───────▶│  /api/search /load /links …      │
│ <video> / hls.js ◀── proxy.rs ◀── hosts  │ token  │ PluginHost: .cs3 → dex2jar → JVM │
│ mpv (optional)  ◀── mpv.rs               │        │ CloudStream library + extractors │
└──────────────────────────────────────────┘        └──────────────────────────────────┘
```

## Why a JVM sidecar

CloudStream plugins are Kotlin code compiled to **Android DEX**. Nothing but a JVM can run them, so instead of rewriting scrapers the engine:

1. Unzips the `.cs3` file and reads `manifest.json` → `pluginClassName`.
2. Converts `classes.dex` to a JVM jar with dex2jar (cached in `cache/plugins/`).
3. Loads it in its own class loader against the real CloudStream `library` and calls `load(context)`, which registers providers in `APIHolder`.
4. Provides stand-ins for the Android-only parts: `com.lagradost.cloudstream3.plugins.Plugin` and `android.content.Context`.

dex2jar runs with `dontSanitizeNames`. Without it, Kotlin names such as `Result.constructor-impl` get renamed and break linking.

### Android compatibility layer

Plugins are compiled against the Android app, so the engine provides desktop versions of what they actually call (`engine/src/main/kotlin/android/…` and `com/lagradost/cloudstream3/…`):

| Android API | Desktop behavior |
|---|---|
| `plugins.Plugin`, `Context`, `CloudStreamApp.context`, `AcraApplication.context` | Desktop context |
| `SharedPreferences`, `PreferenceManager` | Typed JSON files in `engine/prefs/` |
| `android.util.Base64`, `android.util.Log`, `android.net.Uri`, `android.text.TextUtils` | Same semantics on the JVM (Base64 keeps Android's flags and line wrapping) |
| `android.os.Handler` / `Looper` | One daemon scheduler thread |
| `CommonActivity.activity`, `Activity`, `Dialog`, `Intent` | No activity, so plugins skip their Android-only popups |

Plugins that need other Android APIs (resources, WebView-based Cloudflare bypasses, custom UI) fail to load or fail at runtime. The error shows on the Extensions page, and every other plugin keeps working. When a new plugin fails, the engine log names the missing class; add a stand-in next to the others.

## UI parity with the Android app

The UI follows CloudStream's own layouts and resources: `res/layout/*.xml`, `res/values/colors.xml`, `dimens.xml`, `strings.xml`, and the vector drawables used for icons and the logo.

| Android | Desktop |
|---|---|
| NavigationRailView: Home / Search / Library / Downloads / Settings | Same rail (62 px, unlabeled, filled icon when selected) |
| `fragment_home_head`: hero pager, Bookmark / Play / Info, Continue Watching, bookmark chips | Same; arrows or drag to page the hero |
| `home_result_grid`: 114×180 cards, quality/dub/sub badges, progress ring | Same |
| "Source" FAB + provider picker, expandable rows | Same |
| `fragment_search`: rounded bar, filter, TV-type chips, history, per-provider rows | Same |
| `fragment_result`: poster, meta, synopsis, watch status, genres, Resume, season/range/dub/sort selectors, large episode items | Same |
| Custom player: ⟲10 / ▶ / ⟳10, seek bar, Resize, Speed, Sources and subtitles, Next episode, auto-hide | Same, plus fullscreen and an "mpv" hand-off |
| `fragment_library`: tabs per watch status, search, sort | Same (local library; no AniList/MAL sync) |
| `main_settings`, `fragment_extensions`, `fragment_plugins` | Same structure; settings cover only what the desktop app supports |

Not identical yet: account sync, the light/other themes, Chromecast, and Android-only settings.

Fonts: the Android app uses Product Sans, which can't be redistributed. The app bundles **Google Sans Flex** (OFL, `src/assets/fonts/OFL.txt`) instead, plus Twemoji country flags (Windows has no flag emoji).

## Playback

Video hosts usually require a specific `Referer`/`User-Agent`, which a `<video>` tag cannot send. [proxy.rs](../app/src-tauri/src/proxy.rs) is a small local proxy that:

- replays requests with the stream's headers and forwards `Range` so seeking works,
- rewrites HLS playlists so variants, segments and keys also go through it,
- converts SRT subtitles to WebVTT,
- serves downloaded files back to the player.

MP4/WebM and HLS play in the app. For MKV, HEVC, DASH or anything else the webview can't decode, the **mpv** button (or *Settings → Player → Always play in mpv*) hands the stream and its headers to [mpv](https://mpv.io).

## Downloads

[downloads.rs](../app/src-tauri/src/downloads.rs) is a small download manager in Rust. Direct files resume with `Range` requests. HLS downloads fetch the segments of the chosen variant, decrypt AES-128 segments, and keep a local playlist so the built-in player can play them offline. State is saved in `downloads.json`, and the chosen folder in `download-folder.txt`, both in the app's data directory.

## Memory

Most of the RAM goes to the WebView decoding poster images, not to the Rust app (~10 MB). What keeps it low:

- **Thumbnails:** posters go through the `thumb:` scheme ([thumbs.rs](../app/src-tauri/src/thumbs.rs)), which downloads each one and shrinks it to the card's size (~0.15 MB decoded instead of ~6 MB). It uses a custom scheme rather than the localhost proxy because Chromium allows only 6 connections per host, which would queue a screen of posters. There's an 8 MB in-memory cache.
- **Lazy images:** images load near the viewport and drop their `src` when far away ([images.ts](../app/src/lib/images.ts)). Off-screen Home categories use `content-visibility: auto`.
- **Less data on Home:** categories are scraped 4 at a time as you scroll (`/api/home?from=&count=`), and each row shows 12 posters plus "See all".
- **Engine:** SerialGC with a 256 MB heap cap that shrinks after GC, plus an idle-time `System.gc()` ([IdleTrimmer.kt](../engine/src/main/kotlin/com/cloudstream/engine/IdleTrimmer.kt)), C1-only JIT and a small code cache ([engine.rs](../app/src-tauri/src/engine.rs) `JVM_ARGS`).
- **Player:** smaller hls.js buffers, and the decoder is released when the player closes.

Measured on Windows (dev build) after scrolling through ~30 MovieBox categories: about **450–550 MB private** in total, versus ~1.4 GB before these changes. Roughly 100 MB of that is the engine, and most of the rest is WebView2's own baseline. The WebView flags in `tauri.conf.json` only apply to WebView2, so macOS and Linux will differ.

## Prerequisites

| Tool | Why | Install |
|---|---|---|
| Rust (stable) | Tauri app | https://rustup.rs |
| Windows only: MSVC Build Tools | Rust linker | `winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"` |
| Linux only: WebKitGTK and friends | Tauri app | `sudo apt install build-essential curl wget file libssl-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev` |
| Node.js 20+ | Frontend build | https://nodejs.org |
| JDK 17 | Builds the engine; runs it in dev | Point `JAVA_HOME` at it, because Gradle 9.2.1 doesn't run on the newest JDKs |
| mpv (optional) | External player | `winget install mpv` / `brew install mpv` |

## Develop

```powershell
# 1. Build the engine once (and again after changing Kotlin code)
cd engine
./gradlew shadowJar          # -> engine/build/libs/engine.jar

# 2. Run the app (Vite dev server + Rust hot rebuild)
cd ../app
npm install
npm run tauri dev
```

In debug builds the app finds `engine/build/libs/engine.jar` automatically. To use another jar, set `CS_ENGINE_JAR`. Java is looked up in the bundled `jre/` folder first, then `JAVA_HOME`, then `PATH`.

You can also run the engine alone and call it with curl:

```powershell
$env:CS_ENGINE_PORT=4567; java -jar engine/build/libs/engine.jar
curl http://127.0.0.1:4567/health
```

## Release build

```powershell
cd engine
./gradlew shadowJar bundleJre       # engine.jar + trimmed 42 MB Java runtime
cd ../app
npm install
npm run tauri build -- --config src-tauri/tauri.release.conf.json
```

The release config bundles `engine.jar` and `jre/` as resources, so users don't need Java installed. Tauri produces MSI and NSIS installers on Windows, a `.app` and `.dmg` on macOS, and `.deb` and `.rpm` packages on Linux, all under `target/release/bundle/`. Build each platform on that OS, because the Java runtime is platform-specific.

AppImage is turned off in `tauri.conf.json` because `linuxdeploy` fails when it bundles this app.

### GitHub Actions

[build.yml](../.github/workflows/build.yml) builds Windows, macOS (Apple Silicon and Intel) and Linux on every push to `main` and every `v*` tag, and you can start it from the Actions tab. Each build also starts the packaged engine with its bundled Java runtime ([check-engine.sh](../.github/scripts/check-engine.sh)) to catch a broken package. The installers are attached to the run as artifacts.

### Publishing a release

Pushing a `v*` tag also publishes a GitHub release once every platform has passed. The `release` job renames the installers and uses [release-notes.md](../.github/release-notes.md) as the page, filling in `{{version}}` from the tag. Before tagging, update that file and the version in `app/package.json`, `app/src-tauri/tauri.conf.json`, `app/src-tauri/Cargo.toml` and `app/src/views/Settings.svelte`.

## Engine API

All `/api/*` routes need the `X-Engine-Token` header. The app generates a fresh token on each launch and passes it in `CS_ENGINE_TOKEN`.

| Method | Path | Params / body |
|---|---|---|
| GET | `/health` | — |
| GET | `/api/providers` | — |
| GET | `/api/home` | `provider`, `page`, optional `from` and `count` (a slice of the categories) |
| GET | `/api/search` | `query`, optional `provider` (default: all providers in parallel) |
| GET | `/api/load` | `provider`, `url` |
| GET | `/api/links` | `provider`, `data` |
| GET | `/api/plugins` | — |
| POST | `/api/plugins/install` | `{ "url": "…cs3", "id": "InternalName" }` |
| POST | `/api/plugins/uninstall` | `{ "id": "InternalName" }` |
| GET | `/api/repos` | — |
| GET | `/api/repos/plugins` | `url` (repo.json) |
| POST | `/api/repos/add` / `/api/repos/remove` | `{ "url": "…repo.json" }` |

Engine data (plugins, repos.json, dex cache) lives in the app's data directory under `engine/`. On Windows that's `%APPDATA%\app.cloudstream.desktop\engine`.

## Layout

```
engine/                         Kotlin sidecar
  src/main/kotlin/
    com/cloudstream/engine/     Main, Routes (Ktor), PluginHost, RepoStore, DTOs
    com/lagradost/.../          Plugin and CloudStreamApp stand-ins
    android/                    Android API stand-ins
app/
  src/                          Svelte UI (views/, components/, lib/)
  src-tauri/src/
    main.rs                     app setup, lifecycle
    engine.rs                   spawn/supervise engine, API passthrough
    proxy.rs                    header-injecting stream proxy + HLS rewrite
    thumbs.rs                   poster thumbnails (thumb: scheme)
    downloads.rs                download manager
    mpv.rs                      external player hand-off
    commands.rs                 Tauri commands used by the UI
```

## Roadmap

- [x] Engine: plugin loading, repos (URLs, `cloudstreamrepo://`, shortcodes), home/search/load/links
- [x] Android-matching UI: Home, Search, Details, Player, Library, Downloads, Settings, Extensions
- [x] Library (watch status), Continue Watching, episode progress, search history (webview storage)
- [x] Downloads: direct + HLS segment downloader in Rust
- [x] CI: GitHub Actions matrix for Windows, macOS and Linux installers
- [ ] Embedded libmpv render surface (replace the external mpv hand-off)
