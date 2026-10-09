<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import ConnectView from "$lib/components/ConnectView.svelte";
  import MainView from "$lib/components/MainView.svelte";

  let booting = $state(true);

  onMount(() => {
    const unlisten = api.onVolumes((v) => app.setVolumes(v));
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

<style>
  .boot { height: 100vh; }
</style>
