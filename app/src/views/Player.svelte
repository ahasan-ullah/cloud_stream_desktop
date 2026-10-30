<script lang="ts">
  // Mirrors CloudStream's custom player overlay (player_custom_layout.xml).
  import type Hls from "hls.js";
  import { onDestroy, onMount } from "svelte";
  import Dialog from "../components/Dialog.svelte";
  import Icon from "../components/Icon.svelte";
  import { engine, errorMessage, openInMpv, proxyUrl, type Links, type LocalPlayback, type StreamLink } from "../lib/api";
  import { thumb } from "../lib/images";
  import { uniqueBy } from "../lib/unique";
  import { addWatched, playback, saveProgress, savedPosition } from "../lib/library.svelte";
  import { router } from "../lib/router.svelte";
  import { settings, showToast } from "../lib/state.svelte";

  const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];
  const RESIZE = [
    { label: "Fit", fit: "contain" },
    { label: "Fill", fit: "fill" },
    { label: "Zoom", fit: "cover" },
  ] as const;

  const item = playback.item;
  const entry = playback.queue[playback.index];
  const hasNext = playback.index + 1 < playback.queue.length;

  let video: HTMLVideoElement;
  let root: HTMLDivElement;
  let hls: Hls | null = null;

  let links = $state<Links | undefined>();
  let loadError = $state("");
  let current = $state<StreamLink | undefined>();
  let playError = $state("");
  let tracks = $state<{ label: string; src: string; url: string }[]>([]);
  let subtitle = $state(-1);

  let paused = $state(true);
  let buffering = $state(false);
  let time = $state(0);
  let duration = $state(0);
  let buffered = $state(0);
  let speed = $state(settings.speed);
  let resize = $state(0);
  let fullscreen = $state(false);
  let showControls = $state(true);
  let dialog = $state<"sources" | "speed" | "quality" | "audio" | "subtitles" | null>(null);

  /** HLS renditions and alternate audio (hls.js). -1 = automatic quality. */
  let levels = $state<{ index: number; label: string }[]>([]);
  let level = $state(-1);
  let audioTracks = $state<{ index: number; label: string }[]>([]);
  let audioTrack = $state(0);

  /** Subtitle offset in seconds, applied by shifting cue times. */
  let subDelay = $state(0);
  const appliedDelay = new WeakMap<TextTrack, number>();

  let pip = $state(false);
  const pipSupported = typeof document !== "undefined" && document.pictureInPictureEnabled;
  let introSkipped = $state(false);
  /** Seconds left before the next episode starts (Netflix-style countdown), or null. */
  let countdown = $state<number | null>(null);
  let countdownTimer: ReturnType<typeof setInterval> | undefined;
  let recorded = false;

  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let lastSaved = 0;

  /** Pseudo provider for Downloads → Network stream. */
  const STREAM = "__stream__";

  function streamLinks(data: string): Links {
    const { url, referer } = JSON.parse(data) as { url: string; referer: string };
    const type = /\.m3u8(\?|$)/i.test(url) ? "M3U8" : /\.mpd(\?|$)/i.test(url) ? "DASH" : "VIDEO";
    return {
      links: [{ name: "Network stream", source: "stream", url, quality: 0, type, headers: referer ? { Referer: referer } : {} }],
      subtitles: [],
    };
  }

  /** Finished downloads are already served by the local proxy (no headers needed). */
  const isLocal = (url: string) => url.startsWith("http://127.0.0.1:");

  function localLinks(local: LocalPlayback): Links {
    return {
      links: [{ name: "Downloaded", source: "local", url: local.url, quality: 0, type: local.hls ? "M3U8" : "VIDEO", headers: {} }],
      subtitles: local.subtitles.map((s) => ({ lang: s.lang, url: s.url, headers: {} })),
    };
  }

  const inApp = (l: StreamLink) => l.type === "VIDEO" || l.type === "M3U8";
  const quality = (l: StreamLink) => (l.quality > 0 ? `${l.quality}p` : "");

  onMount(async () => {
    document.addEventListener("fullscreenchange", onFullscreenChange);
    if (!item || !entry) {
      loadError = "Nothing to play.";
      return;
    }
    try {
      const found = playback.local
        ? localLinks(playback.local)
        : item.apiName === STREAM
          ? streamLinks(entry.data)
          : await engine.links(item.apiName, entry.data);
      links = {
        links: uniqueBy(found.links, (l) => l.url),
        subtitles: uniqueBy(found.subtitles, (s) => s.url),
      };
    } catch (e) {
      loadError = errorMessage(e);
      return;
    }
    tracks = await Promise.all(
      links.subtitles.map(async (s) => ({
        label: s.lang,
        url: s.url,
        src: isLocal(s.url) ? s.url : await proxyUrl(s.url, s.headers, "sub"),
      })),
    );
    const english = tracks.findIndex((t) => /^en|english/i.test(t.label));
    subtitle = english;
    const first = links.links.find(inApp) ?? links.links[0];
    if (!first) return;
    if (settings.preferMpv) await mpv(first);
    else await play(first);
  });

  onDestroy(() => {
    document.removeEventListener("fullscreenchange", onFullscreenChange);
    persist();
    stopHls();
    // Release the decoder and its GPU surfaces now instead of whenever the element is collected.
    if (video) {
      video.pause();
      video.removeAttribute("src");
      video.load();
    }
    clearTimeout(hideTimer);
    clearInterval(countdownTimer);
    if (document.fullscreenElement) void document.exitFullscreen();
    if (document.pictureInPictureElement) void document.exitPictureInPicture();
  });

  function stopHls() {
    hls?.destroy();
    hls = null;
  }

  async function play(link: StreamLink) {
    const resumeAt = current ? video.currentTime : 0;
    stopHls();
    current = link;
    playError = "";
    video.removeAttribute("src");
    if (!inApp(link)) {
      playError = `${link.type} streams can't play in the built-in player. Use mpv from Sources.`;
      return;
    }
    const isHls = link.type === "M3U8";
    const src =
      isLocal(link.url) && !Object.keys(link.headers).length
        ? link.url
        : await proxyUrl(link.url, link.headers, isHls ? "m3u8" : undefined);
    // Loaded on demand: hls.js is the bulk of the bundle and only HLS sources need it.
    const Hls = isHls ? (await import("hls.js")).default : undefined;
    if (current !== link) return; // Another source was picked while loading.
    video.dataset.resume = String(resumeAt || (item ? savedPosition(item.apiName, entry.data) : 0));
    levels = [];
    level = -1;
    audioTracks = [];
    if (Hls?.isSupported()) {
      // Smaller forward/back buffers than hls.js defaults (60 MB / unbounded back buffer).
      const instance = new Hls({
        enableWorker: true,
        maxBufferLength: 30,
        maxMaxBufferLength: 60,
        maxBufferSize: 30 * 1000 * 1000,
        backBufferLength: 30,
      });
      hls = instance;
      instance.on(Hls.Events.ERROR, (_, d) => {
        if (d.fatal) playError = `Playback failed (${d.details}). Try another source.`;
      });
      instance.on(Hls.Events.MANIFEST_PARSED, () => {
        levels = instance.levels
          .map((l, index) => ({ index, height: l.height, label: l.height ? `${l.height}p` : `${Math.round(l.bitrate / 1000)} kbps` }))
          .sort((a, b) => b.height - a.height)
          .map(({ index, label }) => ({ index, label }));
      });
      instance.on(Hls.Events.AUDIO_TRACKS_UPDATED, () => {
        audioTracks = instance.audioTracks.map((t, index) => ({ index, label: t.name || t.lang || `Track ${index + 1}` }));
        audioTrack = instance.audioTrack;
      });
      instance.loadSource(src);
      instance.attachMedia(video);
    } else {
      video.src = src;
    }
    video.playbackRate = speed;
    video.play().catch(() => {});
  }

  function onLoadedMetadata() {
    duration = video.duration;
    const resumeAt = Number(video.dataset.resume ?? 0);
    if (resumeAt > 5 && resumeAt < duration * 0.95) video.currentTime = resumeAt;
    video.volume = settings.volume;
    video.playbackRate = speed;
    applySubtitle();
  }

  function onPlaying() {
    buffering = false;
    if (!recorded && item && entry && item.apiName !== STREAM) {
      recorded = true;
      addWatched(item, entry);
    }
  }

  // ---- quality, audio, subtitles

  function setLevel(index: number) {
    level = index;
    if (hls) hls.currentLevel = index;
    dialog = null;
  }

  function setAudio(index: number) {
    audioTrack = index;
    if (hls) hls.audioTrack = index;
    dialog = null;
  }

  function selectSubtitle(index: number) {
    subtitle = index;
    applySubtitle();
  }

  /** Shifts the cues of every loaded track to the current delay. */
  function applyDelay() {
    for (let i = 0; i < video.textTracks.length; i++) {
      const track = video.textTracks[i];
      if (!track.cues?.length) continue;
      const shift = subDelay - (appliedDelay.get(track) ?? 0);
      if (!shift) continue;
      for (let c = 0; c < track.cues.length; c++) {
        const cue = track.cues[c];
        cue.startTime = Math.max(0, cue.startTime + shift);
        cue.endTime = Math.max(0, cue.endTime + shift);
      }
      appliedDelay.set(track, subDelay);
    }
  }

  function changeDelay(delta: number) {
    subDelay = Math.round((subDelay + delta) * 10) / 10;
    applyDelay();
    showToast(`Subtitle delay ${subDelay > 0 ? "+" : ""}${subDelay.toFixed(1)} s`);
  }

  // ---- picture-in-picture, skip intro, next episode

  /** Attachment: PiP enter/leave events (not in Svelte's element typings). */
  function trackPip(node: HTMLVideoElement) {
    const enter = () => (pip = true);
    const leave = () => (pip = false);
    node.addEventListener("enterpictureinpicture", enter);
    node.addEventListener("leavepictureinpicture", leave);
    return () => {
      node.removeEventListener("enterpictureinpicture", enter);
      node.removeEventListener("leavepictureinpicture", leave);
    };
  }

  async function togglePip() {
    try {
      if (document.pictureInPictureElement) await document.exitPictureInPicture();
      else await video.requestPictureInPicture();
    } catch (e) {
      showToast(errorMessage(e));
    }
  }

  /** Same jump as Android's "Skip OP" button. */
  function skipIntro() {
    introSkipped = true;
    seekBy(85);
  }

  function startCountdown() {
    clearInterval(countdownTimer);
    countdown = 5;
    countdownTimer = setInterval(() => {
      if (countdown === null) return clearInterval(countdownTimer);
      countdown -= 1;
      if (countdown <= 0) {
        clearInterval(countdownTimer);
        goTo(playback.index + 1);
      }
    }, 1000);
  }

  function cancelCountdown() {
    clearInterval(countdownTimer);
    countdown = null;
  }

  function onTimeUpdate() {
    time = video.currentTime;
    duration = video.duration || 0;
    if (video.buffered.length) buffered = video.buffered.end(video.buffered.length - 1);
    if (Date.now() - lastSaved > 5000) persist();
  }

  function persist() {
    if (!item || !entry || item.apiName === STREAM) return;
    if (!video || !video.duration || !isFinite(video.duration) || video.currentTime < 3) return;
    lastSaved = Date.now();
    saveProgress(item, entry, video.currentTime, video.duration);
  }

  function applySubtitle() {
    for (let i = 0; i < video.textTracks.length; i++) {
      video.textTracks[i].mode = i === subtitle ? "showing" : "disabled";
    }
  }

  function goTo(index: number) {
    persist();
    playback.index = index;
    router.replace({ name: "player", key: Date.now() });
  }

  function onEnded() {
    persist();
    if (hasNext && settings.autoNextEpisode) startCountdown();
  }

  async function mpv(link = current) {
    if (!link || !links) return;
    try {
      await openInMpv({
        mpvPath: settings.mpvPath || undefined,
        url: link.url,
        title: entry ? `${item?.name} - ${entry.label}` : (item?.name ?? ""),
        headers: link.headers,
        subtitles: links.subtitles.map((s) => s.url),
      });
      video?.pause();
      showToast(`Opened "${link.name}" in mpv`);
    } catch (e) {
      showToast(errorMessage(e));
    }
  }

  // ---- controls

  function poke() {
    showControls = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (!video?.paused && !dialog) showControls = false;
    }, 3000);
  }

  function togglePlay() {
    if (video.paused) video.play().catch(() => {});
    else video.pause();
    poke();
  }

  function seekBy(seconds: number) {
    video.currentTime = Math.max(0, Math.min(duration || Infinity, video.currentTime + seconds));
    poke();
  }

  function toggleFullscreen() {
    if (document.fullscreenElement) void document.exitFullscreen();
    else void root.requestFullscreen();
  }

  function onFullscreenChange() {
    fullscreen = !!document.fullscreenElement;
  }

  function cycleResize() {
    resize = (resize + 1) % RESIZE.length;
    showToast(RESIZE[resize].label);
  }

  function setSpeed(s: number) {
    speed = s;
    settings.speed = s;
    video.playbackRate = s;
    dialog = null;
  }

  function pickSource(link: StreamLink) {
    dialog = null;
    if (link !== current) void play(link);
  }

  function fmt(s: number) {
    if (!isFinite(s) || s < 0) s = 0;
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = Math.floor(s % 60);
    const mm = h ? String(m).padStart(2, "0") : String(m);
    return `${h ? `${h}:` : ""}${mm}:${String(sec).padStart(2, "0")}`;
  }

  function onKeydown(e: KeyboardEvent) {
    if (dialog) return;
    const key = e.key.toLowerCase();
    if (key === " " || key === "k") {
      e.preventDefault();
      togglePlay();
    } else if (key === "f") toggleFullscreen();
    else if (key === "m") video.muted = !video.muted;
    else if (key === "arrowright" || key === "l") seekBy(settings.seekSeconds);
    else if (key === "arrowleft" || key === "j") seekBy(-settings.seekSeconds);
    else if (key === "arrowup") {
      e.preventDefault();
      video.volume = Math.min(1, video.volume + 0.1);
      showToast(`Volume ${Math.round(video.volume * 100)}%`);
    } else if (key === "arrowdown") {
      e.preventDefault();
      video.volume = Math.max(0, video.volume - 0.1);
      showToast(`Volume ${Math.round(video.volume * 100)}%`);
    } else if (key === "n" && hasNext) goTo(playback.index + 1);
    else if (key === "s" && showSkip) skipIntro();
    else if (key === "p" && pipSupported) void togglePip();
    else if (key === "[") changeDelay(-0.5);
    else if (key === "]") changeDelay(0.5);
    else if (key === "escape" && countdown !== null) cancelCountdown();
    else if (key === "escape" && !document.fullscreenElement) router.back();
  }

  const progressPct = $derived(duration ? (time / duration) * 100 : 0);
  const bufferedPct = $derived(duration ? (buffered / duration) * 100 : 0);
  const loading = $derived(!links && !loadError);
  /** "Skip intro" during the first minutes of an episode-length video. */
  const showSkip = $derived(!introSkipped && duration > 900 && time > 3 && time < 480);
  /** Next-episode card during the end credits. */
  const showNext = $derived(hasNext && duration > 0 && duration - time < 40 && countdown === null);
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="player sub-{settings.subtitleBackground}"
  class:hide-cursor={!showControls}
  style:--sub-scale={settings.subtitleSize / 100}
  bind:this={root}
  role="application"
  onmousemove={poke}
  onwheel={(e) => {
    video.volume = Math.max(0, Math.min(1, video.volume - Math.sign(e.deltaY) * 0.05));
    showToast(`Volume ${Math.round(video.volume * 100)}%`);
  }}
>
  <!-- svelte-ignore a11y_media_has_caption -->
  <video
    bind:this={video}
    style:object-fit={RESIZE[resize].fit}
    crossorigin="anonymous"
    onloadedmetadata={onLoadedMetadata}
    ontimeupdate={onTimeUpdate}
    onplay={() => ((paused = false), poke())}
    onpause={() => ((paused = true), (showControls = true), persist())}
    onwaiting={() => (buffering = true)}
    onplaying={onPlaying}
    oncanplay={() => (buffering = false)}
    onended={onEnded}
    onvolumechange={() => (settings.volume = video.volume)}
    {@attach trackPip}
    onerror={() => current && (playError = "This source can't be decoded here. Try another source or mpv.")}
  >
    {#each tracks as t (t.src)}
      <track kind="subtitles" label={t.label} src={t.src} onload={applyDelay} />
    {/each}
  </video>

  {#if loading}
    <div class="loading" style:background-image={playback.poster ? `url(${JSON.stringify(thumb(playback.poster, 640))})` : undefined}>
      <div class="loading-inner">
        <div class="spinner"></div>
        <span>Loading…</span>
      </div>
    </div>
  {/if}

  <!-- Click layer: tap toggles controls, double-click toggles fullscreen (Android: tap / double-tap). -->
  <button
    class="tap"
    aria-label="Toggle controls"
    onclick={() => (showControls ? togglePlay() : poke())}
    ondblclick={toggleFullscreen}
  ></button>

  {#if loadError || playError || (links && !links.links.length)}
    <div class="message">
      <p>{loadError || playError || "No links found"}</p>
      {#if links?.links.length}
        <button class="btn btn-white" onclick={() => (dialog = "sources")}>Sources</button>
      {:else}
        <button class="btn btn-white" onclick={() => router.back()}>Go Back</button>
      {/if}
    </div>
  {/if}

  <div class="overlay" class:visible={showControls || paused}>
    <div class="top">
      <button class="icon-btn" title="Go Back" onclick={() => router.back()}><Icon name="back" size={28} /></button>
      <div class="titles">
        <span class="title">{item?.name ?? ""}</span>
        {#if entry && entry.label !== item?.name}<span class="episode">{entry.label}</span>{/if}
      </div>
      {#if current}<span class="source-name">{current.name} {quality(current)}</span>{/if}
    </div>

    {#if !loading}
      <div class="center">
        <button class="seek-btn" title="Rewind" onclick={() => seekBy(-settings.seekSeconds)}>
          <Icon name="rewind" size={60} /><span>{settings.seekSeconds}</span>
        </button>
        <button class="play-btn" title={paused ? "Play" : "Pause"} onclick={togglePlay}>
          {#if buffering && !paused}
            <div class="spinner"></div>
          {:else}
            <Icon name={paused ? "playBig" : "pauseBig"} size={110} />
          {/if}
        </button>
        <button class="seek-btn" title="Fast forward" onclick={() => seekBy(settings.seekSeconds)}>
          <Icon name="fastForward" size={60} /><span>{settings.seekSeconds}</span>
        </button>
      </div>
    {/if}

    <div class="bottom">
      <div class="timeline">
        <span class="time">{fmt(time)}</span>
        <div class="seek">
          <div class="rail-bg"></div>
          <div class="rail-buffer" style:width="{bufferedPct}%"></div>
          <div class="rail-progress" style:width="{progressPct}%"></div>
          <input
            type="range"
            min="0"
            max={duration || 0}
            step="0.1"
            value={time}
            aria-label="Seek"
            oninput={(e) => {
              video.currentTime = Number(e.currentTarget.value);
              poke();
            }}
          />
        </div>
        <span class="time">{fmt(duration)}</span>
      </div>
      <div class="buttons">
        <button class="ctl" onclick={cycleResize}><Icon name="resize" size={22} /> Resize</button>
        <button class="ctl" onclick={() => (dialog = "speed")}><Icon name="speed" size={22} /> Speed ({speed}x)</button>
        <button class="ctl" onclick={() => (dialog = "sources")} disabled={!links}><Icon name="stream" size={22} /> Sources</button>
        <button class="ctl" onclick={() => (dialog = "subtitles")}><Icon name="subtitles" size={22} /> Subtitles</button>
        {#if levels.length > 1}
          <button class="ctl" onclick={() => (dialog = "quality")}>
            <Icon name="tune" size={20} /> {level === -1 ? "Auto" : (levels.find((l) => l.index === level)?.label ?? "Quality")}
          </button>
        {/if}
        {#if audioTracks.length > 1}
          <button class="ctl" onclick={() => (dialog = "audio")}><Icon name="speed" size={20} /> Audio</button>
        {/if}
        {#if hasNext}
          <button class="ctl" onclick={() => goTo(playback.index + 1)}>
            <Icon name="skipNext" size={22} /> Next episode
          </button>
        {/if}
        <div class="spacer"></div>
        <button class="ctl" onclick={() => mpv()} disabled={!current} title="Open in mpv">mpv</button>
        {#if pipSupported}
          <button class="icon-btn" onclick={togglePip} title="Picture-in-picture (P)" class:on={pip}>
            <Icon name="resize" size={22} />
          </button>
        {/if}
        <button class="icon-btn" onclick={toggleFullscreen} title="Fullscreen (F)">
          <Icon name={fullscreen ? "fullscreenExit" : "fullscreen"} size={26} />
        </button>
      </div>
    </div>
  </div>

  {#if showSkip}
    <button class="pill-btn skip" onclick={skipIntro}><Icon name="fastForward" size={20} /> Skip intro</button>
  {/if}
  {#if showNext}
    <button class="pill-btn skip" onclick={() => goTo(playback.index + 1)}>
      <Icon name="skipNext" size={20} /> Next episode
    </button>
  {/if}
  {#if countdown !== null}
    <div class="countdown">
      <span class="gray">Up next</span>
      <strong>{playback.queue[playback.index + 1]?.label}</strong>
      <div class="countdown-actions">
        <button class="btn btn-white" onclick={() => (cancelCountdown(), goTo(playback.index + 1))}>
          <Icon name="play" size={20} /> Play now ({countdown})
        </button>
        <button class="btn btn-black" onclick={cancelCountdown}>Cancel</button>
      </div>
    </div>
  {/if}
</div>

{#if dialog === "speed"}
  <Dialog title="Player Speed" onclose={() => (dialog = null)} width={300}>
    {#each SPEEDS as s}
      <button class="list-item" onclick={() => setSpeed(s)}><span class="radio" class:on={s === speed}></span>{s}x</button>
    {/each}
  </Dialog>
{:else if dialog === "sources" && links}
  <Dialog title="Sources" onclose={() => (dialog = null)} width={560}>
    {#each links.links as l (l.url)}
      <button class="list-item" onclick={() => pickSource(l)}>
        <span class="radio" class:on={l === current}></span>
        <span class="grow">{l.name}</span>
        {#if quality(l)}<span class="gray">{quality(l)}</span>{/if}
        {#if !inApp(l)}<span class="badge">{l.type}</span>{/if}
      </button>
    {/each}
  </Dialog>
{:else if dialog === "quality"}
  <Dialog title="Quality" onclose={() => (dialog = null)} width={300}>
    <button class="list-item" onclick={() => setLevel(-1)}><span class="radio" class:on={level === -1}></span>Auto</button>
    {#each levels as l (l.index)}
      <button class="list-item" onclick={() => setLevel(l.index)}>
        <span class="radio" class:on={level === l.index}></span>{l.label}
      </button>
    {/each}
  </Dialog>
{:else if dialog === "audio"}
  <Dialog title="Audio" onclose={() => (dialog = null)} width={320}>
    {#each audioTracks as t (t.index)}
      <button class="list-item" onclick={() => setAudio(t.index)}>
        <span class="radio" class:on={audioTrack === t.index}></span>{t.label}
      </button>
    {/each}
  </Dialog>
{:else if dialog === "subtitles"}
  <Dialog title="Subtitles" onclose={() => (dialog = null)} width={620}>
    <div class="sources">
      <div class="col">
        <button class="list-item" onclick={() => selectSubtitle(-1)}>
          <span class="radio" class:on={subtitle === -1}></span>No Subtitles
        </button>
        {#each tracks as t, i (t.src)}
          <button class="list-item" onclick={() => selectSubtitle(i)}>
            <span class="radio" class:on={subtitle === i}></span>{t.label}
          </button>
        {:else}
          <p class="gray pad">This source has no subtitles.</p>
        {/each}
      </div>
      <div class="col style">
        <h3>Size</h3>
        <div class="seg">
          {#each [75, 100, 125, 150, 200] as size}
            <button class:on={settings.subtitleSize === size} onclick={() => (settings.subtitleSize = size)}>{size}%</button>
          {/each}
        </div>
        <h3>Style</h3>
        <div class="seg">
          {#each [["outline", "Outline"], ["shadow", "Shadow"], ["box", "Box"]] as [id, name]}
            <button
              class:on={settings.subtitleBackground === id}
              onclick={() => (settings.subtitleBackground = id as typeof settings.subtitleBackground)}>{name}</button
            >
          {/each}
        </div>
        <h3>Delay</h3>
        <div class="seg">
          <button onclick={() => changeDelay(-0.5)}>−0.5 s</button>
          <span class="delay">{subDelay > 0 ? "+" : ""}{subDelay.toFixed(1)} s</span>
          <button onclick={() => changeDelay(0.5)}>+0.5 s</button>
        </div>
      </div>
    </div>
  </Dialog>
{/if}

<style>
  .player {
    position: absolute;
    inset: 0;
    background: #000;
    overflow: hidden;
  }

  .hide-cursor {
    cursor: none;
  }

  video {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    background: #000;
  }

  video::cue {
    background: transparent;
    color: #fff;
    font-family: inherit;
    font-size: calc(var(--sub-scale, 1) * 1.15em);
    text-shadow:
      0 0 3px #000,
      0 0 3px #000,
      1px 1px 2px #000;
  }

  .sub-shadow video::cue {
    text-shadow: 2px 2px 6px rgba(0, 0, 0, 0.95);
  }

  .sub-box video::cue {
    background: rgba(0, 0, 0, 0.75);
    text-shadow: none;
  }

  .pill-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 20px;
    border-radius: 6px;
    background: rgba(20, 20, 20, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.6);
    color: #fff;
    font-weight: 700;
    font-size: 15px;
  }

  .pill-btn:hover {
    background: var(--text);
    color: var(--icon-gray);
  }

  .skip {
    position: absolute;
    right: 32px;
    bottom: 120px;
    z-index: 3;
  }

  .countdown {
    position: absolute;
    right: 32px;
    bottom: 120px;
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 300px;
    padding: 18px 20px;
    border-radius: 12px;
    background: rgba(20, 20, 20, 0.92);
    color: #fff;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.6);
  }

  .countdown-actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }

  .icon-btn.on {
    color: var(--primary);
  }

  .style h3 {
    padding: 4px 16px 8px;
  }

  .seg {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 0 16px 14px;
  }

  .seg button {
    height: 34px;
    padding: 0 12px;
    border-radius: 8px;
    background: var(--bg);
    border: 1px solid rgba(255, 255, 255, 0.12);
    font-size: 13px;
    font-weight: 700;
  }

  .seg button.on {
    background: var(--primary);
    border-color: var(--primary);
    color: #fff;
  }

  .delay {
    min-width: 64px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .pad {
    padding: 8px 16px;
  }

  .loading {
    position: absolute;
    inset: 0;
    background-size: cover;
    background-position: center;
  }

  .loading-inner {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    background: rgba(0, 0, 0, 0.75);
    color: var(--text);
  }

  .tap {
    position: absolute;
    inset: 0;
    cursor: inherit;
  }

  .message {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding: 40px;
    text-align: center;
    color: var(--text);
    pointer-events: none;
  }

  .message .btn {
    pointer-events: auto;
  }

  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    opacity: 0;
    transition: opacity 0.25s;
    pointer-events: none;
  }

  .overlay.visible {
    opacity: 1;
    background: rgba(0, 0, 0, 0.4); /* black_overlay */
  }

  .overlay.visible > * {
    pointer-events: auto;
  }

  .top {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px 40px;
    background: linear-gradient(to bottom, rgba(0, 0, 0, 0.75), transparent);
    color: #fff;
  }

  .titles {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .title {
    font-size: 17px;
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .episode {
    font-size: 14px;
    color: var(--text-gray);
  }

  .source-name {
    font-size: 13px;
    color: var(--text-gray);
  }

  .center {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    align-items: center;
    gap: 64px;
    color: #fff;
  }

  .seek-btn {
    position: relative;
    display: grid;
    place-items: center;
    color: #fff;
  }

  .seek-btn span {
    position: absolute;
    top: 58%;
    left: 50%;
    transform: translate(-50%, -50%);
    font-size: 15px;
    font-weight: 700;
  }

  .play-btn {
    display: grid;
    place-items: center;
    width: 110px;
    height: 110px;
    color: #fff;
  }

  .bottom {
    padding: 40px 16px 10px;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.8), transparent);
  }

  .timeline {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .time {
    min-width: 48px;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    color: #fff;
    text-align: center;
  }

  .seek {
    position: relative;
    flex: 1;
    height: 20px;
  }

  .rail-bg,
  .rail-buffer,
  .rail-progress {
    position: absolute;
    left: 0;
    top: 8px;
    height: 4px;
    border-radius: 2px;
  }

  .rail-bg {
    right: 0;
    background: rgba(255, 255, 255, 0.2);
  }

  .rail-buffer {
    background: rgba(181, 181, 181, 0.4);
  }

  .rail-progress {
    background: var(--primary);
  }

  .seek input {
    position: absolute;
    inset: 0;
    width: 100%;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }

  .seek:hover .rail-progress::after {
    content: "";
    position: absolute;
    right: -7px;
    top: -5px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--primary);
  }

  .buttons {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
  }

  .ctl {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 12px;
    border-radius: 6px;
    color: #fff;
    font-weight: 700;
    font-size: 14px;
  }

  .ctl:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .spacer {
    flex: 1;
  }

  .sources {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding: 12px 4px 0;
  }

  .col {
    max-height: 60vh;
    overflow-y: auto;
  }

  .col h3 {
    font-size: 18px;
    font-weight: 700;
    padding: 6px 16px 10px;
  }

  .grow {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
