<script lang="ts">
  // Android res/layout/fragment_plugins.xml + repository_item.xml.
  // repo = null shows the locally installed plugins.
  import Icon from "../components/Icon.svelte";
  import { errorMessage, type Repo, type RepoPlugin } from "../lib/api";
  import { lazySrc } from "../lib/images";
  import { ext, installPlugin, loadRepoPlugins, refreshInstalled, uninstallPlugin } from "../lib/extensions.svelte";
  import { router } from "../lib/router.svelte";
  import { showToast } from "../lib/state.svelte";
  import { TV_TYPE_CHIPS, flagFor, typesFor } from "../lib/tvtypes";

  let { repo }: { repo: Repo | null } = $props();

  let query = $state("");
  let searching = $state(false);
  let chips = $state<string[]>([]);
  let busy = $state<Record<string, boolean>>({});
  let failedIcons = $state<Record<string, boolean>>({});

  $effect(() => {
    if (repo) void loadRepoPlugins(repo.url);
    else void refreshInstalled();
  });

  const installed = $derived(new Map(ext.installed.map((p) => [p.id, p])));
  const types = $derived(typesFor(chips));
  const plugins = $derived(
    (repo ? (ext.available[repo.url] ?? []) : []).filter((p) => {
      const q = query.trim().toLowerCase();
      const matchesQuery = !q || p.name.toLowerCase().includes(q) || p.description?.toLowerCase().includes(q);
      const matchesType = !types.size || p.tvTypes?.some((t) => types.has(t));
      return matchesQuery && matchesType;
    }),
  );
  const local = $derived(ext.installed.filter((p) => !query.trim() || p.name.toLowerCase().includes(query.trim().toLowerCase())));

  function size(bytes?: number) {
    if (!bytes) return "";
    return bytes > 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.round(bytes / 1024)} kB`;
  }

  async function run(key: string, task: () => Promise<unknown>, done: string) {
    busy[key] = true;
    try {
      await task();
      showToast(done);
    } catch (e) {
      showToast(errorMessage(e));
    } finally {
      busy[key] = false;
    }
  }

  async function install(p: RepoPlugin) {
    busy[p.internalName] = true;
    try {
      const result = await installPlugin(p);
      showToast(result.error ? `${p.name}: ${result.error}` : "Plugin Loaded");
    } catch (e) {
      showToast(errorMessage(e));
    } finally {
      busy[p.internalName] = false;
    }
  }

  function toggleChip(label: string) {
    chips = chips.includes(label) ? chips.filter((c) => c !== label) : [...chips, label];
  }
</script>

<div class="screen">
  <div class="toolbar">
    <button class="icon-btn" title="Go Back" onclick={() => router.back()}><Icon name="back" size={26} /></button>
    {#if searching}
      <input class="search" type="text" placeholder="Search…" bind:value={query} {@attach (el) => el.focus()} />
      <button class="icon-btn" title="Close" onclick={() => ((searching = false), (query = ""))}><Icon name="close" size={22} /></button>
    {:else}
      <h1>{repo?.name ?? "Installed plugins"}</h1>
      <div class="grow"></div>
      <button class="icon-btn" title="Search" onclick={() => (searching = true)}><Icon name="search" size={24} /></button>
    {/if}
  </div>

  {#if repo}
    <div class="row-scroll chips">
      {#each TV_TYPE_CHIPS as chip (chip.label)}
        <button class="chip" class:checked={chips.includes(chip.label)} onclick={() => toggleChip(chip.label)}>{chip.label}</button>
      {/each}
    </div>
  {/if}

  <div class="page list" data-scroll-root>
    {#if repo}
      {#if ext.loadingRepo[repo.url] && !ext.available[repo.url]}
        <div class="center-fill"><div class="spinner"></div></div>
      {/if}
      {#each plugins as p (p.internalName)}
        {@const have = installed.get(p.internalName)}
        <div class="item">
          <span class="entry-icon">
            {#if p.iconUrl && !failedIcons[p.internalName]}
              <img
                {@attach lazySrc(p.iconUrl.replace("%size%", "64"))}
                alt=""
                decoding="async"
                referrerpolicy="no-referrer"
                onerror={() => (failedIcons[p.internalName] = true)}
              />
            {:else}
              <Icon name="extension" size={22} />
            {/if}
          </span>
          <span class="texts">
            <span class="main-text">
              {p.name}
              {#if p.tvTypes?.includes("NSFW")}<span class="nsfw">18+</span>{/if}
            </span>
            <span class="info">
              {#if p.language}<span>{flagFor(p.language)}</span>{/if}
              <span>v{p.version}</span>
              {#if p.fileSize}<span>{size(p.fileSize)}</span>{/if}
              {#if p.status === 0}<span class="error-text">Down</span>{:else if p.status === 2}<span class="warn">Slow</span>{:else if p.status === 3}<span class="warn">Beta</span>{/if}
              {#if have?.error}<span class="error-text">Failed to load</span>{/if}
            </span>
            {#if p.description}<span class="sub-text">{p.description}</span>{/if}
          </span>
          {#if busy[p.internalName]}
            <div class="spinner small"></div>
          {:else if !have}
            <button class="icon-btn" title="Download" onclick={() => install(p)}><Icon name="download" size={24} /></button>
          {:else}
            {#if (have.version ?? 0) < p.version}
              <button class="icon-btn" title="Update" onclick={() => install(p)}><Icon name="autorenew" size={22} /></button>
            {/if}
            <button
              class="icon-btn"
              title="Delete"
              onclick={() => run(p.internalName, () => uninstallPlugin(p.internalName), "Plugin Deleted")}
            >
              <Icon name="delete" size={22} />
            </button>
          {/if}
        </div>
      {:else}
        {#if !ext.loadingRepo[repo.url]}<p class="center-fill">No Data</p>{/if}
      {/each}
    {:else}
      {#each local as p (p.id)}
        <div class="item">
          <span class="entry-icon"><Icon name="extension" size={22} /></span>
          <span class="texts">
            <span class="main-text">{p.name}</span>
            <span class="info">{#if p.version != null}<span>v{p.version}</span>{/if}</span>
            {#if p.error}
              <span class="sub-text error-text">{p.error}</span>
            {:else}
              <span class="sub-text">{p.providers.join(", ") || "No providers registered"}</span>
            {/if}
          </span>
          {#if busy[p.id]}
            <div class="spinner small"></div>
          {:else}
            <button class="icon-btn" title="Delete" onclick={() => run(p.id, () => uninstallPlugin(p.id), "Plugin Deleted")}>
              <Icon name="delete" size={22} />
            </button>
          {/if}
        </div>
      {:else}
        <p class="center-fill">No plugins installed.</p>
      {/each}
    {/if}
  </div>
</div>

<style>
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .grow {
    flex: 1;
  }

  .search {
    flex: 1;
    height: 40px;
    padding: 0 16px;
    border-radius: 20px;
    border: none;
    outline: none;
    background: var(--bg);
    font-size: 15px;
  }

  .chips {
    padding-top: 10px;
    padding-bottom: 10px;
    background: var(--bg-gray);
    flex-shrink: 0;
  }

  .list {
    flex: 1;
    min-height: 0;
    padding: 6px 0 30px;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 12px 10px 16px;
  }

  .item:hover {
    background: var(--ripple);
  }

  .entry-icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 10px;
    overflow: hidden;
    background: var(--bg-gray);
    color: var(--icon);
    flex-shrink: 0;
  }

  .entry-icon img {
    width: 32px;
    height: 32px;
    object-fit: contain;
  }

  .texts {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .main-text {
    font-size: 16px;
    font-weight: 500;
  }

  .nsfw {
    margin-left: 6px;
    padding: 0 6px;
    border-radius: 4px;
    background: var(--sub-bg);
    color: #fff;
    font-size: 11px;
    font-weight: 700;
  }

  .info {
    display: flex;
    gap: 10px;
    font-size: 12px;
    color: var(--text-gray);
  }

  .warn {
    color: var(--warn);
  }

  .sub-text {
    font-size: 12px;
    color: var(--text-gray);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
