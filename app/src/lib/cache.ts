import { engine, type Details } from "./api";
import { titleKey } from "./library.svelte";
import { uniqueBy } from "./unique";

// Like the Android back stack keeping the result fragment alive: reopening a title
// (e.g. after the player) reuses the loaded details instead of scraping again.
const details = new Map<string, Promise<Details>>();
const MAX = 100;

export function loadDetails(provider: string, url: string, force = false): Promise<Details> {
  const key = titleKey(provider, url);
  if (force) details.delete(key);
  let p = details.get(key);
  if (!p) {
    p = engine.load(provider, url).then((d) => ({
      ...d,
      episodes: uniqueBy(d.episodes, (e) => `${e.dubStatus ?? ""}|${e.data}`),
      recommendations: uniqueBy(d.recommendations, (r) => r.url),
    }));
    p.catch(() => details.delete(key));
    details.set(key, p);
    if (details.size > MAX) details.delete(details.keys().next().value!);
  }
  return p;
}
