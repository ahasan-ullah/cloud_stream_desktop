// Backup & restore, like Android's "Updates and Backup": settings, library, watch progress,
// search/watch history, repositories and the list of installed plugins in one JSON file.

import { backupApi, engine } from "./api";
import { refreshInstalled, refreshRepos } from "./extensions.svelte";
import { library, persistLibrary } from "./library.svelte";
import { refreshProviders, settings } from "./state.svelte";

const FORMAT = "cloudstream-desktop-backup";

export async function createBackup(): Promise<string> {
  const [repos, plugins] = await Promise.all([engine.repos(), engine.plugins()]);
  const data = {
    format: FORMAT,
    version: 1,
    createdAt: new Date().toISOString(),
    settings: $state.snapshot(settings),
    library: $state.snapshot(library),
    repos: repos.map((r) => r.url),
    plugins: plugins.map((p) => p.id),
  };
  const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-");
  return backupApi.save(`cloudstream-backup-${stamp}.json`, JSON.stringify(data, null, 2));
}

/** Applies a backup file; returns a short summary for a toast. */
export async function restoreBackup(text: string): Promise<string> {
  const data = JSON.parse(text);
  if (data?.format !== FORMAT) throw new Error("This is not a CloudStream Desktop backup");

  if (data.settings) Object.assign(settings, data.settings);
  const lib = data.library ?? {};
  library.bookmarks = { ...library.bookmarks, ...(lib.bookmarks ?? {}) };
  library.progress = { ...library.progress, ...(lib.progress ?? {}) };
  library.episodes = { ...library.episodes, ...(lib.episodes ?? {}) };
  if (Array.isArray(lib.history)) library.history = lib.history;
  if (Array.isArray(lib.watched)) library.watched = lib.watched;
  persistLibrary();

  let repos = 0;
  for (const url of data.repos ?? []) {
    try {
      await engine.addRepo(url);
      repos++;
    } catch {
      // Unreachable repositories are skipped.
    }
  }

  const installed = new Set((await engine.plugins()).map((p) => p.id));
  const wanted = new Set<string>((data.plugins ?? []).filter((id: string) => !installed.has(id)));
  let plugins = 0;
  if (wanted.size) {
    for (const repo of await engine.repos()) {
      for (const p of await engine.repoPlugins(repo.url).catch(() => [])) {
        if (!wanted.has(p.internalName)) continue;
        try {
          await engine.installPlugin(p.url, p.internalName);
          wanted.delete(p.internalName);
          plugins++;
        } catch {
          // Keep going with the rest.
        }
      }
    }
  }
  await Promise.all([refreshProviders(), refreshInstalled(), refreshRepos().catch(() => {})]);
  return `Restored library and settings · ${repos} repositories · ${plugins} plugins installed`;
}
