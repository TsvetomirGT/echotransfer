<script lang="ts">
  import type { ArtistView, Status } from "$lib/api";
  import { app } from "$lib/state.svelte";

  let { focused = $bindable<string | null>(null) } = $props();

  let query = $state("");
  let onlySelected = $state(false);

  const rank: Record<Status, number> = { removed: 3, updated: 2, new: 1, synced: 0 };

  function artistStatus(a: ArtistView): Status | null {
    let best: Status | null = null;
    for (const al of a.albums) {
      const s = app.diff?.albums[al.id]?.status;
      if (s && (best === null || rank[s] > rank[best])) best = s;
    }
    return best;
  }

  function selCount(a: ArtistView) {
    return a.albums.filter((al) => app.selection.has(al.id)).length;
  }

  const visible = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return app.artists.filter(
      (a) =>
        (!q || a.name.toLowerCase().includes(q) || a.albums.some((al) => al.name.toLowerCase().includes(q))) &&
        (!onlySelected || selCount(a) > 0),
    );
  });

  function indeterminate(node: HTMLInputElement, value: boolean) {
    node.indeterminate = value;
    return { update: (v: boolean) => (node.indeterminate = v) };
  }

  function toggle(a: ArtistView) {
    const all = selCount(a) === a.albums.length;
    app.toggleAlbums(a.albums.map((al) => al.id), !all);
  }
</script>

<div class="top">
  <input class="field" type="search" placeholder="Search artists and albums" bind:value={query} spellcheck="false" />
  <div class="filters" role="tablist">
    <button role="tab" aria-selected={!onlySelected} class:on={!onlySelected} onclick={() => (onlySelected = false)}>All</button>
    <button role="tab" aria-selected={onlySelected} class:on={onlySelected} onclick={() => (onlySelected = true)}>
      On card <span class="num">{app.selection.size}</span>
    </button>
  </div>
</div>

<ul>
  {#each visible as a (a.id)}
    {@const n = selCount(a)}
    {@const st = artistStatus(a)}
    <li class:focused={focused === a.id}>
      <input
        type="checkbox"
        aria-label={`Sync ${a.name}`}
        checked={n === a.albums.length}
        use:indeterminate={n > 0 && n < a.albums.length}
        onchange={() => toggle(a)}
      />
      <button class="name" onclick={() => (focused = a.id)}>
        <span class="label">{a.name}</span>
        {#if st && st !== "synced"}<span class="dot {st}" title={st}></span>{/if}
        <span class="count num">{a.albums.length}</span>
      </button>
    </li>
  {:else}
    <li class="empty">{app.libraryLoading ? "Loading library…" : query ? "No matches" : onlySelected ? "Nothing selected for this card yet" : "No music found"}</li>
  {/each}
</ul>

<style>
  .top { padding: 0 12px 10px; display: flex; flex-direction: column; gap: 8px; }
  .filters { display: flex; gap: 2px; background: var(--active); border-radius: 6px; padding: 2px; }
  .filters button {
    flex: 1; border: 0; background: transparent; border-radius: 4px; height: 22px; color: var(--muted); font-size: 12px;
  }
  .filters button.on { background: var(--surface); color: var(--ink); box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08); }
  .filters .num { color: var(--faint); margin-left: 2px; }
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
