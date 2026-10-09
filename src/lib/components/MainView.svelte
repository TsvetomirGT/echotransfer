<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, formatBytes, type Outcome, type Progress } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import ArtistList from "./ArtistList.svelte";
  import AlbumPane from "./AlbumPane.svelte";
  import CardStrip from "./CardStrip.svelte";
  import SyncPanel from "./SyncPanel.svelte";
  import SettingsSheet from "./SettingsSheet.svelte";

  let focused = $state<string | null>(null);
  let settingsOpen = $state(false);
  let progress = $state<Progress | null>(null);
  let outcome = $state<Outcome | null>(null);
  let syncError = $state<string | null>(null);
  let panelOpen = $state(false);

  const focusedArtist = $derived(app.artists.find((a) => a.id === focused) ?? null);

  $effect(() => {
    if (!focused && app.artists.length) focused = app.artists[0].id;
  });

  const host = $derived(app.server ? new URL(app.server.url).host : "");
  const canSync = $derived(
    !!app.volume && !!app.diff && !app.syncing && !app.diffLoading && app.diff.projectedFree >= 0 &&
      app.diff.summary.new + app.diff.summary.updated + app.diff.summary.removed > 0,
  );

  onMount(() => {
    const subs = [
      api.onProgress((p) => (progress = p)),
      api.onDone((d) => {
        app.syncing = false;
        outcome = d.outcome;
        syncError = d.error;
        void app.refreshDiff();
      }),
    ];
    return () => subs.forEach((s) => void s.then((f) => f()));
  });

  async function sync() {
    if (!app.mount) return;
    outcome = null;
    syncError = null;
    progress = null;
    panelOpen = true;
    app.syncing = true;
    try {
      await api.startSync(app.mount, [...app.selection]);
    } catch (e) {
      app.syncing = false;
      syncError = errorText(e);
    }
  }

  function dismiss() {
    panelOpen = false;
    outcome = null;
    syncError = null;
  }
</script>

<div class="shell">
  <header data-tauri-drag-region>
    <div class="server" data-tauri-drag-region>
      <span class="led" class:on={!app.libraryError}></span>
      <span class="host" title={app.server?.server}>{host}</span>
      <button class="ghost icon" title="Reload library from server" aria-label="Reload library" disabled={app.libraryLoading || app.syncing} onclick={() => app.loadLibrary(true)}>
        <svg viewBox="0 0 16 16" class:spin={app.libraryLoading}><path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2.5v3h-3" /></svg>
      </button>
    </div>
    <div class="spacer" data-tauri-drag-region></div>
    <label class="card">
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 1.5h6l3.5 3.5v9a.5.5 0 0 1-.5.5H4a.5.5 0 0 1-.5-.5V2a.5.5 0 0 1 .5-.5zM6 3.5v2.5M8 3.5v2.5M10 3.5v2.5" /></svg>
      <select
        aria-label="SD card"
        disabled={app.syncing}
        value={app.mount ?? ""}
        onchange={(e) => app.selectVolume(e.currentTarget.value || null)}
      >
        <option value="">{app.volumes.length ? "Choose card" : "No card found"}</option>
        {#each app.volumes as v (v.mountPoint)}
          <option value={v.mountPoint}>{v.name} ({formatBytes(v.availableBytes)} free of {formatBytes(v.totalBytes)})</option>
        {/each}
      </select>
    </label>
    <button class="ghost icon" aria-label="Settings" title="Settings" disabled={app.syncing} onclick={() => (settingsOpen = true)}>
      <svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="2.2" /><path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" /></svg>
    </button>
    <button class="primary" disabled={!canSync} onclick={sync}>{app.syncing ? "Syncing…" : "Sync"}</button>
  </header>

  <aside>
    {#if app.libraryError}
      <div class="lib-error">
        <p>Couldn't load the library: {app.libraryError}</p>
        <button class="secondary" onclick={() => app.loadLibrary(true)}>Try again</button>
      </div>
    {:else}
      <ArtistList bind:focused />
    {/if}
  </aside>

  <main>
    <AlbumPane artist={focusedArtist} />
  </main>

  <footer>
    {#if panelOpen}
      <SyncPanel {progress} {outcome} error={syncError} ondismiss={dismiss} />
    {:else}
      <CardStrip />
    {/if}
  </footer>
</div>

{#if settingsOpen}
  <SettingsSheet onclose={() => (settingsOpen = false)} />
{/if}

<style>
  .shell {
    height: 100vh;
    display: grid;
    grid-template-columns: 260px 1fr;
    grid-template-rows: 52px 1fr auto;
    grid-template-areas: "head head" "side main" "foot foot";
  }
  header {
    grid-area: head;
    display: flex; align-items: center; gap: 8px;
    padding: 0 14px 0 84px; /* room for traffic lights */
    border-bottom: 1px solid var(--line);
  }
  .server { display: flex; align-items: center; gap: 7px; min-width: 0; }
  .led { width: 7px; height: 7px; border-radius: 50%; background: var(--removed); flex: none; }
  .led.on { background: var(--new); }
  .host { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .spacer { flex: 1; align-self: stretch; }
  .icon { width: 28px; height: 28px; padding: 0; display: grid; place-items: center; }
  .icon svg, .card svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; stroke-linejoin: round; }
  .spin { animation: spin 0.9s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .card {
    display: flex; align-items: center; gap: 6px; height: 28px; padding: 0 4px 0 9px;
    border-radius: 6px; background: var(--active); color: var(--muted); max-width: 340px;
  }
  .card select {
    border: 0; background: transparent; color: var(--ink); height: 26px; min-width: 0; max-width: 300px; outline-offset: 0;
  }
  aside {
    grid-area: side; background: var(--sidebar); border-right: 1px solid var(--line);
    display: flex; flex-direction: column; min-height: 0; padding-top: 12px;
  }
  .lib-error { padding: 12px; color: var(--muted); }
  main { grid-area: main; overflow-y: auto; min-height: 0; background: var(--surface); }
  footer { grid-area: foot; border-top: 1px solid var(--line); background: var(--bg); }
</style>
