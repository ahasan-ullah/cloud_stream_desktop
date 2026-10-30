<script lang="ts">
  import { onMount, untrack } from "svelte";
  import Dialog from "./components/Dialog.svelte";
  import Icon, { type IconName } from "./components/Icon.svelte";
  import Logo from "./components/Logo.svelte";
  import { engine, errorMessage } from "./lib/api";
  import { initDownloads } from "./lib/downloads.svelte";
  import { refreshRepos, updatePlugins } from "./lib/extensions.svelte";
  import { router, type RootName, type SettingsPageName } from "./lib/router.svelte";
  import {
    engineState,
    initEngine,
    persistSettings,
    settings,
    showToast,
    toast,
    viewport,
  } from "./lib/state.svelte";
  import Details from "./views/Details.svelte";
  import Downloads from "./views/Downloads.svelte";
  import Extensions from "./views/Extensions.svelte";
  import Home from "./views/Home.svelte";
  import Library from "./views/Library.svelte";
  import Player from "./views/Player.svelte";
  import Plugins from "./views/Plugins.svelte";
  import Search from "./views/Search.svelte";
  import Settings from "./views/Settings.svelte";
  import SettingsPage from "./views/SettingsPage.svelte";

  // res/menu/bottom_nav_menu.xml, shown as a NavigationRailView (unlabeled, centered).
  const nav: { name: RootName; label: string; icon: IconName; active: IconName }[] = [
    { name: "home", label: "Home", icon: "home", active: "homeFilled" },
    { name: "search", label: "Search", icon: "search", active: "search" },
    { name: "library", label: "Library", icon: "library", active: "libraryFilled" },
    { name: "downloads", label: "Downloads", icon: "download", active: "download" },
    { name: "settings", label: "Settings", icon: "settings", active: "settingsFilled" },
  ];

  let initError = $state("");
  let shortcuts = $state(false);
  let pluginsChecked = false;

  onMount(() => {
    initEngine().catch((e) => (initError = errorMessage(e)));
    initDownloads().catch(() => {});
  });

  $effect(() => persistSettings(JSON.stringify(settings)));

  // Accent color (Settings → Layout) drives every primary-colored element.
  $effect(() => {
    document.documentElement.style.setProperty("--primary", settings.accent);
  });

  // Like Android: update plugins from their repositories once per launch.
  $effect(() => {
    if (engineState.status.state !== "ready" || pluginsChecked || !settings.autoUpdatePlugins) return;
    pluginsChecked = true;
    untrack(async () => {
      try {
        await refreshRepos();
        const updated = await updatePlugins();
        if (updated) showToast(`Updated ${updated} plugin${updated === 1 ? "" : "s"}`);
      } catch {
        // Offline or a repository is down: try again next launch.
      }
    });
  });

  function onKeydown(e: KeyboardEvent) {
    if (router.current.name === "player") return;
    const typing = (e.target as HTMLElement).closest?.("input, textarea, select");
    if (e.key === "?" && !typing) {
      shortcuts = !shortcuts;
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
      e.preventDefault();
      router.root("search");
    } else if (e.key === "Escape" && router.canGoBack) {
      router.back();
    } else if (e.key === "BrowserBack" || (e.altKey && e.key === "ArrowLeft")) {
      router.back();
    }
  }

  function onMouseUp(e: MouseEvent) {
    // Mouse "back" side button behaves like Android's back gesture.
    if (e.button === 3 && router.current.name !== "player") router.back();
  }

  const status = $derived(engineState.status);
  const route = $derived(router.current);
</script>

<svelte:window onkeydown={onKeydown} onmouseup={onMouseUp} bind:innerWidth={viewport.width} />

<div class="shell">
  {#if route.name !== "player"}
    <nav class="rail">
      <div class="logo"><Logo size={36} /></div>
      <div class="items">
        {#each nav as item (item.name)}
          {@const active = router.rootName === item.name}
          <button class="rail-item" class:active onclick={() => router.root(item.name)} title={item.label}>
            <span class="pill"><Icon name={active ? item.active : item.icon} size={24} /></span>
          </button>
        {/each}
      </div>
    </nav>
  {/if}

  <main>
    {#if route.name !== "player"}
      {#if status.state === "failed" || initError}
        <div class="banner">
          <span>CloudStream engine unavailable: {initError || (status.state === "failed" ? status.message : "")}</span>
          <button class="btn btn-white" onclick={() => engine.restart()}>Retry connection…</button>
        </div>
      {:else if status.state === "starting"}
        <div class="banner subtle"><div class="spinner small"></div> Starting CloudStream engine…</div>
      {/if}
    {/if}

    <div class="view">
      {#key route}
        <!-- One failing screen (e.g. odd plugin data) shows an error instead of freezing the app. -->
        <svelte:boundary onerror={(e) => console.error(e)}>
          {#if route.name === "home"}
            <Home />
          {:else if route.name === "search"}
            <Search initialQuery={route.query} />
          {:else if route.name === "library"}
            <Library />
          {:else if route.name === "downloads"}
            <Downloads />
          {:else if route.name === "settings"}
            <Settings />
          {:else if route.name.startsWith("settings-")}
            <SettingsPage page={route.name as SettingsPageName} />
          {:else if route.name === "extensions"}
            <Extensions />
          {:else if route.name === "plugins"}
            <Plugins repo={route.repo} />
          {:else if route.name === "details"}
            <Details provider={route.provider} url={route.url} title={route.title} autoplay={route.autoplay} />
          {:else if route.name === "player"}
            <Player />
          {/if}
          {#snippet failed(error, reset)}
            <div class="center-fill crashed">
              <p class="error-text">This screen ran into a problem: {errorMessage(error)}</p>
              <button class="btn btn-white" onclick={reset}><Icon name="autorenew" size={20} /> Retry</button>
              {#if router.canGoBack}
                <button class="btn btn-black" onclick={() => router.back()}>Go Back</button>
              {/if}
            </div>
          {/snippet}
        </svelte:boundary>
      {/key}
    </div>
  </main>

  {#if toast.text}
    {#key toast.id}<div class="toast">{toast.text}</div>{/key}
  {/if}
</div>

{#if shortcuts}
  <Dialog title="Keyboard shortcuts" onclose={() => (shortcuts = false)} width={460}>
    <dl class="keys">
      <dt>Ctrl/⌘ + F</dt><dd>Search</dd>
      <dt>Esc · Alt + ← · mouse back</dt><dd>Back</dd>
      <dt>?</dt><dd>This list</dd>
      <dt>Space / K</dt><dd>Play / pause</dd>
      <dt>← / →  ·  J / L</dt><dd>Rewind / fast forward</dd>
      <dt>↑ / ↓  ·  wheel</dt><dd>Volume</dd>
      <dt>M</dt><dd>Mute</dd>
      <dt>F · double-click</dt><dd>Fullscreen</dd>
      <dt>S</dt><dd>Skip intro</dd>
      <dt>N</dt><dd>Next episode</dd>
      <dt>P</dt><dd>Picture-in-picture</dd>
      <dt>[ / ]</dt><dd>Subtitle delay −/+ 0.5 s</dd>
    </dl>
  </Dialog>
{/if}

<style>
  .shell {
    display: flex;
    height: 100%;
  }

  .rail {
    width: var(--rail);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    background: var(--bg-gray);
  }

  .logo {
    padding: 18px 0 0;
    height: 64px;
  }

  .items {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 14px;
    padding-bottom: 64px;
  }

  .rail-item {
    display: grid;
    place-items: center;
    width: 62px;
    height: 44px;
    color: var(--text);
  }

  .pill {
    display: grid;
    place-items: center;
    width: 52px;
    height: 32px;
    border-radius: 16px;
    transition: background 0.15s;
  }

  .rail-item:hover .pill {
    background: var(--ripple);
  }

  .rail-item.active {
    color: var(--primary);
  }

  .rail-item.active .pill {
    background: rgba(61, 80, 250, 0.16);
  }

  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }

  .view {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  .crashed {
    height: 100%;
  }

  .keys {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px 20px;
    margin: 0;
    padding: 4px 16px 12px;
  }

  .keys dt {
    color: var(--text-gray);
    font-family: ui-monospace, monospace;
    font-size: 13px;
  }

  .keys dd {
    margin: 0;
  }

  .banner {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    background: rgba(234, 89, 110, 0.12);
    color: var(--fail);
    font-weight: 500;
  }

  .banner span {
    flex: 1;
  }

  .banner .btn {
    height: 32px;
  }

  .banner.subtle {
    background: var(--bg-gray);
    color: var(--text-gray);
  }

  .toast {
    position: fixed;
    left: 50%;
    bottom: 48px;
    z-index: 100;
    transform: translateX(-50%);
    max-width: 80vw;
    padding: 10px 18px;
    border-radius: 22px;
    background: rgba(50, 50, 50, 0.95);
    color: #fff;
    font-size: 14px;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4);
    animation: toast 0.2s ease-out;
    pointer-events: none;
  }

  @keyframes toast {
    from {
      opacity: 0;
      transform: translate(-50%, 8px);
    }
  }
</style>
