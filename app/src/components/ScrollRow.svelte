<script lang="ts">
  // Horizontal row of cards. Mouse users get previous/next arrows (a hidden-scrollbar row cannot be
  // scrolled with a vertical wheel), plus a thin scrollbar on hover; touchpads scroll natively.
  import { onMount, type Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let { children, inset = 16 }: { children: Snippet; inset?: number } = $props();

  let row: HTMLDivElement;
  let canLeft = $state(false);
  let canRight = $state(false);

  function update() {
    canLeft = row.scrollLeft > 4;
    canRight = row.scrollLeft + row.clientWidth < row.scrollWidth - 4;
  }

  function page(direction: number) {
    row.scrollBy({ left: direction * row.clientWidth * 0.85, behavior: "smooth" });
  }

  onMount(() => {
    update();
    const resize = new ResizeObserver(update);
    resize.observe(row);
    const mutation = new MutationObserver(update);
    mutation.observe(row, { childList: true });
    return () => {
      resize.disconnect();
      mutation.disconnect();
    };
  });
</script>

<div class="wrap">
  <div class="row" bind:this={row} onscroll={update} style:padding-inline="{inset}px" style:scroll-padding-inline="{inset}px">
    {@render children()}
  </div>
  {#if canLeft}
    <button class="arrow left" title="Previous" onclick={() => page(-1)}><Icon name="back" size={22} /></button>
  {/if}
  {#if canRight}
    <button class="arrow right" title="Next" onclick={() => page(1)}><Icon name="forward" size={22} /></button>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }

  .row {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    overflow-y: hidden;
    padding-top: 2px;
    padding-bottom: 8px;
    scrollbar-width: thin;
    scrollbar-color: transparent transparent;
  }

  .wrap:hover .row {
    scrollbar-color: rgba(255, 255, 255, 0.25) transparent;
  }

  .arrow {
    position: absolute;
    top: calc(50% - 22px);
    z-index: 2;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: rgba(17, 17, 17, 0.85);
    color: var(--text);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.6);
    opacity: 0.75;
    transition: opacity 0.15s, background 0.15s;
  }

  .wrap:hover .arrow {
    opacity: 1;
  }

  .arrow:hover {
    background: var(--primary);
  }

  .left {
    left: 6px;
  }

  .right {
    right: 6px;
  }
</style>
