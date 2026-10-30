<script lang="ts">
  // Pick which source to download (Android shows the same source list before downloading).
  import { onMount } from "svelte";
  import { engine, errorMessage, type Links, type StreamLink } from "../lib/api";
  import { canDownload, startDownload } from "../lib/downloads.svelte";
  import { uniqueBy } from "../lib/unique";
  import Dialog from "./Dialog.svelte";

  let {
    provider,
    data,
    title,
    episode,
    poster,
    pageUrl,
    onclose,
  }: {
    provider: string;
    data: string;
    title: string;
    episode?: string;
    poster?: string;
    pageUrl: string;
    onclose: () => void;
  } = $props();

  let links = $state<Links | undefined>();
  let error = $state("");
  let busy = $state(false);

  onMount(async () => {
    try {
      const found = await engine.links(provider, data);
      links = { links: uniqueBy(found.links, (l) => l.url), subtitles: uniqueBy(found.subtitles, (s) => s.url) };
    } catch (e) {
      error = errorMessage(e);
    }
  });

  async function pick(link: StreamLink) {
    if (!links || busy) return;
    busy = true;
    try {
      await startDownload({ title, episode, poster, provider, pageUrl, data }, link, links.subtitles);
      onclose();
    } catch (e) {
      error = errorMessage(e);
      busy = false;
    }
  }
</script>

<Dialog title={episode ? `Download · ${episode}` : "Download"} {onclose} width={520}>
  {#if error}
    <p class="msg error-text">{error}</p>
  {:else if !links}
    <div class="msg"><div class="spinner small"></div> Finding sources…</div>
  {:else if !links.links.length}
    <p class="msg gray">No sources found.</p>
  {:else}
    {#each links.links as link (link.url)}
      {@const ok = canDownload(link)}
      <button class="list-item" disabled={!ok || busy} onclick={() => pick(link)}>
        <span class="name">{link.name}</span>
        {#if link.quality > 0}<span class="gray">{link.quality}p</span>{/if}
        <span class="badge">{link.type === "M3U8" ? "HLS" : ok ? "MP4" : link.type}</span>
      </button>
    {/each}
    {#if links.subtitles.length}
      <p class="msg gray small">{links.subtitles.length} subtitle file(s) will be saved too.</p>
    {/if}
  {/if}
</Dialog>

<style>
  .msg {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 16px 12px;
  }

  .small {
    font-size: 12px;
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
