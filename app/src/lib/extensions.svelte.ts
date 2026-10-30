import { engine, type InstalledPlugin, type Repo, type RepoPlugin } from "./api";
import { refreshProviders } from "./state.svelte";
import { uniqueBy } from "./unique";

/** Repositories, their plugin lists and installed plugins, shared by the Extensions screens. */
export const ext = $state({
  repos: [] as Repo[],
  installed: [] as InstalledPlugin[],
  available: {} as Record<string, RepoPlugin[]>,
  loadingRepo: {} as Record<string, boolean>,
});

export async function refreshInstalled() {
  ext.installed = await engine.plugins();
}

export async function refreshRepos() {
  [ext.repos, ext.installed] = await Promise.all([engine.repos(), engine.plugins()]);
  await Promise.all(ext.repos.map((r) => loadRepoPlugins(r.url)));
}

export async function loadRepoPlugins(url: string, force = false) {
  if (ext.available[url] && !force) return;
  ext.loadingRepo[url] = true;
  try {
    ext.available[url] = uniqueBy(await engine.repoPlugins(url), (p) => p.internalName);
  } catch {
    ext.available[url] ??= [];
  } finally {
    ext.loadingRepo[url] = false;
  }
}

export async function installPlugin(p: RepoPlugin) {
  const result = await engine.installPlugin(p.url, p.internalName);
  await Promise.all([refreshInstalled(), refreshProviders()]);
  return result;
}

/** Installed plugins for which a repository has a newer version. */
export function outdated(): RepoPlugin[] {
  const installed = new Map(ext.installed.map((p) => [p.id, p.version ?? 0]));
  const newest = new Map<string, RepoPlugin>();
  for (const list of Object.values(ext.available)) {
    for (const p of list) {
      const have = installed.get(p.internalName);
      if (have === undefined || p.version <= have) continue;
      if ((newest.get(p.internalName)?.version ?? -1) < p.version) newest.set(p.internalName, p);
    }
  }
  return [...newest.values()];
}

/** Installs the newer versions; returns how many were updated. */
export async function updatePlugins(list = outdated()): Promise<number> {
  let updated = 0;
  for (const p of list) {
    try {
      await engine.installPlugin(p.url, p.internalName);
      updated++;
    } catch {
      // A broken update keeps the installed version.
    }
  }
  if (updated) await Promise.all([refreshInstalled(), refreshProviders()]);
  return updated;
}

export async function uninstallPlugin(id: string) {
  ext.installed = await engine.uninstallPlugin(id);
  await refreshProviders();
}
