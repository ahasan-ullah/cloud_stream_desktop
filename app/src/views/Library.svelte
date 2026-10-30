<script lang="ts" module>
  import type { WatchStatus } from "../lib/library.svelte";

  const view = $state({
    tab: "watching" as WatchStatus | "history",
    filter: "",
    sort: "updated" as "updated" | "az" | "za",
  });
</script>

<script lang="ts">
  // Android res/layout/fragment_library.xml (local library only).
  import Dialog from "../components/Dialog.svelte";
  import Icon from "../components/Icon.svelte";
  import PosterCard from "../components/PosterCard.svelte";
  import type { SearchItem } from "../lib/api";
  import { lazySrc, thumb } from "../lib/images";
  import { WATCH_STATUSES, bookmarksWith, clearWatched, library, titleKey } from "../lib/library.svelte";
  import { router } from "../lib/router.svelte";

  const SORTS = [
    { id: "updated", label: "Recently updated" },
    { id: "az", label: "Alphabetical (A-Z)" },
    { id: "za", label: "Alphabetical (Z-A)" },
  ] as const;

  let sortDialog = $state(false);

  const counts = $derived.by(() => {
    void library.bookmarks;
    return Object.fromEntries(WATCH_STATUSES.map((s) => [s.id, bookmarksWith(s.id).length]));
  });

  const history = $derived.by(() => {
    const q = view.filter.trim().toLowerCase();
    return library.watched.filter((w) => !q || w.item.name.toLowerCase().includes(q) || w.label.toLowerCase().includes(q));
  });

  const items = $derived.by(() => {
    void library.bookmarks;
    if (view.tab === "history") return [];
    const q = view.filter.trim().toLowerCase();
    const list = bookmarksWith(view.tab).filter((b) => !q || b.item.name.toLowerCase().includes(q));
    if (view.sort === "az") list.sort((a, b) => a.item.name.localeCompare(b.item.name));
    if (view.sort === "za") list.sort((a, b) => b.item.name.localeCompare(a.item.name));
    return list;
  });

  function open(item: SearchItem) {
    router.push({ name: "details", provider: item.apiName, url: item.url, title: item.name });
  }

  /** Android's library "Random" button. */
  function random() {
    const pool = view.tab === "history" ? history.map((h) => h.item) : items.map((b) => b.item);
    if (pool.length) open(pool[Math.floor(Math.random() * pool.length)]);
  }

  function ago(time: number) {
    const minutes = Math.round((Date.now() - time) / 60000);
    if (minutes < 1) return "just now";
    if (minutes < 60) return `${minutes} min ago`;
    const hours = Math.round(minutes / 60);
    if (hours < 24) return `${hours} h ago`;
    const days = Math.round(hours / 24);
    return days < 30 ? `${days} d ago` : new Date(time).toLocaleDateString();
  }
</script>

<div class="library">
  <div class="top">
    <div class="bar">
      <div class="field">
        <Icon name="search" size={22} />
        <input type="text" placeholder="Search…" bind:value={view.filter} />
      </div>
      <button class="icon-btn" title="Sort" onclick={() => (sortDialog = true)}><Icon name="sort" size={24} /></button>
    </div>
    <div class="tabs">
      {#each WATCH_STATUSES as s (s.id)}
        <button class="tab" class:active={view.tab === s.id} onclick={() => (view.tab = s.id)}>
          {s.label}
          {#if counts[s.id]}<span class="count">{counts[s.id]}</span>{/if}
        </button>
      {/each}
      <button class="tab" class:active={view.tab === "history"} onclick={() => (view.tab = "history")}>
        History
        {#if library.watched.length}<span class="count">{library.watched.length}</span>{/if}
      </button>
    </div>
  </div>

  <div class="page body" data-scroll-root>
    {#if view.tab === "history"}
      {#if history.length}
        <div class="history-head">
          <button class="clear" onclick={clearWatched}>Clear history</button>
        </div>
        <ul class="history">
          {#each history as h (h.time + h.data)}
            <li>
              <button class="history-row" onclick={() => open(h.item)}>
                <span class="thumb">
                  {#if h.item.posterUrl}<img {@attach lazySrc(thumb(h.item.posterUrl, 44))} alt="" referrerpolicy="no-referrer" />{/if}
                </span>
                <span class="texts">
                  <span class="name">{h.item.name}</span>
                  {#if h.label !== h.item.name}<span class="gray small">{h.label}</span>{/if}
                </span>
                <span class="gray small">{ago(h.time)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <div class="center-fill empty"><p class="big">Nothing watched yet.</p></div>
      {/if}
    {:else if items.length}
      <div class="grid">
        {#each items as b (titleKey(b.item.apiName, b.item.url))}
          <PosterCard item={b.item} width={126} onselect={open} />
        {/each}
      </div>
    {:else}
      <div class="center-fill empty">
        <p class="big">{Object.keys(library.bookmarks).length ? "This list is empty. Try switching to another one." : "Your library is empty :("}</p>
        {#if !Object.keys(library.bookmarks).length}
          <p>Add shows to your local library from their page with the bookmark button.</p>
        {/if}
      </div>
    {/if}
  </div>

  {#if (view.tab === "history" ? history.length : items.length) > 1}
    <button class="fab" onclick={random}><Icon name="play" size={22} /> Random</button>
  {/if}
</div>

{#if sortDialog}
  <Dialog title="Sort by" onclose={() => (sortDialog = false)} width={320}>
    {#each SORTS as s (s.id)}
      <button class="list-item" onclick={() => ((view.sort = s.id), (sortDialog = false))}>
        <span class="radio" class:on={view.sort === s.id}></span>{s.label}
      </button>
    {/each}
  </Dialog>
{/if}

<style>
  .library {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .history-head {
    display: flex;
    justify-content: flex-end;
    padding: 10px 16px 0;
  }

  .clear {
    color: var(--primary);
    font-weight: 700;
    padding: 6px 10px;
    border-radius: 6px;
  }

  .clear:hover {
    background: var(--ripple);
  }

  .history {
    list-style: none;
    margin: 0;
    padding: 4px 0 90px;
  }

  .history-row {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    padding: 8px 20px;
    text-align: left;
  }

  .history-row:hover {
    background: var(--ripple);
  }

  .thumb {
    width: 44px;
    height: 66px;
    flex-shrink: 0;
    border-radius: 6px;
    overflow: hidden;
    background: var(--bg-gray);
  }

  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .thumb img:not([src]) {
    visibility: hidden;
  }

  .texts {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .name {
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .small {
    font-size: 12px;
  }

  .top {
    background: var(--bg-gray);
    padding-top: 10px;
    flex-shrink: 0;
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
    height: 100%;
    border: none;
    outline: none;
    background: none;
    color: var(--text);
    font-size: 15px;
  }

  .tabs {
    display: flex;
    overflow-x: auto;
    margin-top: 6px;
    scrollbar-width: none;
  }

  .tab {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 44px;
    padding: 0 18px;
    color: var(--text-gray);
    font-weight: 700;
    white-space: nowrap;
  }

  .tab.active {
    color: var(--text);
  }

  .tab.active::after {
    content: "";
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 0;
    height: 3px;
    border-radius: 3px 3px 0 0;
    background: var(--primary);
  }

  .count {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 8px;
    background: var(--bg);
    color: var(--text-gray);
  }

  .body {
    flex: 1;
    min-height: 0;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(132px, 1fr));
    gap: 12px 6px;
    padding: 14px 12px 30px;
    justify-items: center;
  }

  .empty {
    height: 100%;
  }

  .big {
    font-size: 18px;
    color: var(--text);
  }
</style>
