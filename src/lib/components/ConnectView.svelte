<script lang="ts">
  import { api, errorText } from "$lib/api";
  import { app } from "$lib/state.svelte";

  let url = $state(app.settings?.serverUrl ?? "");
  let username = $state(app.settings?.username ?? "");
  let password = $state("");
  let remember = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = null;
    try {
      app.server = await api.connect(url, username, password, remember);
      app.settings = await api.getSettings();
      void app.loadLibrary(true);
    } catch (err) {
      const msg = errorText(err);
      error = msg.startsWith("Server error 40")
        ? "Wrong username or password."
        : msg.startsWith("Network error")
          ? "Can't reach the server. Check the address and that Navidrome is running."
          : msg;
    } finally {
      busy = false;
    }
  }
</script>

<div class="wrap" data-tauri-drag-region>
  <form onsubmit={submit}>
    <svg class="card" viewBox="0 0 48 60" aria-hidden="true">
      <path d="M4 2h28l14 14v40a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z" />
      <g><rect x="10" y="6" width="4" height="12" rx="1" /><rect x="17" y="6" width="4" height="12" rx="1" /><rect x="24" y="6" width="4" height="12" rx="1" /></g>
    </svg>
    <h1>Connect to Navidrome</h1>
    <p class="lede">Pick music from your server and copy it to your Echo's SD card, with cover art the player can show.</p>

    <label>
      <span>Server address</span>
      <input class="field" bind:value={url} placeholder="music.example.com or 192.168.1.10:4533" required autocomplete="url" spellcheck="false" />
    </label>
    <label>
      <span>Username</span>
      <input class="field" bind:value={username} required autocomplete="username" spellcheck="false" />
    </label>
    <label>
      <span>Password</span>
      <input class="field" type="password" bind:value={password} required autocomplete="current-password" />
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={remember} />
      <span>Remember password in Keychain</span>
    </label>

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <button class="primary" type="submit" disabled={busy}>{busy ? "Connecting…" : "Connect"}</button>
  </form>
</div>

<style>
  .wrap {
    height: 100vh;
    display: grid;
    place-items: center;
    padding: 24px;
  }
  form {
    width: 340px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .card {
    width: 34px;
    fill: none;
    stroke: var(--ink);
    stroke-width: 2;
    margin-bottom: 6px;
  }
  .card g rect { fill: var(--ink); stroke: none; }
  h1 { font-size: 22px; font-weight: 650; letter-spacing: -0.01em; margin: 0; }
  .lede { margin: 0 0 10px; color: var(--muted); line-height: 1.5; }
  label { display: flex; flex-direction: column; gap: 5px; }
  label > span { color: var(--muted); font-size: 12px; }
  label.check { flex-direction: row; align-items: center; gap: 8px; }
  label.check span { font-size: 13px; color: var(--ink); }
  .error { color: var(--removed); margin: 0; }
  button { height: 32px; margin-top: 6px; }
</style>
