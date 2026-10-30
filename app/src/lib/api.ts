import { invoke } from "@tauri-apps/api/core";

export type EngineStatus =
  | { state: "starting" }
  | { state: "ready"; port: number }
  | { state: "failed"; message: string }
  | { state: "stopped" };

export type Headers = Record<string, string>;

export interface Provider {
  name: string;
  mainUrl: string;
  lang: string;
  hasMainPage: boolean;
  supportedTypes: string[];
  plugin?: string;
}

export interface SearchItem {
  name: string;
  url: string;
  apiName: string;
  type?: string;
  posterUrl?: string;
  posterHeaders?: Headers;
  quality?: string;
  dubStatus?: string[];
}

export interface HomeSection {
  name: string;
  items: SearchItem[];
  horizontal: boolean;
}

export interface HomeResult {
  provider: string;
  sections: HomeSection[];
  hasNext: boolean;
  errors: string[];
  /** Total number of home categories; request them in slices with `from`/`count`. */
  total: number;
}

export interface SearchGroup {
  provider: string;
  items: SearchItem[];
  error?: string;
}

export interface Episode {
  data: string;
  name?: string;
  season?: number;
  episode?: number;
  posterUrl?: string;
  description?: string;
  dubStatus?: string;
  runTime?: number;
  date?: number;
}

export interface Details {
  name: string;
  url: string;
  apiName: string;
  type?: string;
  posterUrl?: string;
  backgroundPosterUrl?: string;
  year?: number;
  plot?: string;
  tags: string[];
  duration?: number;
  playData?: string;
  episodes: Episode[];
  recommendations: SearchItem[];
  /** Out of 10. */
  rating?: number;
  contentRating?: string;
  comingSoon: boolean;
  actors: { name: string; image?: string; role?: string }[];
  trailers: string[];
}

export type DownloadState = "queued" | "downloading" | "paused" | "done" | "failed";

export interface DownloadRequest {
  title: string;
  episode?: string;
  poster?: string;
  provider: string;
  pageUrl: string;
  data: string;
  linkUrl: string;
  linkName: string;
  headers: Headers;
  hls: boolean;
  subtitles: { lang: string; url: string; headers: Headers }[];
}

export interface DownloadItem extends DownloadRequest {
  id: string;
  state: DownloadState;
  name: string;
  file?: string;
  bytesDone: number;
  bytesTotal?: number;
  segmentsDone: number;
  segmentsTotal: number;
  speed: number;
  error?: string;
  createdAt: number;
}

export interface LocalPlayback {
  url: string;
  hls: boolean;
  subtitles: { lang: string; url: string }[];
}

export const downloadsApi = {
  list: () => invoke<DownloadItem[]>("downloads_list"),
  start: (request: DownloadRequest) => invoke<DownloadItem>("download_start", { request }),
  pause: (id: string) => invoke<void>("download_pause", { id }),
  resume: (id: string) => invoke<void>("download_resume", { id }),
  remove: (id: string, deleteFiles: boolean) => invoke<void>("download_delete", { id, deleteFiles }),
  playback: (id: string) => invoke<LocalPlayback>("download_playback", { id }),
  reveal: (id?: string) => invoke<void>("reveal_download", { id: id ?? null }),
  /** Folder new downloads are saved to. */
  dir: () => invoke<string>("download_dir"),
  /** Opens a folder picker; resolves to the new folder, or null if cancelled. */
  chooseDir: () => invoke<string | null>("choose_download_dir"),
};

export const backupApi = {
  save: (fileName: string, json: string) => invoke<string>("save_backup", { fileName, json }),
  reveal: () => invoke<void>("reveal_backups"),
};

export type LinkType = "VIDEO" | "M3U8" | "DASH" | "TORRENT" | "MAGNET";

export interface StreamLink {
  name: string;
  source: string;
  url: string;
  quality: number;
  type: LinkType;
  headers: Headers;
}

export interface Subtitle {
  lang: string;
  url: string;
  headers: Headers;
}

export interface Links {
  links: StreamLink[];
  subtitles: Subtitle[];
}

export interface InstalledPlugin {
  id: string;
  name: string;
  version?: number;
  providers: string[];
  error?: string;
}

export interface Repo {
  url: string;
  name: string;
  description?: string;
}

export interface RepoPlugin {
  url: string;
  name: string;
  internalName: string;
  version: number;
  /** 0 = down, 1 = ok, 2 = slow, 3 = beta only */
  status: number;
  description?: string;
  authors: string[];
  language?: string;
  iconUrl?: string;
  tvTypes?: string[];
  fileSize?: number;
}

function call<T>(method: "GET" | "POST", path: string, query?: Record<string, string>, body?: unknown) {
  return invoke<T>("engine_request", { method, path, query, body });
}

export const engine = {
  status: () => invoke<EngineStatus>("engine_status"),
  restart: () => invoke<EngineStatus>("engine_restart"),

  providers: () => call<Provider[]>("GET", "/api/providers"),
  home: (provider: string, from = 0, count = 4) =>
    call<HomeResult>("GET", "/api/home", { provider, page: "1", from: String(from), count: String(count) }),
  search: (query: string, provider?: string) =>
    call<SearchGroup[]>("GET", "/api/search", provider ? { query, provider } : { query }),
  load: (provider: string, url: string) => call<Details>("GET", "/api/load", { provider, url }),
  links: (provider: string, data: string) => call<Links>("GET", "/api/links", { provider, data }),

  plugins: () => call<InstalledPlugin[]>("GET", "/api/plugins"),
  installPlugin: (url: string, id?: string) => call<InstalledPlugin>("POST", "/api/plugins/install", undefined, { url, id }),
  uninstallPlugin: (id: string) => call<InstalledPlugin[]>("POST", "/api/plugins/uninstall", undefined, { id }),

  repos: () => call<Repo[]>("GET", "/api/repos"),
  addRepo: (url: string) => call<Repo>("POST", "/api/repos/add", undefined, { url }),
  removeRepo: (url: string) => call<Repo[]>("POST", "/api/repos/remove", undefined, { url }),
  repoPlugins: (url: string) => call<RepoPlugin[]>("GET", "/api/repos/plugins", { url }),
};

/** Returns a local URL that replays `url` with the given headers (see src-tauri/src/proxy.rs). */
export function proxyUrl(url: string, headers: Headers, kind?: "m3u8" | "sub") {
  return invoke<string>("proxy_url", { url, headers, kind: kind ?? null });
}

export function openInMpv(args: { mpvPath?: string; url: string; title: string; headers: Headers; subtitles: string[] }) {
  return invoke<void>("open_in_mpv", args);
}

export function openUrl(url: string) {
  return invoke<void>("open_url", { url });
}

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
