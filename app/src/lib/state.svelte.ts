import { listen } from "@tauri-apps/api/event";
import { engine, type EngineStatus, type Provider } from "./api";

export interface Settings {
  homeProvider: string;
  mpvPath: string;
  preferMpv: boolean;
  /** Providers unticked in the search filter dialog. */
  searchExcluded: string[];
  /** Seconds skipped by the rewind / fast-forward buttons. */
  seekSeconds: number;
  autoNextEpisode: boolean;
  /** Install newer plugin versions from repositories at startup. */
  autoUpdatePlugins: boolean;
  /** Remembered player volume (0-1) and speed. */
  volume: number;
  speed: number;
  /** Subtitle size in percent and background style. */
  subtitleSize: number;
  subtitleBackground: "outline" | "shadow" | "box";
  /** Accent color (Android's "primary color" choices). */
  accent: string;
  posterSize: "small" | "normal" | "large";
}

const SETTINGS_KEY = "cs-settings";
const DEFAULT_SETTINGS: Settings = {
  homeProvider: "",
  mpvPath: "",
  preferMpv: false,
  searchExcluded: [],
  seekSeconds: 10,
  autoNextEpisode: true,
  autoUpdatePlugins: true,
  volume: 1,
  speed: 1,
  subtitleSize: 100,
  subtitleBackground: "outline",
  accent: "#3d50fa",
  posterSize: "normal",
};

/** res/values/colors.xml primary color options. */
export const ACCENTS: { name: string; color: string }[] = [
  { name: "Normal", color: "#3d50fa" },
  { name: "Blue", color: "#5664b7" },
  { name: "Cool Blue", color: "#408cac" },
  { name: "Purple", color: "#6200ea" },
  { name: "Lavender", color: "#6f55af" },
  { name: "Pink", color: "#ff1493" },
  { name: "Party", color: "#ea596e" },
  { name: "Red", color: "#d50000" },
  { name: "Orange", color: "#ce8500" },
  { name: "Banana", color: "#e4d448" },
  { name: "Green", color: "#00bfa5" },
  { name: "Apple", color: "#48e484" },
];

function loadSettings(): Settings {
  try {
    return { ...DEFAULT_SETTINGS, ...JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "{}") };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export const settings = $state<Settings>(loadSettings());

export function persistSettings(snapshot: string) {
  try {
    localStorage.setItem(SETTINGS_KEY, snapshot);
  } catch {
    // Storage can be unavailable; settings then last for the session only.
  }
}

export const engineState = $state<{ status: EngineStatus; providers: Provider[] }>({
  status: { state: "starting" },
  providers: [],
});

export async function refreshProviders() {
  try {
    engineState.providers = await engine.providers();
  } catch {
    engineState.providers = [];
  }
}

function applyStatus(status: EngineStatus) {
  engineState.status = status;
  if (status.state === "ready") void refreshProviders();
}

export async function initEngine() {
  await listen<EngineStatus>("engine-status", (e) => applyStatus(e.payload));
  applyStatus(await engine.status());
}

/** Window width, bound in App.svelte. */
export const viewport = $state({ width: window.innerWidth });

const POSTER_SCALE = { small: 0.85, normal: 1, large: 1.25 };

/** Poster card width: Android's 114dp, growing a little on large windows so rows don't look tiny. */
export const cardWidth = () =>
  Math.round(Math.max(114, Math.min(150, viewport.width * 0.075)) * POSTER_SCALE[settings.posterSize]);

/** Android-style toast. */
export const toast = $state<{ text: string; id: number }>({ text: "", id: 0 });

export function showToast(text: string) {
  toast.text = text;
  const id = ++toast.id;
  setTimeout(() => {
    if (toast.id === id) toast.text = "";
  }, 2500);
}
