<script lang="ts">
  // Android res/layout/main_settings.xml
  import Icon, { type IconName } from "../components/Icon.svelte";
  import Logo from "../components/Logo.svelte";
  import { router, type Route } from "../lib/router.svelte";
  import { engineState } from "../lib/state.svelte";

  // Same categories as Android's main_settings.xml.
  const items: { label: string; icon: IconName; route: Route }[] = [
    { label: "General", icon: "settings", route: { name: "settings-general" } },
    { label: "Player", icon: "play", route: { name: "settings-player" } },
    { label: "Providers", icon: "public", route: { name: "settings-providers" } },
    { label: "Layout", icon: "resize", route: { name: "settings-layout" } },
    { label: "Updates and Backup", icon: "autorenew", route: { name: "settings-backup" } },
    { label: "Extensions", icon: "extension", route: { name: "extensions" } },
  ];

  const status = $derived(engineState.status);
</script>

<div class="page settings">
  <div class="profile">
    <Logo size={44} />
    <div>
      <div class="app">CloudStream</div>
      <div class="gray small">Desktop</div>
    </div>
  </div>

  <nav>
    {#each items as item (item.label)}
      <button class="settings-item" onclick={() => router.push(item.route)}>
        <Icon name={item.icon} size={22} />
        <span>{item.label}</span>
        <Icon name="chevronRight" size={24} />
      </button>
    {/each}
  </nav>

  <p class="version gray">
    v0.1.2 • engine {status.state}{status.state === "ready" ? ` • ${engineState.providers.length} providers` : ""}
  </p>
</div>

<style>
  .settings {
    padding: 10px 0;
  }

  .profile {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 18px 20px 22px;
  }

  .app {
    font-size: 18px;
    font-weight: 700;
  }

  .small {
    font-size: 13px;
  }

  nav {
    display: flex;
    flex-direction: column;
  }

  /* SettingsItem: 50dp rows, 20dp padding, chevron on the right */
  .settings-item {
    display: flex;
    align-items: center;
    gap: 18px;
    height: 50px;
    padding: 0 20px;
    color: var(--text);
    font-size: 15px;
    text-align: left;
  }

  .settings-item span {
    flex: 1;
  }

  .settings-item :global(svg:first-child) {
    color: var(--icon);
  }

  .settings-item:hover {
    background: var(--ripple);
  }

  .version {
    padding: 24px 20px;
    font-size: 12px;
    text-align: center;
  }
</style>
