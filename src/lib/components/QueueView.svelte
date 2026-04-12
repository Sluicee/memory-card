<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import {
    userQueueItems,
    sourceQueueItems,
    sourceQueueIndex,
    sourceReturnContext,
    currentTrack,
    currentAlbum,
    isPlaying,
    currentPlaylistId,
    isShuffled,
    removeFromQueue,
    moveInQueue,
    clearQueue,
    playFromUserQueue,
    jumpToInSourceQueue,
    jumpToInAlbum,
  } from '../stores/player';
  import { playUiSfx } from '$lib/ui-sfx';
  import { t } from '$lib/stores/i18n';

  // ── Source section ────────────────────────────────────────────────────────
  // While user queue is playing, $currentAlbum changes to the user queue track's album.
  // Use $sourceReturnContext to show the correct "next from album" tracks instead.
  const sourceItems = $derived.by(() => {
    if ($sourceQueueItems.length > 0) {
      return $sourceQueueItems.slice($sourceQueueIndex + 1);
    }
    const ctx = $sourceReturnContext;
    if (ctx) {
      const idx = ctx.album.tracks.findIndex(t => t.id === ctx.trackId);
      if (idx !== -1) return ctx.album.tracks.slice(idx + 1).map(t => ({ track: t, album: ctx.album }));
    }
    if ($currentAlbum && $currentTrack) {
      const idx = $currentAlbum.tracks.findIndex(t => t.id === $currentTrack!.id);
      if (idx !== -1) {
        return $currentAlbum.tracks.slice(idx + 1).map(t => ({ track: t, album: $currentAlbum! }));
      }
    }
    return [];
  });

  const sourceLabel = $derived.by(() => {
    if ($currentPlaylistId) return $t('nextFromPlaylist');
    if ($isShuffled)        return $t('nextFromShuffle');
    if ($sourceReturnContext ?? $currentAlbum) return $t('nextFromAlbum');
    return '';
  });

  // The album shown in the source header — prefer saved context over current album.
  const sourceAlbum = $derived($sourceReturnContext?.album ?? $currentAlbum);

  function fmt(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  // ── Pointer-capture drag-to-reorder ──────────────────────────────────────
  let listEl   = $state<HTMLUListElement | null>(null);
  let dragging = $state(false);
  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);

  function onHandleDown(e: PointerEvent, idx: number) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    dragFrom = idx;
    dragOver = idx;
    dragging = true;
    // Capture all pointer events on the list element
    if (listEl) listEl.setPointerCapture(e.pointerId);
  }

  function onListMove(e: PointerEvent) {
    if (!dragging || !listEl) return;
    const items = listEl.querySelectorAll<HTMLElement>('.q-item');
    let over = (dragFrom ?? 0);
    for (let i = 0; i < items.length; i++) {
      const r = items[i].getBoundingClientRect();
      if (e.clientY < r.top + r.height / 2) { over = i; break; }
      if (i === items.length - 1) over = i;
    }
    dragOver = over;
  }

  function onListUp() {
    if (dragging && dragFrom !== null && dragOver !== null && dragFrom !== dragOver) {
      moveInQueue(dragFrom, dragOver);
      playUiSfx('confirm');
    }
    dragging = false;
    dragFrom = null;
    dragOver = null;
  }

  // ── Actions ───────────────────────────────────────────────────────────────
  async function handleUserPlay(idx: number) {
    playUiSfx('confirm');
    await playFromUserQueue(idx);
  }

  function handleRemove(idx: number) {
    playUiSfx('back');
    removeFromQueue(idx);
  }

  function handleClear() {
    playUiSfx('back');
    clearQueue();
  }

  async function handleSourcePlay(relIdx: number) {
    playUiSfx('confirm');
    if ($sourceQueueItems.length > 0) {
      await jumpToInSourceQueue($sourceQueueIndex + 1 + relIdx);
    } else {
      const ctx = $sourceReturnContext;
      const album = ctx?.album ?? $currentAlbum;
      const refTrackId = ctx?.trackId ?? $currentTrack?.id;
      if (album && refTrackId) {
        const cur = album.tracks.findIndex(t => t.id === refTrackId);
        const tgt = album.tracks[cur + 1 + relIdx];
        if (tgt) await jumpToInAlbum(tgt, album);
      }
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="queue-view">

  <!-- NOW PLAYING -->
  {#if $currentTrack}
    <div class="now-section">
      <span class="section-label">{$t('nowPlaying')}</span>
      <div class="now-row">
        <div class="now-art">
          {#if $currentAlbum?.cover_art}
            <img src={convertFileSrc($currentAlbum.cover_art)} alt="" />
          {:else}
            <span class="art-ph">♪</span>
          {/if}
          {#if $isPlaying}<span class="pulse-dot"></span>{/if}
        </div>
        <div class="now-info">
          <span class="now-title">{$currentTrack.title}</span>
          <span class="now-sub">{$currentTrack.artist}{$currentAlbum ? ` · ${$currentAlbum.title}` : ''}</span>
        </div>
      </div>
    </div>
  {/if}

  <!-- USER QUEUE -->
  {#if $userQueueItems.length > 0}
    <div class="section-hd">
      <div class="section-hd-left">
        <span class="section-label">{$t('upNext')}</span>
        <span class="count-pill">{$userQueueItems.length}</span>
      </div>
      <button class="clear-btn" onclick={handleClear}>{$t('clearQueue')}</button>
    </div>

    <ul
      bind:this={listEl}
      class="queue-list"
      class:is-dragging={dragging}
      onpointermove={onListMove}
      onpointerup={onListUp}
      onpointercancel={onListUp}
    >
      {#each $userQueueItems as item, i (i)}
        <li
          class="q-item"
          class:drag-src={dragFrom === i}
          class:drag-tgt={dragging && dragOver === i && dragFrom !== i}
        >
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span
            class="drag-handle"
            onpointerdown={(e) => onHandleDown(e, i)}
            title="Drag to reorder"
          >
            <svg viewBox="0 0 6 10" fill="currentColor" width="7" height="11">
              <circle cx="1.5" cy="1.5" r="1.1"/><circle cx="4.5" cy="1.5" r="1.1"/>
              <circle cx="1.5" cy="5"  r="1.1"/><circle cx="4.5" cy="5"  r="1.1"/>
              <circle cx="1.5" cy="8.5" r="1.1"/><circle cx="4.5" cy="8.5" r="1.1"/>
            </svg>
          </span>

          <span class="q-pos">{i + 1}</span>

          <!-- Click to play immediately -->
          <button class="q-info-btn" onclick={() => handleUserPlay(i)}>
            <span class="q-title">{item.track.title}</span>
            <span class="q-artist">{item.track.artist}</span>
          </button>

          <span class="q-dur">{fmt(item.track.duration)}</span>

          <button class="q-remove" onclick={() => handleRemove(i)} title="Remove">
            <svg viewBox="0 0 8 8" fill="none" stroke="currentColor" stroke-width="1.8"
                 stroke-linecap="round" width="8" height="8">
              <line x1="1" y1="1" x2="7" y2="7"/><line x1="7" y1="1" x2="1" y2="7"/>
            </svg>
          </button>
        </li>
      {/each}
    </ul>

  {:else if !$currentTrack}
    <div class="empty-state">
      <p>{$t('noTrackPlaying')}</p>
    </div>
  {:else}
    <div class="empty-state empty-state--small">
      <p>{$t('queueEmpty')}</p>
      <p class="empty-hint">{$t('queueAddHint')}</p>
    </div>
  {/if}

  <!-- SOURCE (album / shuffle / playlist) -->
  {#if sourceItems.length > 0}
    <div class="section-hd section-hd--source">
      <span class="section-label">{sourceLabel}</span>
      {#if sourceAlbum && !$isShuffled && !$currentPlaylistId}
        <span class="source-name">{sourceAlbum.title}</span>
      {/if}
    </div>

    <ul class="queue-list source-list">
      {#each sourceItems as item, i (i)}
        <li class="q-item q-item--source">
          <span class="q-pos q-pos--dim">{i + 1}</span>
          <button class="q-info-btn" onclick={() => handleSourcePlay(i)}>
            <span class="q-title">{item.track.title}</span>
            <span class="q-artist">{item.track.artist}</span>
          </button>
          <span class="q-dur">{fmt(item.track.duration)}</span>
        </li>
      {/each}
    </ul>
  {/if}

</div>

<style>
  .queue-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    gap: 1px;
  }

  .section-label {
    font-size: 9px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-dim);
  }

  /* ── Now Playing ── */
  .now-section {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 0 2px 8px;
    border-bottom: 1px solid rgba(255,255,255,0.06);
    flex-shrink: 0;
  }

  .now-row {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.07);
    border-radius: 6px;
    padding: 6px 8px;
  }

  .now-art {
    width: 30px; height: 30px;
    border-radius: 3px;
    background: rgba(90,95,120,0.2);
    flex-shrink: 0;
    overflow: hidden;
    display: flex; align-items: center; justify-content: center;
    position: relative;
  }

  .now-art img { width: 100%; height: 100%; object-fit: cover; }

  .art-ph { font-size: 14px; color: var(--text-dim); text-shadow: none; }

  .pulse-dot {
    position: absolute; bottom: 2px; right: 2px;
    width: 5px; height: 5px;
    border-radius: 50%;
    background: var(--track-active);
    box-shadow: 0 0 4px var(--track-active);
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.35; }
  }

  .now-info { display: flex; flex-direction: column; gap: 2px; min-width: 0; flex: 1; }

  .now-title {
    font-size: 12px; color: var(--track-active);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  .now-sub {
    font-size: 9px; color: var(--text-dim);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  /* ── Section header ── */
  .section-hd {
    display: flex; align-items: center; justify-content: space-between;
    padding: 6px 2px 3px;
    flex-shrink: 0;
  }

  .section-hd--source {
    border-top: 1px solid rgba(255,255,255,0.06);
    padding-top: 8px; margin-top: 2px;
  }

  .section-hd-left { display: flex; align-items: center; gap: 6px; }

  .count-pill {
    font-size: 8px;
    background: var(--track-active); color: #000;
    border-radius: 999px; padding: 1px 5px;
    font-weight: 800; text-shadow: none;
  }

  .source-name {
    font-size: 9px; color: var(--text-dim); letter-spacing: 0.04em;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 160px;
  }

  .clear-btn {
    background: none; border: none; cursor: pointer;
    font-size: 9px; letter-spacing: 0.1em; text-transform: uppercase;
    color: var(--text-dim); font-family: inherit; font-weight: 800;
    padding: 2px 4px; border-radius: 3px;
    transition: color 0.12s, background 0.12s;
    text-shadow: var(--text-shadow);
  }

  .clear-btn:hover { color: #e05555; background: rgba(224,85,85,0.1); }

  /* ── Empty states ── */
  .empty-state {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 5px; padding: 20px 8px; color: var(--text-dim); font-size: 11px; flex-shrink: 0;
  }

  .empty-state--small { padding: 10px 8px; }

  .empty-hint { font-size: 9px; opacity: 0.6; text-align: center; }

  /* ── Lists ── */
  .queue-list {
    list-style: none;
    overflow-y: auto;
    min-height: 0;
    flex: 1;
    padding: 0 1px;
    touch-action: none; /* prevent scroll interfering with pointer capture */
  }

  .source-list { flex: 0 1 auto; max-height: 38%; }

  .queue-list::-webkit-scrollbar       { width: 3px; }
  .queue-list::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }

  .is-dragging { cursor: grabbing !important; user-select: none; }

  /* ── Queue item ── */
  .q-item {
    display: flex; align-items: center; gap: 4px;
    padding: 3px 2px;
    border-radius: 4px;
    border: 1px solid transparent;
    min-width: 0;
    transition: background 0.08s, border-color 0.08s, opacity 0.1s;
  }

  .q-item:hover { background: rgba(255,255,255,0.04); }

  .q-item.drag-src  { opacity: 0.3; }

  .q-item.drag-tgt  {
    border-top-color: var(--track-hover);
    background: rgba(16,174,210,0.08);
  }

  .q-item--source { opacity: 0.65; }
  .q-item--source:hover { opacity: 1; }

  /* Drag handle */
  .drag-handle {
    flex-shrink: 0;
    color: rgba(255,255,255,0.15);
    cursor: grab;
    display: flex; align-items: center;
    padding: 0 3px;
    touch-action: none;
    transition: color 0.1s;
    user-select: none;
  }

  .is-dragging .drag-handle { cursor: grabbing; }
  .q-item:hover .drag-handle { color: rgba(255,255,255,0.45); }

  /* Position number */
  .q-pos {
    font-size: 9px; color: var(--text-dim);
    width: 14px; text-align: right; flex-shrink: 0;
  }

  .q-pos--dim { color: rgba(255,255,255,0.2); }

  /* Info button (click to play) */
  .q-info-btn {
    display: flex; flex-direction: column; gap: 1px;
    flex: 1; min-width: 0;
    background: none; border: none; cursor: pointer;
    text-align: left; padding: 3px 4px;
    border-radius: 3px;
    transition: background 0.08s;
  }

  .q-info-btn:hover { background: rgba(255,255,255,0.05); }
  .q-info-btn:hover .q-title { color: var(--track-hover); }

  .q-title {
    font-size: 11px; color: var(--text-primary);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    transition: color 0.1s;
  }

  .q-artist {
    font-size: 9px; color: var(--text-dim);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  .q-dur {
    font-size: 9px; color: var(--text-dim);
    flex-shrink: 0; letter-spacing: 0.03em;
  }

  /* Remove button */
  .q-remove {
    flex-shrink: 0;
    background: none; border: none; cursor: pointer;
    color: var(--text-dim); padding: 3px 4px;
    border-radius: 3px; opacity: 0;
    display: flex; align-items: center;
    transition: opacity 0.1s, color 0.1s, background 0.1s;
  }

  .q-item:hover .q-remove { opacity: 1; }
  .q-remove:hover { color: #e05555; background: rgba(224,85,85,0.12); }
</style>
