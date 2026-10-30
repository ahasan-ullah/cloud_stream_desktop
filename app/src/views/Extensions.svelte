<script lang="ts">
  // Android res/layout/fragment_extensions.xml
  import { untrack } from "svelte";
  import Dialog from "../components/Dialog.svelte";
  import Icon from "../components/Icon.svelte";
  import { engine, errorMessage, openUrl, type Repo } from "../lib/api";
  import { ext, loadRepoPlugins, refreshRepos } from "../lib/extensions.svelte";
  import { router } from "../lib/router.svelte";
  import { engineState, showToast } from "../lib/state.svelte";

  const COMMUNITY_REPOS = "https://recloudstream.github.io/csdocs/repositories/";

  let addDialog = $state(false);
  let repoUrl = $state("");
  let adding = $state(false);
  let addError = $state("");
  let confirmRemove = $state<Repo | null>(null);

  const ready = $derived(engineState.status.state === "ready");

  $effect(() => {
    if (ready) untrack(() => refreshRepos().catch((e) => showToast(errorMessage(e))));
  });

  const installedIds = $derived(new Set(ext.installed.map((p) => p.id)));
  const counts = $derived.by(() => {
    const all = new Map<string, true>();
    for (const list of Object.values(ext.available)) for (const p of list) all.set(p.internalName, true);
    const disabled = ext.installed.filter((p) => p.error).length;
    const downloaded = ext.installed.length - disabled;
    const notDownloaded = [...all.keys()].filter((id) => !installedIds.has(id)).length;
    return { downloaded, disabled, notDownloaded, total: downloaded + disabled + notDownloaded };
  });

  async function add() {
    adding = true;
    addError = "";
    try {
      const repo = await engine.addRepo(repoUrl.trim());
      repoUrl = "";
      addDialog = false;
      ext.repos = await engine.repos();
      await loadRepoPlugins(repo.url, true);
      showToast(`Added ${repo.name}`);
    } catch (e) {
      addError = errorMessage(e);
    } finally {
      adding = false;
    }
  }

  async function remove(repo: Repo) {
    confirmRemove = null;
    ext.repos = await engine.removeRepo(repo.url);
    delete ext.available[repo.url];
  }

  const pct = (n: number) => (counts.total ? (n / counts.total) * 100 : 0);
</script>

<div class="screen">
  <div class="toolbar">
    <button class="icon-btn" title="Go Back" onclick={() => router.back()}><Icon name="back" size={26} /></button>
    <h1>Extensions</h1>
  </div>

  <div class="page list">
    {#if !ext.repos.length}
      <div class="center-fill blank">
        <Icon name="extension" size={30} />
        <p>CloudStream has no sites installed by default. You need to install the sites from repositories.</p>
        <button class="btn btn-white" onclick={() => openUrl(COMMUNITY_REPOS)}>View community repositories</button>
      </div>
    {:else}
      {#each ext.repos as repo (repo.url)}
        <div class="repo-item">
          <button class="repo-main" onclick={() => router.push({ name: "plugins", repo })}>
            <span class="entry-icon"><Icon name="extension" size={22} /></span>
            <span class="texts">
              <span class="main-text">{repo.name}</span>
              <span class="sub-text">{repo.url}</span>
            </span>
            {#if ext.loadingRepo[repo.url]}
              <div class="spinner small"></div>
            {:else if ext.available[repo.url]}
              <span class="gray small">{ext.available[repo.url].length} plugins</span>
            {/if}
          </button>
          <button class="icon-btn" title="Delete" onclick={() => (confirmRemove = repo)}><Icon name="delete" size={22} /></button>
        </div>
      {/each}
    {/if}
    <div style="height: 80px"></div>
  </div>

  <button class="storage" onclick={() => router.push({ name: "plugins", repo: null })} title="Installed plugins">
    <span class="storage-title">Extensions</span>
    <span class="bar">
      <span class="seg downloaded" style:width="{pct(counts.downloaded)}%"></span>
      <span class="seg disabled" style:width="{pct(counts.disabled)}%"></span>
      <span class="seg not" style:width="{pct(counts.notDownloaded)}%"></span>
    </span>
    <span class="legend">
      <span><i class="downloaded"></i>Downloaded: {counts.downloaded}</span>
      <span><i class="disabled"></i>Disabled: {counts.disabled}</span>
      <span><i class="not"></i>Not downloaded: {counts.notDownloaded}</span>
    </span>
  </button>

  <button class="fab add" onclick={() => (addDialog = true)} disabled={!ready}>
    <Icon name="add" size={24} /> Add repository
  </button>
</div>

{#if addDialog}
  <Dialog title="Add repository" onclose={() => (addDialog = false)} width={480}>
    <form class="form" onsubmit={(e) => (e.preventDefault(), add())}>
      <input type="text" placeholder="Repository URL or Shortcode" bind:value={repoUrl} />
      {#if addError}<p class="error-text">{addError}</p>{/if}
    </form>
    {#snippet footer()}
      <button class="btn btn-black" onclick={() => (addDialog = false)}>Cancel</button>
      <button class="btn btn-white" onclick={add} disabled={adding || !repoUrl.trim()}>
        {#if adding}<div class="spinner small"></div>{/if} Add
      </button>
    {/snippet}
  </Dialog>
{/if}

{#if confirmRemove}
  {@const repo = confirmRemove}
  <Dialog title="Delete" onclose={() => (confirmRemove = null)} width={380}>
    <p class="confirm">Remove the repository “{repo.name}”? Installed plugins stay installed.</p>
    {#snippet footer()}
      <button class="btn btn-black" onclick={() => (confirmRemove = null)}>Cancel</button>
      <button class="btn btn-white" onclick={() => remove(repo)}>Delete</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .screen {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .list {
    flex: 1;
    min-height: 0;
    padding: 8px 0;
  }

  .blank {
    max-width: 460px;
    margin: 0 auto;
  }

  .repo-item {
    display: flex;
    align-items: center;
    padding-right: 8px;
  }

  .repo-item:hover {
    background: var(--ripple);
  }

  .repo-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
    text-align: left;
  }

  .entry-icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 10px;
    background: var(--bg-gray);
    color: var(--icon);
    flex-shrink: 0;
  }

  .texts {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .main-text {
    font-size: 16px;
    font-weight: 500;
  }

  .sub-text {
    font-size: 12px;
    color: var(--text-gray);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 12px;
  }

  .storage {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 80px;
    padding: 12px 20px;
    background: var(--bg-gray);
    text-align: left;
  }

  .storage-title {
    font-weight: 700;
  }

  .bar {
    display: flex;
    height: 12px;
    border-radius: 3px;
    overflow: hidden;
    background: var(--bg);
  }

  .seg.downloaded,
  i.downloaded {
    background: var(--text);
  }

  .seg.disabled,
  i.disabled {
    background: var(--fail);
  }

  .seg.not,
  i.not {
    background: var(--text-gray);
    opacity: 0.6;
  }

  .legend {
    display: flex;
    gap: 18px;
    font-size: 12px;
    color: var(--text-gray);
  }

  .legend i {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 2px;
    margin-right: 6px;
  }

  .add {
    bottom: 96px;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 4px 16px 8px;
  }

  .form input {
    height: 44px;
    padding: 0 14px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: var(--bg);
    outline: none;
  }

  .form input:focus {
    border-color: var(--primary);
  }

  .confirm {
    padding: 0 16px 8px;
    color: var(--text-gray);
  }
</style>
