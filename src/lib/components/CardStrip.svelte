<script lang="ts">
  import { formatBytes } from "$lib/api";
  import { app } from "$lib/state.svelte";

  const d = $derived(app.diff);
  const vol = $derived(app.volume);

  // Segments as fractions of card capacity.
  const seg = $derived.by(() => {
    if (!d || !d.totalBytes) return null;
    const total = d.totalBytes;
    const used = total - d.availableBytes;
    const leaving = Math.min(d.summary.freedBytes, used);
    const staying = used - leaving;
    const incoming = d.summary.downloadBytes;
    const pct = (n: number) => `${Math.max(0, (n / total) * 100)}%`;
    return {
      staying: pct(staying),
      leaving: pct(leaving),
      incoming: pct(Math.min(incoming, total - staying)),
      over: d.projectedFree < 0,
    };
  });

  const nothingToDo = $derived(!!d && d.summary.new + d.summary.updated + d.summary.removed === 0);
</script>

<div class="strip">
  {#if !vol}
    <p class="hint">Insert the SD card from your Echo, then choose it at the top.</p>
  {:else if !d}
    <p class="hint">{app.diffError ?? (app.diffLoading || app.libraryLoading ? "Comparing library with card…" : "")}</p>
  {:else}
    <div class="bar" role="img" aria-label="Card space after sync">
      {#if seg}
        <span class="s staying" style:width={seg.staying}></span>
        <span class="s leaving" style:width={seg.leaving}></span>
        <span class="s incoming" class:over={seg.over} style:width={seg.incoming}></span>
      {/if}
    </div>
    <div class="facts">
      <div class="changes">
        {#if nothingToDo}
          <span class="done">Card is up to date{d.summary.synced ? `, ${d.summary.synced} tracks` : ""}</span>
        {:else}
          {#if d.summary.new}<span class="c new"><b class="num">{d.summary.new}</b> new</span>{/if}
          {#if d.summary.updated}<span class="c updated"><b class="num">{d.summary.updated}</b> updated</span>{/if}
          {#if d.summary.removed}<span class="c removed"><b class="num">{d.summary.removed}</b> to remove</span>{/if}
          {#if d.summary.synced}<span class="c synced"><b class="num">{d.summary.synced}</b> unchanged</span>{/if}
        {/if}
      </div>
      <div class="space num">
        {#if d.summary.downloadBytes}<span>{formatBytes(d.summary.downloadBytes)} to copy</span>{/if}
        <span class:over={d.projectedFree < 0}>
          {d.projectedFree < 0
            ? `${formatBytes(-d.projectedFree)} over capacity`
            : `${formatBytes(d.projectedFree)} free after sync`}
        </span>
      </div>
    </div>
  {/if}
  {#if app.cardNotice}<p class="notice">{app.cardNotice}</p>{/if}
</div>

<style>
  .strip { padding: 12px 20px 14px; display: flex; flex-direction: column; gap: 9px; }
  .hint { margin: 0; color: var(--muted); }
  .notice { margin: 0; color: var(--updated); font-size: 12px; }
  .bar {
    height: 10px; border-radius: 3px; background: var(--active); display: flex; overflow: hidden;
  }
  .s { height: 100%; transition: width 0.35s ease; }
  .staying { background: var(--ink); opacity: 0.55; }
  .leaving {
    background: repeating-linear-gradient(135deg, var(--removed) 0 3px, transparent 3px 6px);
  }
  .incoming { background: var(--new); }
  .incoming.over { background: var(--removed); }
  .facts { display: flex; justify-content: space-between; gap: 16px; flex-wrap: wrap; }
  .changes { display: flex; gap: 14px; }
  .c b { font-weight: 650; }
  .c.new b { color: var(--new); }
  .c.updated b { color: var(--updated); }
  .c.removed b { color: var(--removed); }
  .c.synced { color: var(--muted); }
  .done { color: var(--muted); }
  .space { display: flex; gap: 14px; color: var(--muted); }
  .space .over { color: var(--removed); font-weight: 600; }
</style>
