<script lang="ts">
  // Android AlertDialog / bottom sheet stand-in.
  import type { Snippet } from "svelte";

  let {
    title,
    onclose,
    width = 420,
    children,
    footer,
  }: { title?: string; onclose: () => void; width?: number; children: Snippet; footer?: Snippet } = $props();

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window onkeydowncapture={onKeydown} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={title} style:width="{width}px">
    {#if title}<h2>{title}</h2>{/if}
    <div class="body" data-scroll-root>{@render children()}</div>
    {#if footer}<div class="footer">{@render footer()}</div>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.6);
    animation: fade 0.15s ease-out;
  }

  .dialog {
    max-width: calc(100vw - 40px);
    max-height: calc(100vh - 60px);
    display: flex;
    flex-direction: column;
    background: var(--bg-gray);
    border-radius: 16px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.6);
    animation: rise 0.18s ease-out;
  }

  h2 {
    font-size: 20px;
    font-weight: 700;
    padding: 20px 24px 12px;
  }

  .body {
    overflow-y: auto;
    padding: 0 8px 8px;
  }

  .footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 8px 16px 16px;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @keyframes rise {
    from {
      transform: translateY(12px);
      opacity: 0;
    }
  }
</style>
