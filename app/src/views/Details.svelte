<script lang="ts">
  // Android res/layout/fragment_result.xml
  import Dialog from "../components/Dialog.svelte";
  import DownloadDialog from "../components/DownloadDialog.svelte";
  import Icon from "../components/Icon.svelte";
  import PosterCard from "../components/PosterCard.svelte";
  import ScrollRow from "../components/ScrollRow.svelte";
  import { errorMessage, openUrl, type Details, type DownloadItem, type Episode, type SearchItem } from "../lib/api";
  import { loadDetails } from "../lib/cache";
  import { downloadFor, progressOf } from "../lib/downloads.svelte";
  import { lazySrc, thumb } from "../lib/images";
  import {
    WATCH_STATUSES,
    bookmarkOf,
    episodeLabel,
    episodeProgress,
    library,
    playback,
    setBookmark,
    setEpisodeWatched,
    statusLabel,
    titleKey,
    type QueueEntry,
    type WatchStatus,
  } from "../lib/library.svelte";
  import { router } from "../lib/router.svelte";
  import { showToast } from "../lib/state.svelte";
  import { typeLabel } from "../lib/tvtypes";

  let { provider, url, title, autoplay = false }: { provider: string; url: string; title: string; autoplay?: boolean } =
    $props();

  const RANGE = 50;

  let details = $state<Details | undefined>();
  let error = $state("");
  let season = $state<number | undefined>();
  let dub = $state<string | undefined>();
  let range = $state(0);
  let descending = $state(false);
  let plotOpen = $state(false);
  let dialog = $state<"status" | "season" | "range" | "dub" | null>(null);

  async function load(force = false) {
    error = "";
    details = undefined;
    try {
      details = await loadDetails(provider, url, force);
      if (autoplay) playDefault();
    } catch (e) {
      error = errorMessage(e);
    }
  }

  $effect(() => {
    void load();
  });

  /** SearchItem used for bookmarks / continue watching (keyed by the URL we were opened with). */
  const item = $derived<SearchItem>({
    name: details?.name ?? title,
    url,
    apiName: provider,
    type: details?.type,
    posterUrl: details?.posterUrl,
  });

  const status = $derived.by(() => {
    void library.bookmarks;
    return bookmarkOf(provider, url);
  });
  const resume = $derived(library.progress[titleKey(provider, url)]);

  const dubs = $derived([...new Set(details?.episodes.map((e) => e.dubStatus).filter((d): d is string => !!d))]);
  const activeDub = $derived(dubs.includes(dub ?? "") ? dub : dubs[0]);
  const dubbed = $derived(details?.episodes.filter((e) => !activeDub || e.dubStatus === activeDub) ?? []);
  const seasons = $derived([...new Set(dubbed.map((e) => e.season ?? 0))].sort((a, b) => a - b));
  const activeSeason = $derived(seasons.includes(season ?? -1) ? season : seasons[0]);
  const seasonEpisodes = $derived(dubbed.filter((e) => (e.season ?? 0) === activeSeason));
  const ranges = $derived(
    Array.from({ length: Math.ceil(seasonEpisodes.length / RANGE) }, (_, i) => ({
      from: i * RANGE,
      to: Math.min(seasonEpisodes.length, (i + 1) * RANGE),
    })),
  );
  const activeRange = $derived(Math.min(range, Math.max(0, ranges.length - 1)));
  const shown = $derived.by(() => {
    const r = ranges[activeRange];
    const list = r ? seasonEpisodes.slice(r.from, r.to) : seasonEpisodes;
    return descending ? [...list].reverse() : list;
  });
  const large = $derived(dubbed.some((e) => e.posterUrl || e.description));

  const seasonName = (s: number | undefined) => (s ? `Season ${s}` : "Episodes");
  const rangeName = (i: number) => (ranges[i] ? `${ranges[i].from + 1}-${ranges[i].to}` : "");

  function entry(e: Episode, i: number): QueueEntry {
    return { data: e.data, label: episodeLabel(e, i), season: e.season, episode: e.episode ?? i + 1 };
  }

  function startPlayback(queue: QueueEntry[], index: number) {
    // Coming back from the player must not autoplay again.
    const here = router.current;
    if (here.name === "details") here.autoplay = false;
    playback.item = $state.snapshot(item);
    playback.queue = queue;
    playback.index = index;
    playback.poster = details?.backgroundPosterUrl ?? details?.posterUrl;
    playback.local = null;
    router.push({ name: "player", key: Date.now() });
  }

  // ---- downloads & watched state

  let downloadTarget = $state<{ data: string; episode?: string } | null>(null);

  function askDownload(data: string, episode?: string) {
    const existing = downloadFor(provider, data);
    if (existing && existing.state !== "failed") {
      router.root("downloads");
      return;
    }
    downloadTarget = { data, episode };
  }

  function downloadLabel(d: DownloadItem | undefined): string {
    if (!d) return "Download";
    switch (d.state) {
      case "done":
        return "Downloaded";
      case "downloading":
        return `${Math.round(progressOf(d) * 100)}%`;
      case "queued":
        return "Queued";
      case "paused":
        return "Paused";
      case "failed":
        return "Retry download";
    }
  }

  const seasonWatched = $derived.by(() => {
    void library.episodes;
    return seasonEpisodes.length > 0 && seasonEpisodes.every((e) => episodeProgress(provider, e.data) >= 0.9);
  });

  function toggleSeasonWatched() {
    const watched = !seasonWatched;
    for (const e of seasonEpisodes) setEpisodeWatched(provider, e.data, watched);
  }

  function playEpisode(e: Episode) {
    const index = dubbed.indexOf(e);
    startPlayback(dubbed.map(entry), Math.max(0, index));
  }

  function playMovie() {
    if (details?.playData) startPlayback([{ data: details.playData, label: details.name }], 0);
  }

  function playResume() {
    if (!resume) return;
    const queue = dubbed.length ? dubbed.map(entry) : [{ data: resume.data, label: resume.label }];
    const index = queue.findIndex((q) => q.data === resume.data);
    if (index >= 0) startPlayback(queue, index);
    else startPlayback([{ data: resume.data, label: resume.label, season: resume.season, episode: resume.episode }], 0);
  }

  function playDefault() {
    if (resume) playResume();
    else if (details?.playData) playMovie();
    else if (dubbed[0]) playEpisode(dubbed[0]);
  }

  async function share() {
    try {
      await navigator.clipboard.writeText(url);
      showToast("Link copied to clipboard");
    } catch {
      showToast(url);
    }
  }

  function fmtTime(s: number) {
    const m = Math.floor(s / 60);
    return m >= 60 ? `${Math.floor(m / 60)}h ${m % 60}m` : `${m}m`;
  }

  function fmtDate(ms?: number) {
    return ms ? new Date(ms).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" }) : "";
  }

  function open(i: SearchItem) {
    router.push({ name: "details", provider: i.apiName, url: i.url, title: i.name });
  }
</script>

<div class="page result" data-scroll-root>
  {#if details?.backgroundPosterUrl || details?.posterUrl}
    <div class="backdrop">
      <img src={thumb(details.backgroundPosterUrl ?? details.posterUrl, 640)} alt="" decoding="async" referrerpolicy="no-referrer" />
    </div>
  {/if}

  <div class="topbar">
    <button class="icon-btn" title="Go Back" onclick={() => router.back()}><Icon name="back" size={26} /></button>
    <div class="grow"></div>
    <button class="icon-btn" title="Share" onclick={share}><Icon name="share" size={22} /></button>
    <button class="icon-btn" title="Open In Browser" onclick={() => openUrl(url)}><Icon name="public" size={22} /></button>
    <button class="icon-btn" title="Search" onclick={() => router.root("search", details?.name ?? title)}>
      <Icon name="search" size={24} />
    </button>
  </div>

  {#if error}
    <div class="center-fill">
      <p class="error-text">{error}</p>
      <button class="btn btn-white" onclick={() => load(true)}><Icon name="autorenew" size={20} /> Retry connection…</button>
      <button class="btn btn-black" onclick={() => openUrl(url)}><Icon name="public" size={20} /> Open In Browser</button>
    </div>
  {:else if !details}
    <div class="content">
      <div class="header">
        <div class="poster shimmer"></div>
        <div class="meta-col">
          <div class="shimmer" style="height: 24px; width: 60%"></div>
          <div class="shimmer" style="height: 15px; width: 40%"></div>
          <div class="shimmer" style="height: 15px; width: 90%"></div>
        </div>
      </div>
    </div>
  {:else}
    <div class="content">
      <div class="header">
        {#if details.posterUrl}
          <img class="poster" src={thumb(details.posterUrl, 100)} alt="Poster" decoding="async" referrerpolicy="no-referrer" />
        {/if}
        <div class="meta-col">
          <h1>{details.name}</h1>
          <div class="meta">
            <span class="small-black">{provider}</span>
            {#if details.type}<span>{typeLabel(details.type)}</span>{/if}
            {#if details.year}<span>{details.year}</span>{/if}
            {#if details.duration}<span>{details.duration} min</span>{/if}
            {#if details.rating}<span class="rating">★ {details.rating.toFixed(1)}</span>{/if}
            {#if details.contentRating}<span class="small-black">{details.contentRating}</span>{/if}
            {#if details.comingSoon}<span class="badge soon">Coming soon</span>{/if}
          </div>
        </div>
      </div>

      {#if details.plot}
        <button class="plot" class:open={plotOpen} onclick={() => (plotOpen = !plotOpen)}>{details.plot}</button>
      {/if}

      <div class="button-row">
        <button class="btn btn-black" onclick={() => (dialog = "status")}>
          <Icon name={status ? "bookmark" : "bookmarkBorder"} size={22} />
          {statusLabel(status)}
        </button>
        {#if details.trailers.length}
          <button class="btn btn-black" onclick={() => openUrl(details!.trailers[0])} title="Opens in your browser">
            <Icon name="playBig" size={18} /> Trailer
          </button>
        {/if}
      </div>

      {#if details.tags.length}
        <h3 class="sub-title">Genres</h3>
        <div class="tags">
          {#each details.tags as tag}<span class="tag">{tag}</span>{/each}
        </div>
      {/if}

      {#if details.actors.length}
        <h3 class="sub-title">Cast</h3>
        <ScrollRow inset={0}>
          {#each details.actors.slice(0, 30) as actor (actor.name)}
            <div class="actor">
              <div class="actor-img">
                {#if actor.image}
                  <img {@attach lazySrc(thumb(actor.image, 64))} alt="" decoding="async" referrerpolicy="no-referrer" />
                {:else}
                  <span>{actor.name.slice(0, 1)}</span>
                {/if}
              </div>
              <span class="actor-name">{actor.name}</span>
              {#if actor.role}<span class="actor-role gray">{actor.role}</span>{/if}
            </div>
          {/each}
        </ScrollRow>
      {/if}

      {#if resume}
        <div class="resume">
          <button class="btn btn-white wide" onclick={playResume}>
            <Icon name="play" size={22} /> Resume
          </button>
          <div class="resume-info">
            <span>{resume.label}</span>
            <span class="gray">{fmtTime(Math.max(0, resume.duration - resume.position))} remaining</span>
          </div>
          <div class="resume-bar"><div style:width="{(resume.position / resume.duration) * 100}%"></div></div>
        </div>
      {/if}

      {#if details.playData}
        {@const dl = downloadFor(provider, details.playData)}
        <div class="movie-row">
          <button class="btn btn-white wide" onclick={playMovie}>
            <Icon name="play" size={22} />
            {details.type === "Live" ? "Play Livestream" : "Play Movie"}
          </button>
          {#if details.type !== "Live"}
            <button
              class="btn btn-black download"
              onclick={() => (dl?.state === "done" ? router.root("downloads") : askDownload(details!.playData!))}
            >
              <Icon name={dl?.state === "done" ? "check" : "download"} size={20} />
              {downloadLabel(dl)}
            </button>
          {/if}
        </div>
      {/if}

      {#if details.episodes.length}
        <div class="selectors">
          {#if seasons.length > 1 || activeSeason}
            <button class="btn btn-black select" onclick={() => (dialog = "season")}>
              {seasonName(activeSeason)} <Icon name="expandMore" size={20} />
            </button>
          {/if}
          {#if ranges.length > 1}
            <button class="btn btn-black select" onclick={() => (dialog = "range")}>
              {rangeName(activeRange)} <Icon name="expandMore" size={20} />
            </button>
          {/if}
          {#if dubs.length > 1}
            <button class="btn btn-black select" onclick={() => (dialog = "dub")}>
              {activeDub} <Icon name="expandMore" size={20} />
            </button>
          {/if}
          <button class="btn btn-black select" onclick={() => (descending = !descending)} title="Sort">
            <Icon name="sort" size={20} />
            {descending ? "Descending" : "Ascending"}
          </button>
        </div>

        <div class="episodes-head">
          <h3 class="sub-title">{seasonEpisodes.length} Episodes</h3>
          <button class="text-btn" onclick={toggleSeasonWatched}>
            <Icon name="check" size={16} />
            {seasonWatched ? "Mark season unwatched" : "Mark season watched"}
          </button>
        </div>

        <div class="episodes" class:large>
          {#each shown as e (e.data)}
            {@const i = seasonEpisodes.indexOf(e)}
            {@const done = episodeProgress(provider, e.data)}
            {@const watched = done >= 0.9}
            {@const dl = downloadFor(provider, e.data)}
            {#snippet actions()}
              <div class="ep-actions">
                <button
                  class="ep-action"
                  class:on={watched}
                  title={watched ? "Mark as unwatched" : "Mark as watched"}
                  onclick={() => setEpisodeWatched(provider, e.data, !watched)}
                >
                  <Icon name="check" size={18} />
                </button>
                <button
                  class="ep-action"
                  class:on={dl?.state === "done"}
                  title={dl ? downloadLabel(dl) : "Download"}
                  onclick={() => (dl?.state === "done" ? router.root("downloads") : askDownload(e.data, episodeLabel(e, i)))}
                >
                  {#if dl && dl.state !== "done"}
                    <span class="ep-pct">{dl.state === "failed" ? "!" : `${Math.round(progressOf(dl) * 100)}`}</span>
                  {:else}
                    <Icon name={dl ? "check" : "download"} size={18} />
                  {/if}
                </button>
              </div>
            {/snippet}
            {#if large}
              <div class="ep-large" class:watched>
                <button class="ep-main" onclick={() => playEpisode(e)}>
                  <div class="ep-row">
                    <div class="ep-thumb">
                      {#if e.posterUrl}
                        <img {@attach lazySrc(thumb(e.posterUrl, 128))} alt="" decoding="async" referrerpolicy="no-referrer" />
                      {/if}
                      <span class="ep-play"><Icon name="play" size={22} /></span>
                      {#if done > 0}<div class="ep-progress"><div style:width="{done * 100}%"></div></div>{/if}
                    </div>
                    <div class="ep-text">
                      <span class="ep-title">{e.episode ?? i + 1}. {e.name ?? `Episode ${e.episode ?? i + 1}`}</span>
                      <span class="gray small">
                        {[e.runTime ? `${e.runTime} min` : "", fmtDate(e.date)].filter(Boolean).join(" • ")}
                      </span>
                    </div>
                  </div>
                  {#if e.description}<p class="ep-desc gray">{e.description}</p>{/if}
                </button>
                {@render actions()}
              </div>
            {:else}
              <div class="ep" class:watched>
                <button class="ep-main" onclick={() => playEpisode(e)}>
                  {e.name ? `${e.episode ?? i + 1}. ${e.name}` : `Episode ${e.episode ?? i + 1}`}
                </button>
                {@render actions()}
                {#if done > 0}<div class="ep-progress"><div style:width="{done * 100}%"></div></div>{/if}
              </div>
            {/if}
          {/each}
        </div>
      {:else if !details.playData}
        <p class="gray no-eps">No Episodes found</p>
      {/if}

      {#if details.recommendations.length}
        <h3 class="sub-title">Recommended</h3>
        <ScrollRow inset={0}>
          {#each details.recommendations as rec (rec.url)}
            <PosterCard item={rec} onselect={open} />
          {/each}
        </ScrollRow>
      {/if}
      <div style="height: 40px"></div>
    </div>
  {/if}
</div>

{#if downloadTarget}
  <DownloadDialog
    {provider}
    data={downloadTarget.data}
    title={details?.name ?? title}
    episode={downloadTarget.episode}
    poster={details?.posterUrl}
    pageUrl={url}
    onclose={() => (downloadTarget = null)}
  />
{/if}

{#if dialog === "status"}
  <Dialog title="Set watch status" onclose={() => (dialog = null)} width={340}>
    {#each [{ id: undefined, label: "None" }, ...WATCH_STATUSES] as s (s.label)}
      <button class="list-item" onclick={() => (setBookmark(item, s.id as WatchStatus | undefined), (dialog = null))}>
        <span class="radio" class:on={status === s.id}></span>{s.label}
      </button>
    {/each}
  </Dialog>
{:else if dialog === "season"}
  <Dialog title="Season" onclose={() => (dialog = null)} width={320}>
    {#each seasons as s}
      <button class="list-item" onclick={() => ((season = s), (range = 0), (dialog = null))}>
        <span class="radio" class:on={s === activeSeason}></span>{seasonName(s)}
      </button>
    {/each}
  </Dialog>
{:else if dialog === "range"}
  <Dialog title="Episodes" onclose={() => (dialog = null)} width={320}>
    {#each ranges as _, i}
      <button class="list-item" onclick={() => ((range = i), (dialog = null))}>
        <span class="radio" class:on={i === activeRange}></span>{rangeName(i)}
      </button>
    {/each}
  </Dialog>
{:else if dialog === "dub"}
  <Dialog onclose={() => (dialog = null)} width={320}>
    {#each dubs as d}
      <button class="list-item" onclick={() => ((dub = d), (season = undefined), (range = 0), (dialog = null))}>
        <span class="radio" class:on={d === activeDub}></span>{d}
      </button>
    {/each}
  </Dialog>
{/if}

<style>
  .result {
    position: relative;
  }

  .backdrop {
    position: absolute;
    inset: 0 0 auto 0;
    height: 360px;
    overflow: hidden;
    pointer-events: none;
  }

  .backdrop img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0.35;
    filter: blur(2px);
  }

  .backdrop::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(to bottom, rgba(17, 17, 17, 0.2), var(--bg));
  }

  .topbar {
    position: sticky;
    top: 0;
    z-index: 3;
    display: flex;
    align-items: center;
    gap: 4px;
    height: 56px;
    padding: 0 8px;
  }

  .grow {
    flex: 1;
  }

  .content {
    position: relative;
    max-width: 1100px;
    padding: 0 15px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .header {
    display: flex;
    gap: 15px;
    align-items: flex-end;
  }

  .poster {
    width: 100px;
    height: 140px;
    flex-shrink: 0;
    object-fit: cover;
    border-radius: var(--radius);
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.5);
  }

  .meta-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h1 {
    font-size: 20px;
    font-weight: 700;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    color: var(--text);
    font-size: 14px;
  }

  .small-black {
    padding: 2px 8px;
    border-radius: 4px;
    border: 1px solid var(--text);
    background: var(--icon-gray);
    font-size: 12px;
    font-weight: 700;
  }

  .plot {
    text-align: left;
    color: var(--text-gray);
    font-size: 15px;
    line-height: 1.45;
    max-width: 900px;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
    user-select: text;
  }

  .plot.open {
    -webkit-line-clamp: unset;
    line-clamp: unset;
  }

  .button-row,
  .movie-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .movie-row {
    max-width: 900px;
    flex-wrap: nowrap;
  }

  .movie-row .wide {
    flex: 1;
  }

  .movie-row .download {
    height: 44px;
    min-width: 150px;
  }

  .rating {
    color: #ffc93c;
    font-weight: 700;
  }

  .soon {
    background: var(--primary);
    color: #fff;
  }

  .actor {
    width: 84px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    text-align: center;
  }

  .actor-img {
    width: 64px;
    height: 64px;
    border-radius: 50%;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--bg-gray);
    color: var(--text-gray);
    font-size: 22px;
    font-weight: 700;
  }

  .actor-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .actor-img img:not([src]) {
    visibility: hidden;
  }

  .actor-name,
  .actor-role {
    width: 100%;
    font-size: 12px;
    line-height: 1.25;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actor-role {
    font-size: 11px;
  }

  .episodes-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    max-width: 900px;
  }

  .text-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 6px;
    color: var(--primary);
    font-weight: 700;
    font-size: 13px;
  }

  .text-btn:hover {
    background: var(--ripple);
  }

  .sub-title {
    font-size: 17px;
    font-weight: 700;
    margin-top: 6px;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .tag {
    padding: 5px 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.25);
    font-size: 13px;
  }

  .wide {
    width: 100%;
    max-width: 900px;
    height: 44px;
    font-size: 15px;
  }

  .resume {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 900px;
  }

  .resume-info {
    display: flex;
    justify-content: space-between;
    font-size: 14px;
  }

  .resume-bar,
  .ep-progress {
    height: 4px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 2px;
    overflow: hidden;
  }

  .resume-bar div,
  .ep-progress div {
    height: 100%;
    background: var(--primary);
  }

  .selectors {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }

  .select {
    gap: 4px;
    padding: 0 12px 0 14px;
  }

  .episodes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 8px;
  }

  .episodes.large {
    grid-template-columns: repeat(auto-fill, minmax(420px, 1fr));
    gap: 12px;
  }

  .ep {
    position: relative;
    display: flex;
    align-items: center;
    min-height: 50px;
    border-radius: var(--radius);
    background: var(--bg-gray);
    overflow: hidden;
  }

  .ep .ep-main {
    flex: 1;
    min-width: 0;
    align-self: stretch;
    padding: 0 6px 0 15px;
    text-align: left;
    font-weight: 500;
  }

  .ep:hover,
  .ep-large:hover {
    background: #34353a;
  }

  .watched .ep-title,
  .ep.watched .ep-main {
    color: var(--text-gray);
  }

  .ep .ep-progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    border-radius: 0;
  }

  .ep-large {
    position: relative;
    border-radius: var(--radius);
    background: var(--bg-gray);
  }

  .ep-large .ep-main {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 80px 8px 8px;
    text-align: left;
  }

  .ep-large .ep-actions {
    position: absolute;
    top: 8px;
    right: 8px;
  }

  .ep-actions {
    display: flex;
    gap: 2px;
    padding-right: 6px;
  }

  .ep-action {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    color: var(--text-gray);
  }

  .ep-action:hover {
    background: var(--ripple);
    color: var(--text);
  }

  .ep-action.on {
    color: var(--primary);
  }

  .ep-pct {
    font-size: 11px;
    font-weight: 700;
    color: var(--primary);
  }

  .ep-row {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .ep-thumb {
    position: relative;
    width: 128px;
    height: 72px;
    flex-shrink: 0;
    border-radius: 8px;
    overflow: hidden;
    background: var(--icon-gray);
  }

  .ep-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .ep-thumb img:not([src]) {
    visibility: hidden;
  }

  .ep-play {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 36px;
    height: 36px;
    transform: translate(-50%, -50%);
    display: grid;
    place-items: center;
    padding-left: 2px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.7);
    color: #fff;
  }

  .ep-thumb .ep-progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    border-radius: 0;
  }

  .ep-text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .ep-title {
    font-weight: 700;
    font-size: 15px;
  }

  .small {
    font-size: 12px;
  }

  .ep-desc {
    font-size: 13px;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .no-eps {
    padding: 20px 0;
  }

</style>
