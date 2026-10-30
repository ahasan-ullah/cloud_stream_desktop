<script lang="ts">
  // Android res/layout/fragment_downloads.xml + download_child_episode.xml.
  import Dialog from "../components/Dialog.svelte";
  import Icon from "../components/Icon.svelte";
  import { downloadsApi, errorMessage, type DownloadItem } from "../lib/api";
  import { chooseFolder, downloads, formatBytes, label, progressOf } from "../lib/downloads.svelte";
  import { lazySrc, thumb } from "../lib/images";
  import { playback } from "../lib/library.svelte";
  import { router } from "../lib/router.svelte";
  import { showToast } from "../lib/state.svelte";

  let streamDialog = $state(false);
  let url = $state("");
  let referer = $state("");
  let deleting = $state<DownloadItem | null>(null);
  let deleteFiles = $state(true);

  const items = $derived([...downloads.items].sort((a, b) => b.createdAt - a.createdAt));
  const active = $derived(items.filter((d) => d.state === "downloading").length);
  const usedBytes = $derived(items.reduce((sum, d) => sum + d.bytesDone, 0));

  function playStream() {
    const link = url.trim();
    if (!/^https?:\/\//i.test(link)) return;
    const name = decodeURIComponent(link.split("?")[0].split("/").pop() || "Network stream");
    playback.item = { name, url: link, apiName: "__stream__" };
    playback.queue = [{ data: JSON.stringify({ url: link, referer: referer.trim() }), label: name }];
    playback.index = 0;
    playback.poster = undefined;
    playback.local = null;
    streamDialog = false;
    router.push({ name: "player", key: Date.now() });
  }

  async function play(d: DownloadItem) {
    try {
      const local = await downloadsApi.playback(d.id);
      playback.item = { name: d.title, url: d.pageUrl, apiName: d.provider, posterUrl: d.poster };
      playback.queue = [{ data: d.data, label: d.episode ?? d.title }];
      playback.index = 0;
      playback.poster = d.poster;
      playback.local = local;
      router.push({ name: "player", key: Date.now() });
    } catch (e) {
      showToast(errorMessage(e));
    }
  }

  async function confirmDelete() {
    if (!deleting) return;
    await downloadsApi.remove(deleting.id, deleteFiles);
    deleting = null;
  }

  function status(d: DownloadItem): string {
    const size = d.bytesTotal && d.state !== "done" ? `${formatBytes(d.bytesDone)} / ${formatBytes(d.bytesTotal)}` : formatBytes(d.bytesDone);
    switch (d.state) {
      case "downloading": {
        const pct = Math.round(progressOf(d) * 100);
        return `${pct}% · ${size}${d.speed ? ` · ${formatBytes(d.speed)}/s` : ""}`;
      }
      case "queued":
        return "Queued";
      case "paused":
        return `Paused · ${size}`;
      case "done":
        return size;
      case "failed":
        return d.error ?? "Failed";
    }
  }
</script>

<div class="downloads">
  <div class="appbar">
    <h1>Downloads</h1>
    {#if items.length}
      <span class="gray summary">{items.length} items · {formatBytes(usedBytes)}{active ? ` · ${active} active` : ""}</span>
    {/if}
    <div class="grow"></div>
    <button class="folder" title="Change where new downloads are saved" onclick={chooseFolder}>
      <Icon name="library" size={18} />
      <span class="path">{downloads.folder || "Downloads folder"}</span>
      <span class="change">Change</span>
    </button>
    <button class="icon-btn" title="Open downloads folder" onclick={() => downloadsApi.reveal()}>
      <Icon name="external" size={20} />
    </button>
  </div>

  {#if !items.length}
    <div class="center-fill empty">
      <Icon name="download" size={56} />
      <p>There are currently no downloads.</p>
      <p class="gray small">Use the download button on a movie or episode.</p>
    </div>
  {:else}
    <div class="page list" data-scroll-root>
      {#each items as d (d.id)}
        <div class="row" class:failed={d.state === "failed"}>
          <button class="poster" onclick={() => d.state === "done" && play(d)} title={d.state === "done" ? "Play" : ""}>
            {#if d.poster}<img {@attach lazySrc(thumb(d.poster, 60))} alt="" referrerpolicy="no-referrer" />{/if}
            {#if d.state === "done"}<span class="play"><Icon name="play" size={22} /></span>{/if}
          </button>
          <div class="info">
            <span class="title">{label(d)}</span>
            <span class="gray small">{d.linkName}{d.hls ? " · HLS" : ""}</span>
            {#if d.state !== "done"}
              <div class="bar"><div style:width="{progressOf(d) * 100}%"></div></div>
            {/if}
            <span class="small" class:error-text={d.state === "failed"} class:gray={d.state !== "failed"}>{status(d)}</span>
          </div>
          <div class="actions">
            {#if d.state === "downloading" || d.state === "queued"}
              <button class="icon-btn" title="Pause" onclick={() => downloadsApi.pause(d.id)}><Icon name="pauseBig" size={22} /></button>
            {:else if d.state === "paused" || d.state === "failed"}
              <button class="icon-btn" title="Resume" onclick={() => downloadsApi.resume(d.id)}><Icon name="download" size={24} /></button>
            {:else}
              <button class="icon-btn" title="Play" onclick={() => play(d)}><Icon name="play" size={24} /></button>
            {/if}
            <button class="icon-btn" title="Show in folder" onclick={() => downloadsApi.reveal(d.id)}><Icon name="library" size={20} /></button>
            <button class="icon-btn" title="Delete" onclick={() => ((deleting = d), (deleteFiles = true))}><Icon name="delete" size={22} /></button>
          </div>
        </div>
      {/each}
      <div style="height: 90px"></div>
    </div>
  {/if}

  <button class="fab" onclick={() => (streamDialog = true)}>
    <Icon name="stream" size={22} /> Network stream
  </button>
</div>

{#if streamDialog}
  <Dialog title="Network stream" onclose={() => (streamDialog = false)} width={480}>
    <form class="form" onsubmit={(e) => (e.preventDefault(), playStream())}>
      <input type="text" placeholder="https://…/video.m3u8 or .mp4" bind:value={url} />
      <input type="text" placeholder="Referer (optional)" bind:value={referer} />
    </form>
    {#snippet footer()}
      <button class="btn btn-black" onclick={() => (streamDialog = false)}>Cancel</button>
      <button class="btn btn-white" onclick={playStream} disabled={!/^https?:\/\//i.test(url.trim())}>Play</button>
    {/snippet}
  </Dialog>
{/if}

{#if deleting}
  {@const d = deleting}
  <Dialog title="Delete" onclose={() => (deleting = null)} width={400}>
    <p class="confirm">Remove “{label(d)}” from downloads?</p>
    <label class="check"><input type="checkbox" bind:checked={deleteFiles} /> Also delete the file from disk</label>
    {#snippet footer()}
      <button class="btn btn-black" onclick={() => (deleting = null)}>Cancel</button>
      <button class="btn btn-white" onclick={confirmDelete}>Delete</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .downloads {
    position: relative;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .appbar {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 56px;
    padding: 0 12px 0 20px;
    background: var(--bg-gray);
    flex-shrink: 0;
  }

  .appbar h1 {
    font-size: 20px;
    font-weight: 500;
  }

  .summary,
  .small {
    font-size: 12px;
  }

  .folder {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    max-width: 46%;
    height: 34px;
    padding: 0 12px;
    border-radius: 17px;
    background: var(--bg);
    color: var(--text-gray);
    font-size: 13px;
  }

  .folder:hover {
    color: var(--text);
  }

  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl; /* keep the end of long paths visible */
  }

  .change {
    color: var(--primary);
    font-weight: 700;
    flex-shrink: 0;
  }

  .grow {
    flex: 1;
  }

  .empty {
    flex: 1;
  }

  .list {
    flex: 1;
    min-height: 0;
    padding: 8px 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 12px 10px 20px;
  }

  .row:hover {
    background: var(--ripple);
  }

  .poster {
    position: relative;
    width: 60px;
    height: 90px;
    flex-shrink: 0;
    border-radius: 8px;
    overflow: hidden;
    background: var(--bg-gray);
  }

  .poster img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .poster img:not([src]) {
    visibility: hidden;
  }

  .play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.35);
    color: #fff;
  }

  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .title {
    font-size: 15px;
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .bar {
    height: 4px;
    max-width: 520px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.1);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--primary);
    transition: width 0.4s;
  }

  .actions {
    display: flex;
    gap: 2px;
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

  .check {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 16px 8px;
    cursor: pointer;
  }

  .check input {
    accent-color: var(--primary);
  }
</style>
