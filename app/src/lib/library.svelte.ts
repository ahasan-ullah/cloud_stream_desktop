// User data the Android app keeps in its DataStore: watch status (bookmarks), playback
// progress ("Continue Watching") and search history. Persisted in the webview's storage.

import type { LocalPlayback, SearchItem } from "./api";

export type WatchStatus = "watching" | "planToWatch" | "onHold" | "completed" | "dropped";

/** Same order and labels as the Android library tabs. */
export const WATCH_STATUSES: { id: WatchStatus; label: string }[] = [
  { id: "watching", label: "Watching" },
  { id: "planToWatch", label: "Plan to Watch" },
  { id: "onHold", label: "On-Hold" },
  { id: "completed", label: "Completed" },
  { id: "dropped", label: "Dropped" },
];

export const statusLabel = (s: WatchStatus | undefined) => WATCH_STATUSES.find((w) => w.id === s)?.label ?? "None";

export interface Bookmark {
  status: WatchStatus;
  item: SearchItem;
  updatedAt: number;
}

/** Last thing watched for a title; drives "Continue Watching" and the Resume button. */
export interface TitleProgress {
  item: SearchItem;
  data: string;
  label: string;
  season?: number;
  episode?: number;
  position: number;
  duration: number;
  updatedAt: number;
}

export interface QueueEntry {
  data: string;
  label: string;
  season?: number;
  episode?: number;
}

/** One entry of the watch history (Library → History). */
export interface WatchedEntry {
  item: SearchItem;
  data: string;
  label: string;
  time: number;
}

export const KEYS = {
  bookmarks: "cs-bookmarks",
  progress: "cs-progress",
  episodes: "cs-episode-progress",
  history: "cs-search-history",
  watched: "cs-watch-history",
};

function read<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage unavailable or full: keep in memory only.
  }
}

export const titleKey = (provider: string, url: string) => `${provider}\u0001${url}`;

export const library = $state({
  bookmarks: read<Record<string, Bookmark>>(KEYS.bookmarks, {}),
  progress: read<Record<string, TitleProgress>>(KEYS.progress, {}),
  /** provider|episodeData -> [position, duration] in seconds */
  episodes: read<Record<string, [number, number]>>(KEYS.episodes, {}),
  history: read<{ query: string; time: number }[]>(KEYS.history, []),
  watched: read<WatchedEntry[]>(KEYS.watched, []),
});

/** Writes every library collection back to storage (after a restore). */
export function persistLibrary() {
  write(KEYS.bookmarks, library.bookmarks);
  write(KEYS.progress, library.progress);
  write(KEYS.episodes, library.episodes);
  write(KEYS.history, library.history);
  write(KEYS.watched, library.watched);
}

// ---- watch history

const MAX_WATCHED = 300;

export function addWatched(item: SearchItem, entry: QueueEntry) {
  const latest = library.watched[0];
  if (latest && latest.item.url === item.url && latest.data === entry.data) {
    latest.time = Date.now();
  } else {
    library.watched = [
      { item: $state.snapshot(item), data: entry.data, label: entry.label, time: Date.now() },
      ...library.watched,
    ].slice(0, MAX_WATCHED);
  }
  write(KEYS.watched, library.watched);
}

export function clearWatched() {
  library.watched = [];
  write(KEYS.watched, library.watched);
}

/** Marks an episode fully watched, or clears its progress. */
export function setEpisodeWatched(provider: string, data: string, watched: boolean) {
  const key = titleKey(provider, data);
  if (watched) library.episodes[key] = [1, 1];
  else delete library.episodes[key];
  write(KEYS.episodes, library.episodes);
}

// ---- bookmarks

export function bookmarkOf(provider: string, url: string): WatchStatus | undefined {
  return library.bookmarks[titleKey(provider, url)]?.status;
}

export function setBookmark(item: SearchItem, status: WatchStatus | undefined) {
  const key = titleKey(item.apiName, item.url);
  if (status) library.bookmarks[key] = { status, item: $state.snapshot(item), updatedAt: Date.now() };
  else delete library.bookmarks[key];
  write(KEYS.bookmarks, library.bookmarks);
}

export function bookmarksWith(status: WatchStatus): Bookmark[] {
  return Object.values(library.bookmarks)
    .filter((b) => b.status === status)
    .sort((a, b) => b.updatedAt - a.updatedAt);
}

// ---- playback progress

const MAX_EPISODE_ENTRIES = 3000;

export function episodeProgress(provider: string, data: string): number {
  const p = library.episodes[titleKey(provider, data)];
  return p && p[1] > 0 ? Math.min(1, p[0] / p[1]) : 0;
}

export function savedPosition(provider: string, data: string): number {
  return library.episodes[titleKey(provider, data)]?.[0] ?? 0;
}

export function saveProgress(item: SearchItem, entry: QueueEntry, position: number, duration: number) {
  if (!duration || !isFinite(duration)) return;
  library.episodes[titleKey(item.apiName, entry.data)] = [Math.floor(position), Math.floor(duration)];
  const keys = Object.keys(library.episodes);
  if (keys.length > MAX_EPISODE_ENTRIES) delete library.episodes[keys[0]];
  write(KEYS.episodes, library.episodes);

  const key = titleKey(item.apiName, item.url);
  // Finished titles drop out of Continue Watching, like on Android.
  if (position / duration > 0.95) delete library.progress[key];
  else
    library.progress[key] = {
      item: $state.snapshot(item),
      ...entry,
      position,
      duration,
      updatedAt: Date.now(),
    };
  write(KEYS.progress, library.progress);
}

export function removeProgress(provider: string, url: string) {
  delete library.progress[titleKey(provider, url)];
  write(KEYS.progress, library.progress);
}

export function continueWatching(): TitleProgress[] {
  return Object.values(library.progress).sort((a, b) => b.updatedAt - a.updatedAt);
}

// ---- search history

export function addHistory(query: string) {
  const q = query.trim();
  if (!q) return;
  library.history = [{ query: q, time: Date.now() }, ...library.history.filter((h) => h.query !== q)].slice(0, 30);
  write(KEYS.history, library.history);
}

export function removeHistory(query: string) {
  library.history = library.history.filter((h) => h.query !== query);
  write(KEYS.history, library.history);
}

export function clearHistory() {
  library.history = [];
  write(KEYS.history, library.history);
}

// ---- what the player should play (set by the details page before opening it)

export const playback = $state<{
  item: SearchItem | null;
  queue: QueueEntry[];
  index: number;
  poster?: string;
  /** Set when playing a finished download instead of streaming. */
  local?: LocalPlayback | null;
}>({
  item: null,
  queue: [],
  index: 0,
});

export function episodeLabel(e: { name?: string; season?: number; episode?: number }, index: number): string {
  const num = e.episode ?? index + 1;
  const prefix = e.season != null ? `S${e.season}:E${num}` : `E${num}`;
  return e.name ? `${prefix} ${e.name}` : prefix;
}
