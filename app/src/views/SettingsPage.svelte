<script lang="ts">
  // Preference screens in Android PreferenceFragment style.
  import Dialog from "../components/Dialog.svelte";
  import Icon from "../components/Icon.svelte";
  import { backupApi, downloadsApi, engine, errorMessage } from "../lib/api";
  import { createBackup, restoreBackup } from "../lib/backup.svelte";
  import { chooseFolder, downloads } from "../lib/downloads.svelte";
  import { ext, outdated, refreshRepos, updatePlugins } from "../lib/extensions.svelte";
  import { clearHistory, clearWatched, library } from "../lib/library.svelte";
  import { router, type SettingsPageName } from "../lib/router.svelte";
  import { ACCENTS, engineState, settings, showToast } from "../lib/state.svelte";
  import { flagFor, typeLabel } from "../lib/tvtypes";

  let { page }: { page: SettingsPageName } = $props();

  const TITLES: Record<SettingsPageName, string> = {
    "settings-general": "General",
    "settings-player": "Player",
    "settings-providers": "Providers",
    "settings-layout": "Layout",
    "settings-backup": "Updates and Backup",
  };
  const SEEK_OPTIONS = [5, 10, 15, 20, 25, 30];

  let seekDialog = $state(false);
  let restarting = $state(false);
  let busy = $state("");
  let fileInput = $state<HTMLInputElement>();

  async function backup() {
    busy = "backup";
    try {
      const path = await createBackup();
      showToast(`Backup saved to ${path}`);
      await backupApi.reveal();
    } catch (e) {
      showToast(errorMessage(e));
    } finally {
      busy = "";
    }
  }

  async function restore(file: File | undefined) {
    if (!file) return;
    busy = "restore";
    try {
      showToast(await restoreBackup(await file.text()));
    } catch (e) {
      showToast(errorMessage(e));
    } finally {
      busy = "";
      if (fileInput) fileInput.value = "";
    }
  }

  async function checkUpdates() {
    busy = "updates";
    try {
      await refreshRepos();
      const count = outdated().length;
      if (!count) {
        showToast("All plugins are up to date");
        return;
      }
      const updated = await updatePlugins();
      showToast(`Updated ${updated} of ${count} plugins`);
    } catch (e) {
      showToast(errorMessage(e));
    } finally {
      busy = "";
    }
  }

  function clearProgress() {
    library.progress = {};
    library.episodes = {};
    localStorage.removeItem("cs-progress");
    localStorage.removeItem("cs-episode-progress");
    showToast("Watch history cleared");
  }

  function clearLibrary() {
    library.bookmarks = {};
    localStorage.removeItem("cs-bookmarks");
    showToast("Library cleared");
  }

  async function restart() {
    restarting = true;
    try {
      await engine.restart();
    } finally {
      restarting = false;
    }
  }

  const status = $derived(engineState.status);
</script>

<div class="screen">
  <div class="toolbar">
    <button class="icon-btn" title="Go Back" onclick={() => router.back()}><Icon name="back" size={26} /></button>
    <h1>{TITLES[page]}</h1>
  </div>

  <div class="page">
    {#if page === "settings-general"}
      <button class="pref" onclick={() => (clearHistory(), showToast("Search history cleared"))}>
        <span class="pref-text"><span>Clear search history</span><span class="summary">{library.history.length} entries</span></span>
      </button>
      <button class="pref" onclick={clearProgress}>
        <span class="pref-text">
          <span>Clear watch history</span>
          <span class="summary">Removes Continue Watching and episode progress</span>
        </span>
      </button>
      <button class="pref" onclick={() => (clearWatched(), showToast("Watch history list cleared"))}>
        <span class="pref-text">
          <span>Clear history list</span>
          <span class="summary">{library.watched.length} entries in Library → History</span>
        </span>
      </button>
      <button class="pref" onclick={clearLibrary}>
        <span class="pref-text">
          <span>Clear library</span>
          <span class="summary">{Object.keys(library.bookmarks).length} bookmarked titles</span>
        </span>
      </button>
      <div class="pref">
        <span class="pref-text">
          <span>Downloads folder</span>
          <span class="summary">{downloads.folder || "Videos\\CloudStream"}</span>
        </span>
        <button class="btn btn-black" onclick={chooseFolder}>Change</button>
        <button class="btn btn-black" onclick={() => downloadsApi.reveal()}>Open</button>
      </div>
    {:else if page === "settings-layout"}
      <h3 class="category">Accent color</h3>
      <div class="swatches">
        {#each ACCENTS as a (a.color)}
          <button
            class="swatch"
            class:on={settings.accent === a.color}
            style:background={a.color}
            title={a.name}
            aria-label={a.name}
            onclick={() => (settings.accent = a.color)}
          ></button>
        {/each}
      </div>
      <h3 class="category">Poster size</h3>
      <div class="segmented">
        {#each [["small", "Small"], ["normal", "Normal"], ["large", "Large"]] as [id, name]}
          <button
            class:on={settings.posterSize === id}
            onclick={() => (settings.posterSize = id as typeof settings.posterSize)}>{name}</button
          >
        {/each}
      </div>
    {:else if page === "settings-backup"}
      <label class="pref">
        <span class="pref-text">
          <span>Auto-update plugins</span>
          <span class="summary">Install newer versions from your repositories when the app starts</span>
        </span>
        <input type="checkbox" class="switch" bind:checked={settings.autoUpdatePlugins} />
      </label>
      <button class="pref" onclick={checkUpdates} disabled={!!busy}>
        <span class="pref-text">
          <span>Update plugins now</span>
          <span class="summary">{ext.installed.length} installed</span>
        </span>
        {#if busy === "updates"}<div class="spinner small"></div>{/if}
      </button>
      <h3 class="category">Backup</h3>
      <button class="pref" onclick={backup} disabled={!!busy}>
        <span class="pref-text">
          <span>Back up data</span>
          <span class="summary">Library, watch progress, history, settings, repositories and plugin list → Documents\CloudStream</span>
        </span>
        {#if busy === "backup"}<div class="spinner small"></div>{/if}
      </button>
      <button class="pref" onclick={() => fileInput?.click()} disabled={!!busy}>
        <span class="pref-text">
          <span>Restore data</span>
          <span class="summary">Pick a backup file; missing repositories and plugins are installed again</span>
        </span>
        {#if busy === "restore"}<div class="spinner small"></div>{/if}
      </button>
      <input
        bind:this={fileInput}
        type="file"
        accept=".json,application/json"
        hidden
        onchange={(e) => restore(e.currentTarget.files?.[0])}
      />
    {:else if page === "settings-player"}
      <label class="pref">
        <span class="pref-text">
          <span>Autoplay next episode</span>
          <span class="summary">Start the next episode when one ends</span>
        </span>
        <input type="checkbox" class="switch" bind:checked={settings.autoNextEpisode} />
      </label>
      <button class="pref" onclick={() => (seekDialog = true)}>
        <span class="pref-text"><span>Fast forward / rewind amount</span><span class="summary">{settings.seekSeconds} seconds</span></span>
      </button>
      <label class="pref">
        <span class="pref-text">
          <span>Always play in mpv</span>
          <span class="summary">The built-in player handles MP4 and HLS; mpv also plays MKV, HEVC and DASH</span>
        </span>
        <input type="checkbox" class="switch" bind:checked={settings.preferMpv} />
      </label>
      <div class="pref column">
        <span class="pref-text"><span>mpv executable</span><span class="summary">Leave empty to find mpv on PATH</span></span>
        <input type="text" class="text" placeholder="C:\Program Files\mpv\mpv.exe" bind:value={settings.mpvPath} />
      </div>
    {:else}
      <div class="pref">
        <span class="pref-text">
          <span>CloudStream engine</span>
          <span class="summary">
            <span class="dot {status.state}"></span>{status.state}{status.state === "ready" ? ` on port ${status.port}` : ""}
            {#if status.state === "failed"}: {status.message}{/if}
          </span>
        </span>
        <button class="btn btn-black" onclick={restart} disabled={restarting || status.state === "starting"}>Restart</button>
      </div>
      <h3 class="category">Installed providers</h3>
      {#each engineState.providers as p (p.name)}
        <div class="pref">
          <span class="flag">{flagFor(p.lang)}</span>
          <span class="pref-text">
            <span>{p.name}</span>
            <span class="summary">{p.supportedTypes.map(typeLabel).join(", ")}</span>
          </span>
        </div>
      {:else}
        <p class="summary pad">No providers installed. Add them from Extensions.</p>
      {/each}
    {/if}
  </div>
</div>

{#if seekDialog}
  <Dialog title="Fast forward / rewind amount" onclose={() => (seekDialog = false)} width={320}>
    {#each SEEK_OPTIONS as s}
      <button class="list-item" onclick={() => ((settings.seekSeconds = s), (seekDialog = false))}>
        <span class="radio" class:on={settings.seekSeconds === s}></span>{s} seconds
      </button>
    {/each}
  </Dialog>
{/if}

<style>
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .page {
    flex: 1;
    padding: 8px 0 30px;
  }

  .pref {
    display: flex;
    align-items: center;
    gap: 16px;
    width: 100%;
    min-height: 64px;
    padding: 10px 20px;
    text-align: left;
    cursor: pointer;
  }

  div.pref {
    cursor: default;
  }

  .pref:hover {
    background: var(--ripple);
  }

  .pref.column {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
  }

  .pref-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 16px;
  }

  .summary {
    font-size: 14px;
    color: var(--text-gray);
  }

  .pad {
    padding: 10px 20px;
  }

  .category {
    padding: 18px 20px 6px;
    font-size: 14px;
    font-weight: 700;
    color: var(--primary);
  }

  .flag {
    font-size: 22px;
    width: 28px;
    text-align: center;
  }

  .text {
    height: 42px;
    padding: 0 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: var(--bg);
    outline: none;
  }

  .text:focus {
    border-color: var(--primary);
  }

  /* Material switch */
  .switch {
    appearance: none;
    position: relative;
    width: 44px;
    height: 24px;
    border-radius: 12px;
    background: var(--search);
    border: 2px solid var(--text-gray);
    cursor: pointer;
    transition: background 0.15s;
    flex-shrink: 0;
  }

  .switch::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--text-gray);
    transition: all 0.15s;
  }

  .switch:checked {
    background: var(--primary);
    border-color: var(--primary);
  }

  .switch:checked::after {
    left: 21px;
    top: 1px;
    width: 18px;
    height: 18px;
    background: #fff;
  }

  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    padding: 8px 20px 16px;
  }

  .swatch {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    border: 3px solid transparent;
    box-shadow: inset 0 0 0 2px rgba(0, 0, 0, 0.25);
  }

  .swatch.on {
    border-color: var(--text);
  }

  .segmented {
    display: flex;
    gap: 8px;
    padding: 8px 20px 16px;
  }

  .segmented button {
    height: 38px;
    padding: 0 18px;
    border-radius: 8px;
    background: var(--bg-gray);
    border: 1px solid rgba(255, 255, 255, 0.12);
    font-weight: 700;
  }

  .segmented button.on {
    background: var(--primary);
    border-color: var(--primary);
    color: #fff;
  }

  .pref:disabled {
    opacity: 0.6;
  }

  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 6px;
    border-radius: 50%;
    background: var(--text-gray);
  }

  .dot.ready {
    background: var(--ok);
  }

  .dot.starting {
    background: var(--warn);
  }

  .dot.failed {
    background: var(--fail);
  }
</style>
