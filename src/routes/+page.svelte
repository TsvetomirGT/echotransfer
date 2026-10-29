<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import ConnectView from "$lib/components/ConnectView.svelte";
  import MainView from "$lib/components/MainView.svelte";
  import UpdateBanner from "$lib/components/UpdateBanner.svelte";
  import { updater } from "$lib/updater.svelte";

  let booting = $state(true);

  onMount(() => {
    const unlisten = api.onVolumes((v) => app.setVolumes(v));
    void updater.check(true);
    (async () => {
      app.settings = await api.getSettings();
      app.setVolumes(await api.listVolumes());
      try {
        app.server = await api.autoConnect();
      } catch {
        app.server = null;
      }
      booting = false;
      if (app.server) void app.loadLibrary();
    })();
    return () => void unlisten.then((f) => f());
  });
</script>

{#if booting}
  <div class="boot" data-tauri-drag-region></div>
{:else if app.server}
  <MainView />
{:else}
  <ConnectView />
{/if}

<UpdateBanner />

<style>
  .boot { height: 100vh; }
</style>
