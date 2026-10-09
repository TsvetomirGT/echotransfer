<script lang="ts">
  import { api, formatBytes, type Outcome, type Progress } from "$lib/api";
  import { app } from "$lib/state.svelte";

  let { progress, outcome, error, ondismiss }: {
    progress: Progress | null;
    outcome: Outcome | null;
    error: string | null;
    ondismiss: () => void;
  } = $props();

  let speed = $state(0);
  let last: { t: number; b: number } | null = null;

  $effect(() => {
    if (!progress) return;
    const now = performance.now();
    if (last && now > last.t) {
      const inst = ((progress.bytesDone - last.b) * 1000) / (now - last.t);
      speed = speed ? speed * 0.85 + inst * 0.15 : inst;
    }
    last = { t: now, b: progress.bytesDone };
  });

  const phaseText: Record<string, string> = {
    preparing: "Preparing",
    removing: "Removing tracks",
    moving: "Renaming files",
    downloading: "Copying",
    cleaning: "Tidying up",
    done: "Finishing",
  };

  const frac = $derived(
    progress
      ? progress.bytesTotal > 0 && progress.phase === "downloading"
        ? Math.min(1, progress.bytesDone / progress.bytesTotal)
        : progress.total > 0
          ? progress.done / progress.total
          : 0
      : 0,
  );

  const eta = $derived.by(() => {
    if (!progress || speed < 1000 || progress.phase !== "downloading") return null;
    const s = Math.max(0, (progress.bytesTotal - progress.bytesDone) / speed);
    return s < 60 ? "under a minute left" : `about ${Math.round(s / 60)} min left`;
  });

  let showFailures = $state(false);
</script>

<div class="panel">
  {#if outcome || error}
    <div class="result">
      <div class="msg">
        {#if error}
          <b class="bad">Sync stopped</b><span>{error}</span>
        {:else if outcome}
          <b>{outcome.cancelled ? "Sync cancelled" : outcome.failures.length ? "Synced with problems" : "Card synced"}</b>
          <span class="num">
            {outcome.downloaded} copied, {outcome.deleted} removed{outcome.moved ? `, ${outcome.moved} renamed` : ""}{outcome.failures.length ? `, ${outcome.failures.length} failed` : ""}.
            {#if !outcome.cancelled && !outcome.failures.length}It's safe to eject the card.{/if}
          </span>
        {/if}
      </div>
      <div class="actions">
        {#if outcome?.failures.length}
          <button class="ghost" onclick={() => (showFailures = !showFailures)}>{showFailures ? "Hide" : "Show"} failures</button>
        {/if}
        <button class="primary" onclick={ondismiss}>Done</button>
      </div>
    </div>
    {#if showFailures && outcome}
      <ul class="failures">
        {#each outcome.failures as f}
          <li><b>{f.title}</b><span>{f.error}</span></li>
        {/each}
      </ul>
    {/if}
  {:else}
    <div class="now">
      <span class="cover">
        {#if progress?.currentCover}<img src={app.coverUrl(progress.currentCover, 120)} alt="" />{/if}
      </span>
      <div class="text">
        <b>{phaseText[progress?.phase ?? "preparing"] ?? "Working"}</b>
        <span class="cur">{progress?.current ?? ""}</span>
      </div>
      <div class="stats num">
        {#if progress}
          <span>{progress.done} of {progress.total}</span>
          {#if progress.phase === "downloading"}
            <span>{formatBytes(progress.bytesDone)} of {formatBytes(progress.bytesTotal)}</span>
            {#if speed > 0}<span>{formatBytes(speed)}/s</span>{/if}
          {/if}
          {#if eta}<span>{eta}</span>{/if}
        {/if}
      </div>
      <button class="secondary" onclick={() => api.cancelSync()}>Cancel</button>
    </div>
    <div class="track" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(frac * 100)}>
      <span style:width={`${frac * 100}%`}></span>
    </div>
  {/if}
</div>

<style>
  .panel { padding: 12px 20px 14px; display: flex; flex-direction: column; gap: 10px; }
  .now { display: flex; align-items: center; gap: 12px; }
  .cover { width: 36px; height: 36px; border-radius: 4px; background: var(--active); overflow: hidden; flex: none; }
  .cover img { width: 100%; height: 100%; object-fit: cover; }
  .text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .cur { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .stats { display: flex; gap: 14px; color: var(--muted); font-size: 12px; }
  .track { height: 4px; background: var(--active); border-radius: 2px; overflow: hidden; }
  .track span { display: block; height: 100%; background: var(--new); transition: width 0.25s linear; }
  .result { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  .msg { display: flex; flex-direction: column; }
  .msg span { color: var(--muted); }
  .bad { color: var(--removed); }
  .actions { display: flex; gap: 6px; }
  .failures { list-style: none; margin: 0; padding: 0; max-height: 140px; overflow-y: auto; font-size: 12px; user-select: text; -webkit-user-select: text; }
  .failures li { display: flex; flex-direction: column; padding: 4px 0; border-top: 1px solid var(--line); }
  .failures span { color: var(--removed); }
</style>
