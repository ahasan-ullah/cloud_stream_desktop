import { listen } from "@tauri-apps/api/event";
import { downloadsApi, type DownloadItem, type DownloadRequest, type StreamLink, type Subtitle } from "./api";
import { showToast } from "./state.svelte";

/** Mirrors the Rust download manager (src-tauri/src/downloads.rs), updated by its events. */
export const downloads = $state<{ items: DownloadItem[]; folder: string }>({ items: [], folder: "" });

/** Lets the user pick the downloads folder (used by Downloads and Settings). */
export async function chooseFolder() {
  try {
    const picked = await downloadsApi.chooseDir();
    if (picked) {
      downloads.folder = picked;
      showToast(`New downloads go to ${picked}`);
    }
  } catch (e) {
    showToast(e instanceof Error ? e.message : String(e));
  }
}

let started = false;

export async function initDownloads() {
  if (started) return;
  started = true;
  await listen<DownloadItem>("download", (e) => {
    const item = e.payload;
    const i = downloads.items.findIndex((d) => d.id === item.id);
    const previous = i >= 0 ? downloads.items[i].state : undefined;
    if (i >= 0) downloads.items[i] = item;
    else downloads.items.push(item);
    if (item.state === "done" && previous && previous !== "done") showToast(`Downloaded: ${label(item)}`);
    if (item.state === "failed" && previous && previous !== "failed") showToast(`Download failed: ${item.error ?? ""}`);
  });
  await listen<string>("download-removed", (e) => {
    downloads.items = downloads.items.filter((d) => d.id !== e.payload);
  });
  [downloads.items, downloads.folder] = await Promise.all([downloadsApi.list(), downloadsApi.dir()]);
}

export const label = (d: DownloadItem) => (d.episode ? `${d.title} · ${d.episode}` : d.title);

/** The download of an episode/movie (matched by provider + data), if any. */
export function downloadFor(provider: string, data: string): DownloadItem | undefined {
  return downloads.items.find((d) => d.provider === provider && d.data === data);
}

export const canDownload = (l: StreamLink) => l.type === "VIDEO" || l.type === "M3U8";

export async function startDownload(
  target: Omit<DownloadRequest, "linkUrl" | "linkName" | "headers" | "hls" | "subtitles">,
  link: StreamLink,
  subtitles: Subtitle[],
) {
  await downloadsApi.start({
    ...target,
    linkUrl: link.url,
    linkName: link.name,
    headers: link.headers,
    hls: link.type === "M3U8",
    subtitles,
  });
  showToast("Download started");
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${units[u]}`;
}

export function progressOf(d: DownloadItem): number {
  if (d.state === "done") return 1;
  if (d.segmentsTotal) return d.segmentsDone / d.segmentsTotal;
  if (d.bytesTotal) return d.bytesDone / d.bytesTotal;
  return 0;
}
