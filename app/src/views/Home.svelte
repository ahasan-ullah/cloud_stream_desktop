<script lang="ts" module>
  import type { HomeSection } from "../lib/api";

  type HomeState = { sections: HomeSection[]; next: number; total: number; errors: string[] };

  // Survives navigating to details and back.
  const homeCache = new Map<string, HomeState>();
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import Dialog from "../components/Dialog.svelte";
  import Icon from "../components/Icon.svelte";
  import PosterCard from "../components/PosterCard.svelte";
  import ScrollRow from "../components/ScrollRow.svelte";
  import { engine, errorMessage, openUrl, type SearchItem } from "../lib/api";
  import { thumb } from "../lib/images";
  import {
    WATCH_STATUSES,
    bookmarkOf,
    bookmarksWith,
    continueWatching,
    library,
    removeProgress,
    setBookmark,
    statusLabel,
    titleKey,
    type WatchStatus,
  } from "../lib/library.svelte";
  import { router } from "../lib/router.svelte";
  import { cardWidth, engineState, settings } from "../lib/state.svelte";
  import { uniqueBy } from "../lib/unique";
  import { flagFor, qualityLabel, typeLabel } from "../lib/tvtypes";

  /** Categories scraped per request; more are fetched as the user scrolls down. */
  const BATCH = 4;
  /** Cards per row on Home; "See all" opens the full list. */
  const ROW_LIMIT = 12;
  const HERO_LIMIT = 8;

  const homeProviders = $derived(engineState.providers.filter((p) => p.hasMainPage));
  const provider = $derived(
    homeProviders.some((p) => p.name === settings.homeProvider) ? settings.homeProvider : (homeProviders[0]?.name ?? ""),
  );
  const providerInfo = $derived(homeProviders.find((p) => p.name === provider));

  let home = $state<HomeState | undefined>();
  let loading = $state(false);
  let error = $state("");
  let requestId = 0;

  async function loadMore(name: string) {
    if (loading || (home && home.next >= home.total)) return;
    const id = ++requestId;
    const from = home?.next ?? 0;
    loading = true;
    error = "";
    try {
      const res = await engine.home(name, from, BATCH);
      if (id !== requestId) return;
      const prev = home ?? { sections: [], next: 0, total: res.total, errors: [] };
      home = {
        sections: [...prev.sections, ...res.sections.map((s) => ({ ...s, items: uniqueBy(s.items, (i) => i.url) }))],
        next: from + BATCH,
        total: res.total,
        errors: [...prev.errors, ...res.errors],
      };
      homeCache.set(name, home);
    } catch (e) {
      if (id === requestId) error = errorMessage(e);
    } finally {
      if (id === requestId) loading = false;
    }
  }

  function reload() {
    homeCache.delete(provider);
    home = undefined;
    requestId++;
    loading = false;
    void loadMore(provider);
  }

  // Track only `provider`; loadMore() reads and writes `home`, which must not re-trigger this.
  $effect(() => {
    const name = provider;
    untrack(() => {
      requestId++;
      loading = false;
      error = "";
      home = homeCache.get(name);
      if (name && !home) void loadMore(name);
    });
  });

  /** Loads the next categories when the end of the page comes near. */
  function sentinel(node: HTMLElement) {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0].isIntersecting && provider) void loadMore(provider);
      },
      { root: node.closest("[data-scroll-root]"), rootMargin: "500px" },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }

  const done = $derived(!!home && home.next >= home.total);

  // ---- hero ("home_preview_viewpager")
  const heroItems = $derived(
    (home?.sections.find((s) => !s.horizontal) ?? home?.sections[0])?.items.filter((i) => i.posterUrl).slice(0, HERO_LIMIT) ??
      [],
  );
  let heroIndex = $state(0);
  const hero = $derived(heroItems[Math.min(heroIndex, heroItems.length - 1)]);

  function step(delta: number) {
    if (!heroItems.length) return;
    heroIndex = (heroIndex + delta + heroItems.length) % heroItems.length;
  }

  let dragX: number | null = null;
  function onPointerDown(e: PointerEvent) {
    dragX = e.clientX;
  }
  function onPointerUp(e: PointerEvent) {
    if (dragX != null && Math.abs(e.clientX - dragX) > 60) step(e.clientX < dragX ? 1 : -1);
    dragX = null;
  }

  // ---- bookmarks & continue watching
  const watching = $derived.by(() => {
    void library.progress;
    return continueWatching();
  });
  const bookmarkStatuses = $derived.by(() => {
    void library.bookmarks;
    return WATCH_STATUSES.filter((s) => bookmarksWith(s.id).length);
  });
  let bookmarkTab = $state<WatchStatus | undefined>();
  const activeTab = $derived(bookmarkStatuses.some((s) => s.id === bookmarkTab) ? bookmarkTab : bookmarkStatuses[0]?.id);
  const bookmarked = $derived.by(() => {
    void library.bookmarks;
    return activeTab ? bookmarksWith(activeTab).slice(0, ROW_LIMIT) : [];
  });

  const heroStatus = $derived.by(() => {
    void library.bookmarks;
    return hero ? bookmarkOf(hero.apiName, hero.url) : undefined;
  });

  // ---- dialogs
  let statusDialog = $state(false);
  let sourceDialog = $state(false);
  let sourceFilter = $state("");
  let expanded = $state<HomeSection | null>(null);

  const filteredProviders = $derived(
    homeProviders.filter((p) => p.name.toLowerCase().includes(sourceFilter.trim().toLowerCase())),
  );

  function open(item: SearchItem, autoplay = false) {
    router.push({ name: "details", provider: item.apiName, url: item.url, title: item.name, autoplay });
  }

  /** Android's home "Random" button. */
  function randomTitle() {
    const pool = home?.sections.flatMap((s) => s.items) ?? [];
    if (pool.length) open(pool[Math.floor(Math.random() * pool.length)]);
  }
</script>

<div class="page home" data-scroll-root>
  {#if engineState.status.state === "ready" && !homeProviders.length}
    <div class="center-fill blank">
      <Icon name="extension" size={40} />
      <p>CloudStream has no sites installed by default. You need to install the sites from repositories.</p>
      <button class="btn btn-white" onclick={() => router.push({ name: "extensions" })}>Open Extensions</button>
    </div>
  {:else}
    <!-- hero -->
    {#if !home && !error}
      <div class="hero shimmer-hero shimmer"></div>
    {:else if hero}
      <section class="hero" aria-label="Featured" onpointerdown={onPointerDown} onpointerup={onPointerUp}>
        <!-- Backdrop: a tiny thumbnail blurred to fill any width (a stretched poster looks broken on wide windows). -->
        {#key hero.url}
          <img
            class="hero-bg"
            src={thumb(hero.posterUrl, 120)}
            alt=""
            decoding="async"
            referrerpolicy="no-referrer"
            draggable="false"
          />
        {/key}
        <div class="hero-scrim"></div>

        <div class="hero-top">
          <button class="search-pill" onclick={() => router.root("search")}>
            <Icon name="search" size={20} /> <span>Search…</span>
          </button>
        </div>

        <div class="hero-body">
          {#key hero.url}
            <img
              class="hero-poster"
              src={thumb(hero.posterUrl, 200)}
              alt=""
              decoding="async"
              referrerpolicy="no-referrer"
              draggable="false"
            />
          {/key}
          <div class="hero-info">
            <h2>{hero.name}</h2>
            <div class="hero-meta">
              {#if hero.quality}<span class="badge">{qualityLabel(hero.quality)}</span>{/if}
              {#if hero.type}<span>{typeLabel(hero.type)}</span>{/if}
              <span class="gray">{hero.apiName}</span>
            </div>
            <div class="hero-actions">
              <button class="text-action" onclick={() => (statusDialog = true)}>
                <Icon name={heroStatus ? "bookmark" : "bookmarkBorder"} size={24} />
                <span>{statusLabel(heroStatus)}</span>
              </button>
              <button class="btn btn-white" onclick={() => open(hero, true)}><Icon name="play" size={22} /> Play</button>
              <button class="text-action" onclick={() => open(hero)}>
                <Icon name="info" size={24} />
                <span>Info</span>
              </button>
            </div>
          </div>
        </div>

        {#if heroItems.length > 1}
          <div class="dots">
            {#each heroItems as _, i}
              <button class="dot" class:active={i === heroIndex} aria-label="Featured {i + 1}" onclick={() => (heroIndex = i)}></button>
            {/each}
          </div>
          <button class="pager prev" onclick={() => step(-1)} title="Previous"><Icon name="back" size={22} /></button>
          <button class="pager next" onclick={() => step(1)} title="Next"><Icon name="forward" size={22} /></button>
        {/if}
      </section>
    {:else}
      <div class="hero-spacer">
        <button class="search-pill" onclick={() => router.root("search")}>
          <Icon name="search" size={20} /> <span>Search…</span>
        </button>
      </div>
    {/if}

    {#if watching.length}
      <h3 class="watch-header">Continue Watching</h3>
      <ScrollRow>
        {#each watching.slice(0, ROW_LIMIT) as w (titleKey(w.item.apiName, w.item.url))}
          <PosterCard
            item={w.item}
            width={cardWidth()}
            progress={w.position / w.duration}
            episodeText={w.season != null || w.episode != null ? w.label.split(" ")[0] : undefined}
            onselect={(i) => open(i, true)}
            onremove={() => removeProgress(w.item.apiName, w.item.url)}
          />
        {/each}
      </ScrollRow>
    {/if}

    {#if bookmarkStatuses.length}
      <div class="chips">
        {#each bookmarkStatuses as s (s.id)}
          <button class="chip" class:checked={activeTab === s.id} onclick={() => (bookmarkTab = s.id)}>{s.label}</button>
        {/each}
      </div>
      <ScrollRow>
        {#each bookmarked as b (titleKey(b.item.apiName, b.item.url))}
          <PosterCard item={b.item} width={cardWidth()} onselect={(i) => open(i)} />
        {/each}
      </ScrollRow>
    {/if}

    {#if home}
      {#each home.sections as section, si (si + section.name)}
        {@const width = section.horizontal ? cardWidth() * 2 : cardWidth()}
        <section class="category">
          <button class="watch-header link" onclick={() => (expanded = section)}>
            {section.name} <Icon name="forward" size={20} />
          </button>
          <ScrollRow>
            {#each section.items.slice(0, ROW_LIMIT) as item (item.url)}
              <PosterCard {item} horizontal={section.horizontal} {width} onselect={(i) => open(i)} />
            {/each}
            {#if section.items.length > ROW_LIMIT}
              <button
                class="see-all"
                style:width="{width}px"
                style:height="{section.horizontal ? Math.round((width * 9) / 16) : Math.round((width * 180) / 114)}px"
                onclick={() => (expanded = section)}
              >
                <Icon name="forward" size={26} />
                <span>See all</span>
                <span class="gray small">{section.items.length}</span>
              </button>
            {/if}
          </ScrollRow>
        </section>
      {/each}
    {/if}

    {#if loading}
      {#each home ? [0] : [0, 1, 2] as _}
        <div class="shimmer shimmer-title"></div>
        <div class="shimmer-row">
          {#each Array(8) as __}<div class="shimmer shimmer-card"></div>{/each}
        </div>
      {/each}
    {:else if error}
      <div class="center-fill">
        <p class="error-text">{error}</p>
        <button class="btn btn-white" onclick={reload}><Icon name="autorenew" size={20} /> Retry connection…</button>
        {#if providerInfo}
          <button class="btn btn-black" onclick={() => openUrl(providerInfo.mainUrl)}><Icon name="public" size={20} /> Open In Browser</button>
        {/if}
      </div>
    {:else if done && home && !home.sections.length}
      <div class="center-fill">
        <p class={home.errors.length ? "error-text" : ""}>{home.errors[0] ?? `${provider} returned no data`}</p>
        <button class="btn btn-white" onclick={reload}><Icon name="autorenew" size={20} /> Retry connection…</button>
        {#if providerInfo}
          <button class="btn btn-black" onclick={() => openUrl(providerInfo.mainUrl)}><Icon name="public" size={20} /> Open In Browser</button>
        {/if}
      </div>
    {:else if home && !done}
      <!-- Re-created after every batch so the observer reports again if it is still on screen. -->
      {#key home.next}<div class="sentinel" {@attach sentinel}></div>{/key}
    {/if}
    <div style="height: 90px"></div>
  {/if}
</div>

{#if home?.sections.length}
  <button class="fab random" onclick={randomTitle} title="Open a random title">
    <Icon name="play" size={22} /> Random
  </button>
{/if}

{#if homeProviders.length}
  <button class="fab" onclick={() => (sourceDialog = true)} title="Change Provider">
    <Icon name="filter" size={22} />
    {provider || "Source"}
  </button>
{/if}

{#if statusDialog && hero}
  <Dialog title="Set watch status" onclose={() => (statusDialog = false)} width={340}>
    {#each [{ id: undefined, label: "None" }, ...WATCH_STATUSES] as s (s.label)}
      <button
        class="list-item"
        onclick={() => {
          setBookmark(hero, s.id as WatchStatus | undefined);
          statusDialog = false;
        }}
      >
        <span class="radio" class:on={heroStatus === s.id}></span>{s.label}
      </button>
    {/each}
  </Dialog>
{/if}

{#if sourceDialog}
  <Dialog title="Source" onclose={() => (sourceDialog = false)} width={380}>
    <input class="dialog-search" type="search" placeholder="Search…" bind:value={sourceFilter} />
    {#each filteredProviders as p (p.name)}
      <button
        class="list-item"
        onclick={() => {
          settings.homeProvider = p.name;
          sourceDialog = false;
        }}
      >
        <span class="radio" class:on={p.name === provider}></span>
        <span class="grow">{p.name}</span>
        <span class="flag">{flagFor(p.lang)}</span>
      </button>
    {/each}
  </Dialog>
{/if}

{#if expanded}
  <Dialog title={expanded.name} onclose={() => (expanded = null)} width={980}>
    <div class="expanded-grid" class:wide={expanded.horizontal}>
      {#each expanded.items as item (item.url)}
        <PosterCard
          {item}
          horizontal={expanded.horizontal}
          width={expanded.horizontal ? 230 : 126}
          onselect={(i) => {
            expanded = null;
            open(i);
          }}
        />
      {/each}
    </div>
  </Dialog>
{/if}

<style>
  .home {
    position: relative;
  }

  .random {
    left: 16px;
    right: auto;
  }

  .hero {
    position: relative;
    height: clamp(320px, 46vh, 460px);
    overflow: hidden;
    background: var(--bg-gray);
    touch-action: pan-y;
  }

  .shimmer-hero {
    border-radius: 0;
  }

  .hero-bg {
    position: absolute;
    inset: -60px;
    width: calc(100% + 120px);
    height: calc(100% + 120px);
    object-fit: cover;
    filter: blur(40px) saturate(1.3) brightness(0.6);
    animation: fade 0.35s ease-out;
    user-select: none;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .hero-scrim {
    position: absolute;
    inset: 0;
    background:
      linear-gradient(to right, rgba(17, 17, 17, 0.85), rgba(17, 17, 17, 0.35) 55%, rgba(17, 17, 17, 0.15)),
      linear-gradient(to bottom, transparent 55%, var(--bg));
  }

  /* Sits below the search bar, so the poster scales with the banner height instead of overlapping it. */
  .hero-body {
    position: absolute;
    top: 76px;
    left: 0;
    right: 0;
    bottom: 34px;
    display: flex;
    align-items: flex-end;
    gap: 32px;
    padding: 0 64px;
  }

  .hero-poster {
    height: 100%;
    max-height: 300px;
    aspect-ratio: 2 / 3;
    flex-shrink: 0;
    object-fit: cover;
    border-radius: var(--radius);
    background: var(--bg-gray);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.6);
    animation: fade 0.35s ease-out;
    user-select: none;
  }

  .hero-info {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
    max-width: 680px;
    padding-bottom: 4px;
  }

  .hero-info h2 {
    font-size: clamp(22px, 2.4vw, 36px);
    font-weight: 700;
    line-height: 1.15;
    color: #fff;
    text-shadow: 0 2px 12px rgba(0, 0, 0, 0.6);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .hero-meta {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 14px;
  }

  .hero-actions {
    display: flex;
    align-items: center;
    gap: 28px;
  }

  .dots {
    position: absolute;
    right: 64px;
    bottom: 40px;
    display: flex;
    gap: 6px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.35);
    transition: width 0.2s, background 0.2s;
  }

  .dot.active {
    width: 20px;
    background: var(--text);
  }

  /* Narrow windows: Android's centered layout without the side poster. */
  @media (max-width: 760px) {
    .hero-poster {
      display: none;
    }

    .hero-body {
      justify-content: center;
      padding: 0 24px;
      text-align: center;
    }

    .hero-info {
      align-items: center;
    }

    .dots {
      right: 50%;
      bottom: 12px;
      transform: translateX(50%);
    }
  }

  .hero-top {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 50px;
    display: flex;
    align-items: center;
    padding: 10px 16px 0;
  }

  .hero-spacer {
    padding: 10px 16px;
  }

  .search-pill {
    display: flex;
    align-items: center;
    gap: 10px;
    width: min(420px, 100%);
    height: 40px;
    padding: 0 14px;
    border-radius: 20px;
    background: rgba(17, 17, 17, 0.75);
    color: var(--text-gray);
    font-size: 15px;
  }

  .search-pill:hover {
    color: var(--text);
  }

  .text-action {
    width: 70px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    color: #fff;
    font-size: 12px;
  }

  .text-action span {
    max-width: 90px;
    white-space: nowrap;
  }

  .hero-actions .btn {
    min-width: 120px;
    font-size: 16px;
  }

  .pager {
    position: absolute;
    top: 50%;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.45);
    color: #fff;
    opacity: 0;
    transition: opacity 0.2s;
  }

  .hero:hover .pager {
    opacity: 1;
  }

  .prev {
    left: 14px;
  }

  .next {
    right: 14px;
  }

  /* Off-screen categories are not laid out or painted, so their posters are never rasterized. */
  .category {
    content-visibility: auto;
    contain-intrinsic-size: auto 250px;
  }

  .watch-header.link {
    text-align: left;
  }

  .watch-header.link:hover {
    color: var(--primary);
  }

  .chips {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 14px 16px 8px;
    scrollbar-width: none;
  }

  .see-all {
    flex-shrink: 0;
    margin: 2px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border-radius: var(--radius);
    background: var(--bg-gray);
    color: var(--text);
    font-weight: 700;
  }

  .see-all:hover {
    background: #34353a;
  }

  .small {
    font-size: 12px;
  }

  .shimmer-title {
    width: 160px;
    height: 18px;
    margin: 22px 16px 10px;
  }

  .shimmer-row {
    display: flex;
    gap: 8px;
    padding: 2px 16px;
    overflow: hidden;
  }

  .shimmer-card {
    flex-shrink: 0;
    width: 114px;
    height: 180px;
    margin: 2px;
  }

  .sentinel {
    height: 1px;
  }

  .blank {
    height: 100%;
    max-width: 480px;
    margin: 0 auto;
  }

  .grow {
    flex: 1;
  }

  .flag {
    font-size: 18px;
  }

  .dialog-search {
    width: calc(100% - 16px);
    margin: 0 8px 8px;
    height: 40px;
    padding: 0 16px;
    border-radius: 20px;
    border: none;
    outline: none;
    background: var(--bg);
  }

  .expanded-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 10px 6px;
    padding: 4px 12px 12px;
    justify-items: center;
  }

  .expanded-grid.wide {
    grid-template-columns: repeat(auto-fill, minmax(234px, 1fr));
  }
</style>
