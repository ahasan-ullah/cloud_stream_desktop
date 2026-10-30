<script lang="ts">
  // Android res/layout/home_result_grid.xml: 114x180dp card, 10dp corners, title over a bottom shadow,
  // quality badge top-left, dub/sub badges top-right, optional circular watch progress.
  import type { SearchItem } from "../lib/api";
  import { lazySrc, thumb } from "../lib/images";
  import { qualityLabel } from "../lib/tvtypes";
  import Icon from "./Icon.svelte";

  let {
    item,
    width = 114,
    horizontal = false,
    progress,
    episodeText,
    onselect,
    onremove,
  }: {
    item: SearchItem;
    width?: number;
    horizontal?: boolean;
    progress?: number;
    episodeText?: string;
    onselect: (item: SearchItem) => void;
    onremove?: () => void;
  } = $props();

  let failed = $state(false);
  const height = $derived(horizontal ? Math.round((width * 9) / 16) : Math.round((width * 180) / 114));
  const dub = $derived(item.dubStatus?.includes("Dubbed"));
  const sub = $derived(item.dubStatus?.includes("Subbed"));
  const RING = 2 * Math.PI * 23;
</script>

<div class="card" style:width="{width}px" style:height="{height}px">
  <button class="hit" onclick={() => onselect(item)} title={item.name}>
    {#if item.posterUrl && !failed}
      <img
        {@attach lazySrc(thumb(item.posterUrl, width))}
        alt=""
        decoding="async"
        referrerpolicy="no-referrer"
        onerror={() => (failed = true)}
      />
    {/if}
    <div class="shadow"></div>
    <div class="bottom">
      {#if episodeText}<span class="badge episode">{episodeText}</span>{/if}
      <span class="title">{item.name}</span>
    </div>
    {#if item.quality}<span class="badge quality">{qualityLabel(item.quality)}</span>{/if}
    {#if dub || sub}
      <span class="flags">
        {#if dub}<span class="badge dub">Dub</span>{/if}
        {#if sub}<span class="badge sub">Sub</span>{/if}
      </span>
    {/if}
    {#if progress != null}
      <span class="progress">
        <svg viewBox="0 0 50 50" width="50" height="50">
          <circle cx="25" cy="25" r="25" fill="rgba(0,0,0,0.7)" />
          <circle cx="25" cy="25" r="23" fill="none" stroke="rgba(255,255,255,0.1)" stroke-width="2" />
          <circle
            cx="25"
            cy="25"
            r="23"
            fill="none"
            stroke="var(--primary)"
            stroke-width="2"
            stroke-dasharray="{RING * progress} {RING}"
            transform="rotate(-90 25 25)"
          />
        </svg>
        <span class="play"><Icon name="play" size={26} /></span>
      </span>
    {/if}
  </button>
  {#if onremove}
    <button class="remove" title="Remove" onclick={onremove}><Icon name="close" size={16} /></button>
  {/if}
</div>

<style>
  .card {
    position: relative;
    flex-shrink: 0;
    margin: 2px;
  }

  .hit {
    position: relative;
    display: block;
    width: 100%;
    height: 100%;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--bg-gray);
    box-shadow: 0 4px 10px rgba(0, 0, 0, 0.35);
    outline: 2px solid transparent;
    outline-offset: -2px;
    transition: outline-color 0.12s;
  }

  .hit:hover,
  .hit:focus-visible {
    outline-color: var(--text);
  }

  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  img:not([src]) {
    visibility: hidden;
  }

  .shadow {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 50px;
    background: linear-gradient(to bottom, transparent, #000);
  }

  .bottom {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 0 5px 5px;
  }

  .title {
    width: 100%;
    color: var(--text);
    font-weight: 700;
    font-size: 14px;
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badge {
    margin: 1px;
  }

  .quality {
    position: absolute;
    top: 0;
    left: 0;
  }

  .flags {
    position: absolute;
    top: 0;
    right: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }

  .episode {
    min-width: 0;
  }

  .progress {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 50px;
    height: 50px;
    transform: translate(-50%, -50%);
  }

  .progress svg {
    position: absolute;
    inset: 0;
  }

  .play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    padding-left: 3px;
    color: #fff;
  }

  .remove {
    position: absolute;
    top: -6px;
    right: -6px;
    display: none;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--bg-gray);
    color: var(--text);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.5);
  }

  .card:hover .remove {
    display: grid;
  }
</style>
