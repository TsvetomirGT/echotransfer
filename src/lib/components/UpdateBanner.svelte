<script lang="ts">
  import { formatBytes } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import { updater } from "$lib/updater.svelte";

  const u = $derived(updater.available);
  const show = $derived(!!u && !updater.dismissed && (updater.status === "available" || updater.status === "downloading" || (updater.status === "error" && !!updater.available)));
  const pct = $derived(updater.total ? Math.min(100, (updater.downloaded / updater.total) * 100) : 0);
</script>

{#if show && u}
  <div class="banner" role="status">
    {#if updater.status === "downloading"}
      <span class="text">
        Downloading EchoTransfer {u.version}…
        {#if updater.total}<span class="num muted">{formatBytes(updater.downloaded)} of {formatBytes(updater.total)}</span>{/if}
      </span>
      <div class="bar"><span style:width={`${pct}%`}></span></div>
    {:else}
      <span class="text">
        {#if updater.status === "error"}
          <span class="error">Update failed: {updater.error}</span>
        {:else}
          EchoTransfer {u.version} is available <span class="muted">(you have {updater.current})</span>
        {/if}
      </span>
      <button
        class="primary"
        disabled={app.syncing}
        title={app.syncing ? "Wait for the sync to finish" : updater.canSelfUpdate ? "Download, install and restart" : "Open the release page"}
        onclick={() => updater.install()}
      >
        {updater.canSelfUpdate ? (updater.status === "error" ? "Try again" : "Update") : "Download"}
      </button>
      <button class="ghost close" aria-label="Dismiss" onclick={() => (updater.dismissed = true)}>×</button>
    {/if}
  </div>
{/if}

<style>
  .banner {
    position: fixed; z-index: 9; top: 60px; left: 50%; transform: translateX(-50%);
    display: flex; align-items: center; gap: 10px; min-width: 340px; max-width: calc(100vw - 32px);
    padding: 8px 8px 8px 14px; border-radius: var(--radius);
    background: var(--surface); box-shadow: 0 10px 30px rgba(0, 0, 0, 0.18), 0 0 0 1px var(--line);
    animation: drop 0.18s ease-out;
  }
  @keyframes drop { from { opacity: 0; transform: translate(-50%, -6px); } }
  .text { flex: 1; min-width: 0; }
  .muted { color: var(--muted); }
  .error { color: var(--removed); }
  .close { padding: 0 8px; font-size: 16px; }
  .bar { width: 120px; height: 4px; border-radius: 2px; background: var(--active); overflow: hidden; margin-right: 6px; }
  .bar span { display: block; height: 100%; background: var(--focus); transition: width 0.2s; }
</style>
