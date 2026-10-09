<script lang="ts">
  import { api, errorText, type Settings } from "$lib/api";
  import { app } from "$lib/state.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let draft = $state<Settings>({ ...app.settings! });
  let error = $state<string | null>(null);
  const defaultTemplate = "Music/{albumartist}/[{year} - ]{album}/[{disc}-]{track:02} {title}";

  // Preview of the folder layout with sample data (mirrors paths.rs rules loosely).
  const preview = $derived.by(() => {
    const sample: Record<string, string> = {
      albumartist: "Radiohead", artist: "Radiohead", album: "OK Computer", title: "Paranoid Android",
      year: "1997", track: "2", disc: "", genre: "Rock",
    };
    const fill = (s: string, strict: boolean) =>
      s.replace(/\{(\w+)(?::(\d+))?\}/g, (_, k: string, pad?: string) => {
        const v = sample[k] ?? "";
        if (!v && strict) throw 0;
        return pad && v ? v.padStart(Number(pad), "0") : v;
      });
    const out = draft.pathTemplate.replace(/\[([^\]]*)\]/g, (_, g: string) => {
      try { return fill(g, true); } catch { return ""; }
    });
    const ext = draft.profile === "original" ? "flac" : "mp3";
    return `${fill(out, false).trim()}.${ext}`;
  });

  async function save() {
    try {
      app.settings = await api.saveSettings(draft);
      app.scheduleDiff(0);
      onclose();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function signOut() {
    onclose();
    await app.signOut();
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window onkeydown={keydown} />

<div class="scrim" onclick={onclose} role="presentation"></div>
<div class="sheet" role="dialog" aria-modal="true" aria-labelledby="settings-title">
  <h2 id="settings-title">Settings</h2>

  <label>
    <span>Audio format on card</span>
    <select class="field" bind:value={draft.profile}>
      <option value="original">Original files (FLAC, MP3, …)</option>
      <option value="mp3-320">MP3 320 kbps, converted by server</option>
      <option value="mp3-256">MP3 256 kbps, converted by server</option>
      <option value="mp3-192">MP3 192 kbps, converted by server</option>
    </select>
    <small>Converting saves space. Changing this re-copies every track.</small>
  </label>

  <label>
    <span>Embedded cover size</span>
    <select class="field" bind:value={draft.coverSize}>
      <option value={300}>300 px</option>
      <option value={500}>500 px (recommended)</option>
      <option value={800}>800 px</option>
    </select>
    <small>The Echo only shows covers stored inside each track, not cover.jpg files.</small>
  </label>

  <label>
    <span>Parallel downloads</span>
    <input class="field" type="number" min="1" max="16" bind:value={draft.concurrency} />
  </label>

  <label>
    <span>Folder layout</span>
    <input class="field mono" bind:value={draft.pathTemplate} spellcheck="false" />
    <small class="preview">{preview}</small>
    <small>
      Fields: {"{albumartist} {artist} {album} {title} {year} {track} {disc} {genre}"}. Parts in [brackets] are left out when a field is empty.
      {#if draft.pathTemplate !== defaultTemplate}
        <button class="link" onclick={() => (draft.pathTemplate = defaultTemplate)}>Reset</button>
      {/if}
    </small>
  </label>

  <label class="check">
    <input type="checkbox" bind:checked={draft.cleanAppleDouble} />
    <span>Delete macOS “._” files after syncing</span>
  </label>

  {#if error}<p class="error">{error}</p>{/if}

  <div class="foot">
    <button class="ghost danger" onclick={signOut}>Sign out of {app.server?.username}</button>
    <span></span>
    <button class="secondary" onclick={onclose}>Cancel</button>
    <button class="primary" onclick={save}>Save</button>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.25); z-index: 10; }
  .sheet {
    position: fixed; z-index: 11; top: 52px; left: 50%; transform: translateX(-50%);
    width: min(480px, calc(100vw - 32px)); max-height: calc(100vh - 72px); overflow-y: auto;
    background: var(--surface); border-radius: 10px; padding: 20px;
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.25), 0 0 0 1px var(--line);
    display: flex; flex-direction: column; gap: 14px;
    animation: drop 0.18s ease-out;
  }
  @keyframes drop { from { opacity: 0; transform: translate(-50%, -8px); } }
  h2 { margin: 0; font-size: 16px; font-weight: 650; }
  label { display: flex; flex-direction: column; gap: 5px; }
  label > span { font-weight: 550; }
  small { color: var(--muted); font-size: 11.5px; line-height: 1.45; }
  .preview { color: var(--ink); word-break: break-all; }
  .mono { font-family: ui-monospace, "SF Mono", Menlo, monospace; font-size: 12px; }
  .check { flex-direction: row; align-items: center; gap: 8px; }
  .check span { font-weight: 400; }
  .link { border: 0; background: none; color: var(--focus); padding: 0; font-size: inherit; }
  .error { color: var(--removed); margin: 0; }
  .foot { display: flex; gap: 8px; align-items: center; margin-top: 4px; }
  .foot span { flex: 1; }
  .danger:hover { color: var(--removed); }
</style>
