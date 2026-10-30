// Mirrors the Android search/plugin "tvtypes" chips and the TvType groups behind them.

export const TV_TYPE_CHIPS: { label: string; types: string[] }[] = [
  { label: "Movies", types: ["Movie", "AnimeMovie"] },
  { label: "TV Series", types: ["TvSeries"] },
  { label: "Anime", types: ["Anime", "AnimeMovie", "OVA"] },
  { label: "Asian Dramas", types: ["AsianDrama"] },
  { label: "Cartoons", types: ["Cartoon"] },
  { label: "Documentaries", types: ["Documentary"] },
  { label: "Livestreams", types: ["Live"] },
  { label: "Torrents", types: ["Torrent"] },
  { label: "NSFW", types: ["NSFW"] },
  { label: "Others", types: ["Others", "Music", "AudioBook", "Audio", "Podcast", "CustomMedia", "Video"] },
];

const TYPE_LABELS: Record<string, string> = {
  Movie: "Movie",
  AnimeMovie: "Movie",
  TvSeries: "TV Series",
  Cartoon: "Cartoon",
  Anime: "Anime",
  OVA: "OVA",
  Torrent: "Torrent",
  Documentary: "Documentary",
  AsianDrama: "Asian Drama",
  Live: "Livestream",
  NSFW: "NSFW",
  Others: "Other",
};

export const typeLabel = (t?: string) => (t ? (TYPE_LABELS[t] ?? t) : "");

// SearchQuality enum name -> label Android shows on cards.
const QUALITY_LABELS: Record<string, string> = {
  Cam: "Cam", CamRip: "CamRip", HdCam: "HDCam", Telesync: "TS", WorkPrint: "WP", Telecine: "TC", HQ: "HQ",
  HD: "HD", HDR: "HDR", BlueRay: "BluRay", DVD: "DVD", SD: "SD", FourK: "4K", UHD: "UHD", SDR: "SDR", WebRip: "WEB",
};

export const qualityLabel = (q?: string) => (q ? (QUALITY_LABELS[q] ?? q) : "");

/** Types selected by a set of chip labels; empty = everything. */
export function typesFor(selected: string[]): Set<string> {
  return new Set(TV_TYPE_CHIPS.filter((c) => selected.includes(c.label)).flatMap((c) => c.types));
}

const LANG_TO_REGION: Record<string, string> = {
  en: "GB", ar: "SA", bn: "BD", cs: "CZ", da: "DK", el: "GR", fa: "IR", he: "IL", hi: "IN", ja: "JP", ko: "KR",
  ms: "MY", sv: "SE", ta: "IN", te: "IN", uk: "UA", ur: "PK", vi: "VN", zh: "CN", ml: "IN", kn: "IN", mr: "IN",
  gu: "IN", pa: "IN", tl: "PH", fil: "PH", sq: "AL", sr: "RS", sl: "SI", et: "EE", ka: "GE", hy: "AM", kk: "KZ",
  ne: "NP", si: "LK", my: "MM", km: "KH", lo: "LA", sw: "KE", af: "ZA", zu: "ZA", ca: "ES", eu: "ES", gl: "ES",
};

/** Flag emoji for a plugin language code, as the Android extension list shows. */
export function flagFor(lang?: string): string {
  if (!lang) return "";
  const code = lang.toLowerCase();
  if (code === "mult" || code === "multi") return "🌐";
  const region = (LANG_TO_REGION[code] ?? (code.length === 2 ? code : "")).toUpperCase();
  if (region.length !== 2) return lang;
  return String.fromCodePoint(...[...region].map((c) => 0x1f1e6 + c.charCodeAt(0) - 65));
}
