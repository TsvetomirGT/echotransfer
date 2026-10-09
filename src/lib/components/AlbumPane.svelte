<script lang="ts">
  import { api, formatBytes, formatDuration, type ArtistView, type Status, type TrackRow } from "$lib/api";
  import { app } from "$lib/state.svelte";

  let { artist }: { artist: ArtistView | null } = $props();

  let open = $state<string | null>(null);
  let tracks = $state<TrackRow[]>([]);

  const label: Record<Status, string> = { new: "New", updated: "Updated", removed: "Remove", synced: "On card" };

  const allSelected = $derived(!!artist && artist.albums.every((a) => app.selection.has(a.id)));

  async function expand(id: string) {
    if (open === id) {
      open = null;
      return;
    }
    open = id;
    tracks = await api.albumTracks(id);
  }

  // Refresh open track list when the diff changes.
  $effect(() => {
    app.diff;
    if (open) api.albumTracks(open).then((t) => (tracks = t));
  });

  function trackLabel(s: Status | null) {
    return s ? label[s] : "";
  }
</script>

{#if !artist}
  <div class="empty">
    <p>Choose an artist on the left to see their albums.</p>
  </div>
{:else}
  <div class="head">
    <h2>{artist.name}</h2>
    <button class="secondary" onclick={() => app.toggleAlbums(artist.albums.map((a) => a.id), !allSelected)}>
      {allSelected ? "Remove all from card" : "Add all to card"}
    </button>
  </div>
  <ul>
    {#each artist.albums as al (al.id)}
      {@const d = app.diff?.albums[al.id]}
      {@const cover = app.coverUrl(al.coverArt, 120)}
      <li class:open={open === al.id}>
        <div class="row">
          <input
            type="checkbox"
            aria-label={`Sync ${al.name}`}
            checked={app.selection.has(al.id)}
            onchange={(e) => app.toggleAlbums([al.id], e.currentTarget.checked)}
          />
          <button class="main" onclick={() => expand(al.id)} aria-expanded={open === al.id}>
            <span class="cover">
              {#if cover}<img src={cover} alt="" loading="lazy" onerror={(e) => ((e.currentTarget as HTMLImageElement).style.visibility = "hidden")} />{/if}
            </span>
            <span class="text">
              <span class="title">{al.name}</span>
              <span class="meta num">
                {#if al.year}{al.year}, {/if}{al.songCount} {al.songCount === 1 ? "track" : "tracks"}, {formatDuration(al.duration)}
              </span>
            </span>
            <span class="size num">{formatBytes(al.size)}</span>
            <span class="status">
              {#if d?.status}<span class="pill {d.status}">{label[d.status]}</span>{/if}
            </span>
          </button>
        </div>
        {#if open === al.id}
          <ol class="tracks">
            {#each tracks as t (t.id)}
              <li>
                <span class="n num">{t.track ?? ""}</span>
                <span class="t">{t.title}</span>
                <span class="fmt">{t.suffix}</span>
                <span class="num dur">{formatDuration(t.duration)}</span>
                <span class="ts {t.status ?? ''}">{trackLabel(t.status)}</span>
              </li>
            {/each}
          </ol>
        {/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .empty { height: 100%; display: grid; place-items: center; color: var(--muted); }
  .head {
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    padding: 18px 24px 12px;
  }
  h2 { margin: 0; font-size: 20px; font-weight: 650; letter-spacing: -0.01em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  ul { list-style: none; margin: 0; padding: 0 16px 24px; }
  li.open { background: var(--hover); border-radius: var(--radius); }
  .row { display: flex; align-items: center; gap: 8px; padding-left: 8px; border-radius: var(--radius); }
  .row:hover { background: var(--hover); }
  .row input { margin: 0; flex: none; }
  .main {
    flex: 1; min-width: 0; display: grid; grid-template-columns: 44px 1fr auto 86px; align-items: center; gap: 12px;
    border: 0; background: transparent; text-align: left; padding: 7px 8px;
  }
  .cover { width: 44px; height: 44px; border-radius: 4px; background: var(--active); overflow: hidden; }
  .cover img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .text { min-width: 0; display: flex; flex-direction: column; }
  .title { font-weight: 560; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { color: var(--muted); font-size: 12px; }
  .size { color: var(--muted); font-size: 12px; }
  .status { justify-self: end; }
  .tracks { list-style: none; margin: 0; padding: 2px 16px 10px 76px; }
  .tracks li { display: grid; grid-template-columns: 24px 1fr auto 44px 64px; gap: 10px; padding: 4px 0; border-top: 1px solid var(--line); font-size: 12px; }
  .tracks li:first-child { border-top: 0; }
  .n, .dur, .fmt { color: var(--faint); }
  .fmt { text-transform: lowercase; }
  .t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ts { text-align: right; font-weight: 600; font-size: 11px; color: var(--synced); }
  .ts.new { color: var(--new); }
  .ts.updated { color: var(--updated); }
  .ts.removed { color: var(--removed); }
</style>
