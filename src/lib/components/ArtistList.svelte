<script lang="ts">
  import { formatBytes, type AlbumView, type ArtistView, type Status } from "$lib/api";
  import { app, FILTERS } from "$lib/state.svelte";

  let { focused = $bindable<string | null>(null) } = $props();

  const rank: Record<Status, number> = { removed: 3, updated: 2, new: 1, synced: 0 };

  function artistStatus(a: ArtistView): Status | null {
    let best: Status | null = null;
    for (const al of a.albums) {
      const s = app.diff?.albums[al.id]?.status;
      if (s && (best === null || rank[s] > rank[best])) best = s;
    }
    return best;
  }

  function selCount(albums: AlbumView[]) {
    return albums.filter((al) => app.selection.has(al.id)).length;
  }

  const allMatchingSelected = $derived(app.matching.length > 0 && selCount(app.matching) === app.matching.length);
  const matchingBytes = $derived(app.matching.reduce((n, al) => n + al.size, 0));

  const emptyText: Record<string, string> = {
    missing: "Every album is on this card",
    recent: "Nothing added in the last 30 days",
    updated: "Nothing on the card has changed",
    onCard: "Nothing selected for this card yet",
  };

  function indeterminate(node: HTMLInputElement, value: boolean) {
    node.indeterminate = value;
    return { update: (v: boolean) => (node.indeterminate = v) };
  }

  function toggle(albums: AlbumView[]) {
    app.toggleAlbums(albums.map((al) => al.id), selCount(albums) !== albums.length);
  }
</script>

<div class="top">
  <input class="field" type="search" placeholder="Search artists and albums" bind:value={app.query} spellcheck="false" />
  <div class="filters" role="radiogroup" aria-label="Show albums">
    {#each FILTERS as f (f.id)}
      <button role="radio" aria-checked={app.filter === f.id} class:on={app.filter === f.id} title={f.title} onclick={() => (app.filter = f.id)}>
        {f.label} <span class="num">{app.filterCounts[f.id]}</span>
      </button>
    {/each}
  </div>
  {#if app.matching.length > 0}
    <div class="bulk">
      <span class="num">{app.matching.length} {app.matching.length === 1 ? "album" : "albums"}, {formatBytes(matchingBytes)}</span>
      <button class="secondary" onclick={() => toggle(app.matching)}>
        {allMatchingSelected ? "Remove all" : "Add all"}
      </button>
    </div>
  {/if}
</div>

<ul>
  {#each app.visible as { artist: a, albums } (a.id)}
    {@const n = selCount(albums)}
    {@const st = artistStatus(a)}
    <li class:focused={focused === a.id}>
      <input
        type="checkbox"
        aria-label={`Sync ${a.name}`}
        checked={n === albums.length}
        use:indeterminate={n > 0 && n < albums.length}
        onchange={() => toggle(albums)}
      />
      <button class="name" onclick={() => (focused = a.id)}>
        <span class="label">{a.name}</span>
        {#if st && st !== "synced"}<span class="dot {st}" title={st}></span>{/if}
        <span class="count num">{albums.length}</span>
      </button>
    </li>
  {:else}
    <li class="empty">
      {app.libraryLoading ? "Loading library…" : app.query.trim() ? "No matches" : (emptyText[app.filter] ?? "No music found")}
    </li>
  {/each}
</ul>

<style>
  .top { padding: 0 12px 10px; display: flex; flex-direction: column; gap: 8px; }
  .filters { display: flex; flex-wrap: wrap; gap: 4px; }
  .filters button {
    border: 0; background: var(--active); border-radius: 11px; height: 22px; padding: 0 8px; color: var(--muted); font-size: 12px;
  }
  .filters button:hover { color: var(--ink); }
  .filters button.on { background: var(--surface); color: var(--ink); box-shadow: 0 0 0 1px var(--line), 0 1px 2px rgba(0, 0, 0, 0.08); }
  .filters .num { color: var(--faint); margin-left: 2px; }
  .bulk { display: flex; align-items: center; justify-content: space-between; gap: 8px; font-size: 12px; color: var(--muted); }
  .bulk button { height: 24px; padding: 0 10px; font-size: 12px; }
  ul { list-style: none; margin: 0; padding: 0 6px 12px; overflow-y: auto; flex: 1; }
  li { display: flex; align-items: center; gap: 4px; padding-left: 6px; border-radius: var(--radius-sm); }
  li:hover { background: var(--hover); }
  li.focused { background: var(--active); }
  li input { margin: 0; flex: none; }
  .name {
    flex: 1; min-width: 0; display: flex; align-items: center; gap: 6px;
    border: 0; background: transparent; text-align: left; padding: 5px 6px; height: 28px;
  }
  .label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count { color: var(--faint); font-size: 11px; }
  .dot { width: 7px; height: 7px; border-radius: 50%; flex: none; }
  .dot.new { background: var(--new); }
  .dot.updated { background: var(--updated); }
  .dot.removed { background: var(--removed); }
  .empty { color: var(--muted); padding: 16px 8px; }
  .empty:hover { background: none; }
</style>
