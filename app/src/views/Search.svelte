<script lang="ts" module>
  import type { SearchGroup } from "../lib/api";

  // Kept at module level so results survive opening a title and coming back.
  const search = $state({ query: "", ran: "", groups: [] as SearchGroup[], chips: [] as string[] });
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import Dialog from "../components/Dialog.svelte";
  import Icon from "../components/Icon.svelte";
  import PosterCard from "../components/PosterCard.svelte";
  import ScrollRow from "../components/ScrollRow.svelte";
  import { engine, errorMessage, type SearchItem } from "../lib/api";
  import { addHistory, clearHistory, library, removeHistory } from "../lib/library.svelte";
  import { router } from "../lib/router.svelte";
  import { cardWidth, engineState, settings } from "../lib/state.svelte";
  import { TV_TYPE_CHIPS, flagFor, typesFor } from "../lib/tvtypes";
  import { uniqueBy } from "../lib/unique";

  let { initialQuery }: { initialQuery?: string } = $props();

  let input: HTMLInputElement;
  let loading = $state(false);
  let error = $state("");
  let filterDialog = $state(false);
  let expanded = $state<SearchGroup | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let requestId = 0;

  onMount(() => {
    if (initialQuery) {
      search.query = initialQuery;
      run(true);
    }
    input.focus();
  });

  const types = $derived(typesFor(search.chips));
  const searchable = $derived(
    engineState.providers.filter(
      (p) => !settings.searchExcluded.includes(p.name) && (!types.size || p.supportedTypes.some((t) => types.has(t))),
    ),
  );

  async function run(force = false) {
    clearTimeout(timer);
    const query = search.query.trim();
    if (!query) {
      search.groups = [];
      search.ran = "";
      return;
    }
    if (query === search.ran && !force) return;
    const id = ++requestId;
    loading = true;
    error = "";
    try {
      // One request per provider so rows appear as each provider answers.
      search.ran = query;
      search.groups = [];
      await Promise.all(
        searchable.map(async (p) => {
          const [group] = await engine.search(query, p.name).catch((e) => [
            { provider: p.name, items: [], error: errorMessage(e) } as SearchGroup,
          ]);
          if (id === requestId && group) search.groups = [...search.groups, group];
        }),
      );
      if (id === requestId) addHistory(query);
    } catch (e) {
      if (id === requestId) error = errorMessage(e);
    } finally {
      if (id === requestId) loading = false;
    }
  }

  function onInput() {
    clearTimeout(timer);
    timer = setTimeout(() => run(), 600);
  }

  function toggleChip(label: string) {
    search.chips = search.chips.includes(label) ? search.chips.filter((c) => c !== label) : [...search.chips, label];
    if (search.ran) run(true);
  }

  function toggleProvider(name: string) {
    settings.searchExcluded = settings.searchExcluded.includes(name)
      ? settings.searchExcluded.filter((n) => n !== name)
      : [...settings.searchExcluded, name];
  }

  const groups = $derived(
    uniqueBy(search.groups, (g) => g.provider)
      .map((g) => ({
        ...g,
        items: uniqueBy(types.size ? g.items.filter((i) => !i.type || types.has(i.type)) : g.items, (i) => i.url),
      }))
      .filter((g) => g.items.length)
      .sort((a, b) => a.provider.localeCompare(b.provider)),
  );

  function open(item: SearchItem) {
    router.push({ name: "details", provider: item.apiName, url: item.url, title: item.name });
  }
</script>

<div class="search-root">
  <div class="top">
    <div class="bar">
      <div class="field">
        <Icon name="search" size={22} />
        <input
          bind:this={input}
          type="text"
          placeholder="Search…"
          bind:value={search.query}
          oninput={onInput}
          onkeydown={(e) => e.key === "Enter" && run(true)}
        />
        {#if loading}
          <div class="spinner small"></div>
        {:else if search.query}
          <button class="clear" title="Clear" onclick={() => ((search.query = ""), run(), input.focus())}>
            <Icon name="close" size={20} />
          </button>
        {/if}
      </div>
      <button class="icon-btn" title="Filter providers" onclick={() => (filterDialog = true)}>
        <Icon name="tune" size={24} />
      </button>
    </div>
    <div class="row-scroll chips">
      {#each TV_TYPE_CHIPS as chip (chip.label)}
        <button class="chip" class:checked={search.chips.includes(chip.label)} onclick={() => toggleChip(chip.label)}>
          {chip.label}
        </button>
      {/each}
    </div>
  </div>

  <div class="results page" data-scroll-root>
    {#if error}
      <p class="center-fill error-text">{error}</p>
    {:else if !search.ran}
      {#if library.history.length}
        <ul class="history">
          {#each uniqueBy(library.history, (h) => h.query) as h (h.query)}
            <li>
              <button class="history-item" onclick={() => ((search.query = h.query), run(true))}>
                <Icon name="history" size={22} />
                <span>{h.query}</span>
              </button>
              <button class="icon-btn" title="Remove" onclick={() => removeHistory(h.query)}><Icon name="close" size={20} /></button>
            </li>
          {/each}
        </ul>
        <button class="clear-history" onclick={clearHistory}>Clear history</button>
      {:else if !engineState.providers.length && engineState.status.state === "ready"}
        <p class="center-fill">Install a plugin from Settings → Extensions to start searching.</p>
      {/if}
    {:else}
      {#each groups as group (group.provider)}
        <button class="watch-header link" onclick={() => (expanded = group)}>
          {group.provider} <span class="badge">{group.items.length}</span> <Icon name="forward" size={20} />
        </button>
        <ScrollRow>
          {#each group.items as item (item.url)}
            <PosterCard {item} width={cardWidth()} onselect={open} />
          {/each}
        </ScrollRow>
      {:else}
        {#if !loading}
          <div class="center-fill">
            <p>No Data</p>
            {#each search.groups.filter((g) => g.error) as g (g.provider)}
              <p class="gray small">{g.provider}: {g.error}</p>
            {/each}
          </div>
        {/if}
      {/each}
      <div style="height: 24px"></div>
    {/if}
  </div>
</div>

{#if expanded}
  {@const group = expanded}
  <Dialog title={`${group.provider} · “${search.ran}”`} onclose={() => (expanded = null)} width={980}>
    <div class="expanded-grid">
      {#each group.items as item (item.url)}
        <PosterCard {item} width={126} onselect={(i) => ((expanded = null), open(i))} />
      {/each}
    </div>
  </Dialog>
{/if}

{#if filterDialog}
  <Dialog title="Search using providers" onclose={() => (filterDialog = false)} width={380}>
    {#each engineState.providers as p (p.name)}
      <label class="check-row">
        <input type="checkbox" checked={!settings.searchExcluded.includes(p.name)} onchange={() => toggleProvider(p.name)} />
        <span class="grow">{p.name}</span>
        <span>{flagFor(p.lang)}</span>
      </label>
    {:else}
      <p class="gray pad">No providers installed.</p>
    {/each}
  </Dialog>
{/if}

<style>
  .search-root {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-gray);
  }

  .top {
    flex-shrink: 0;
    background: var(--bg-gray);
    padding-top: 10px;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px 0 16px;
  }

  .field {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    padding: 0 12px;
    border-radius: 20px;
    background: var(--bg);
    color: var(--text-gray);
  }

  .field input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: none;
    outline: none;
    background: none;
    color: var(--text);
    font-size: 15px;
  }

  .clear {
    color: var(--text-gray);
    display: grid;
  }

  .chips {
    padding-top: 10px;
    padding-bottom: 10px;
  }

  .results {
    flex: 1;
    min-height: 0;
    background: var(--bg);
  }

  .history {
    list-style: none;
    margin: 0;
    padding: 6px 0;
  }

  .history li {
    display: flex;
    align-items: center;
    padding-right: 8px;
  }

  .history-item {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 16px;
    height: 48px;
    padding: 0 18px;
    color: var(--text);
    font-size: 15px;
    text-align: left;
  }

  .history-item :global(svg) {
    color: var(--text-gray);
  }

  .history-item:hover {
    background: var(--ripple);
  }

  .clear-history {
    display: block;
    margin: 6px auto 20px;
    color: var(--primary);
    font-weight: 700;
    padding: 8px 14px;
  }

  .check-row {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 46px;
    padding: 0 16px;
    border-radius: 8px;
    cursor: pointer;
  }

  .check-row:hover {
    background: var(--ripple);
  }

  .check-row input {
    width: 18px;
    height: 18px;
    accent-color: var(--primary);
  }

  .grow {
    flex: 1;
  }

  .pad {
    padding: 12px 16px;
  }

  .small {
    font-size: 12px;
    max-width: 640px;
  }

  .link {
    text-align: left;
  }

  .link:hover {
    color: var(--primary);
  }

  .expanded-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 10px 6px;
    padding: 4px 12px 12px;
    justify-items: center;
  }
</style>
