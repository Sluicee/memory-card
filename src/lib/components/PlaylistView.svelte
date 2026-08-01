<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { Track } from '../types';
  import { playlists, reorderPlaylistTrack, exportPlaylist, importIntoPlaylist, type Playlist } from '../stores/playlists';
  import { albums } from '../stores/library';
  import {
    currentTrack,
    currentAlbum,
    isPlaying,
    playPlaylist,
    playShuffledPlaylist,
    playNext,
    playPrev,
    pause,
    resume,
    isShuffled,
    repeatMode,
    toggleRepeat,
    type QueueItem,
  } from '../stores/player';
  import SpinningCover from './SpinningCover.svelte';
  import VolumeControl from './VolumeControl.svelte';
  import PS2Btn from './PS2Btn.svelte';
  import { playUiSfx } from '$lib/ui-sfx';
  import { t } from '$lib/stores/i18n';

  let {
    playlist,
    onclose,
  }: {
    playlist: Playlist;
    onclose: () => void;
  } = $props();

  // Always read from the store so reorders are reflected immediately
  const currentPlaylist = $derived($playlists.find((p) => p.id === playlist.id) ?? playlist);

  // Build QueueItem[] for playback (rebuilt when tracks reorder)
  const queueItems = $derived(
    currentPlaylist.tracks
      .map((track) => {
        const album = $albums.find(
          (a) => a.title === track.album && (a.artist === track.album_artist || a.artist === track.artist)
        );
        return album ? ({ track, album } satisfies QueueItem) : null;
      })
      .filter((x): x is QueueItem => x !== null)
  );

  // Is any track from this playlist currently active?
  const isActivePlaylist = $derived(currentPlaylist.tracks.some((t) => t.id === $currentTrack?.id));

  // Cover: current album's cover if playing from this playlist, else first available
  const activeCoverSrc = $derived((() => {
    if (isActivePlaylist && $currentAlbum?.cover_art) {
      return convertFileSrc($currentAlbum.cover_art);
    }
    for (const item of queueItems) {
      if (item.album.cover_art) return convertFileSrc(item.album.cover_art);
    }
    return null;
  })());

  // Tint from cover
  let tintColor = $state('rgba(120, 120, 140, 0.25)');

  $effect(() => {
    if (activeCoverSrc) {
      extractDominantColor(activeCoverSrc).then((c) => (tintColor = c));
    } else {
      tintColor = 'rgba(120, 120, 140, 0.25)';
    }
  });

  async function extractDominantColor(src: string): Promise<string> {
    try {
      const bitmap = await createImageBitmap(
        await fetch(src).then((r) => r.blob()),
        { resizeWidth: 8, resizeHeight: 8, resizeQuality: 'low' }
      );
      const canvas = document.createElement('canvas');
      canvas.width = 8; canvas.height = 8;
      const ctx = canvas.getContext('2d')!;
      ctx.drawImage(bitmap, 0, 0);
      bitmap.close();
      const data = ctx.getImageData(0, 0, 8, 8).data;
      let r = 0, g = 0, b = 0;
      const px = data.length / 4;
      for (let i = 0; i < data.length; i += 4) {
        r += data[i]; g += data[i + 1]; b += data[i + 2];
      }
      return `rgba(${Math.round(r / px)}, ${Math.round(g / px)}, ${Math.round(b / px)}, 0.4)`;
    } catch {
      return 'rgba(120, 120, 140, 0.25)';
    }
  }

  function formatDuration(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  async function handleTrackClick(track: Track) {
    if ($currentTrack?.id === track.id) {
      if ($isPlaying) await pause(); else await resume();
    } else {
      playUiSfx('confirm');
      const idx = queueItems.findIndex((qi) => qi.track.id === track.id);
      if (idx !== -1) await playPlaylist(queueItems, idx, playlist.id);
    }
  }

  async function handlePlayPause() {
    if (isActivePlaylist) {
      if ($isPlaying) await pause(); else await resume();
    } else {
      if (!queueItems.length) return;
      playUiSfx('confirm');
      await playPlaylist(queueItems, 0, playlist.id);
    }
  }

  async function handleShuffle() {
    playUiSfx('confirm');
    await playShuffledPlaylist(queueItems, playlist.id);
  }

  async function handlePrev() {
    if (!$currentAlbum) return;
    playUiSfx('nextPrev');
    await playPrev($currentAlbum);
  }

  async function handleNext() {
    if (!$currentAlbum) return;
    playUiSfx('nextPrev');
    await playNext($currentAlbum);
  }

  function handleClose() {
    playUiSfx('back');
    onclose();
  }

  function handleOverlayMouseDown(e: MouseEvent) {
    if (e.target === e.currentTarget) handleClose();
  }

  const isFavourites = $derived(playlist.id === 'favourites');

  // M3U export / import
  let m3uStatus = $state<string | null>(null);
  let m3uStatusTimer = $state<ReturnType<typeof setTimeout> | null>(null);

  function showM3UStatus(msg: string) {
    if (m3uStatusTimer) clearTimeout(m3uStatusTimer);
    m3uStatus = msg;
    m3uStatusTimer = setTimeout(() => { m3uStatus = null; }, 2800);
  }

  async function handleExport() {
    playUiSfx('confirm');
    try {
      await exportPlaylist(currentPlaylist);
    } catch (e) {
      showM3UStatus(String(e));
    }
  }

  async function handleImport() {
    playUiSfx('confirm');
    try {
      const n = await importIntoPlaylist(playlist.id);
      showM3UStatus($t('importedTracks', n));
    } catch (e) {
      showM3UStatus(String(e));
    }
  }

  // Drag-to-reorder state (pointer-based, avoids Tauri's OS file-drop interceptor)
  let listEl = $state<HTMLUListElement | null>(null);
  let dragIdx = $state<number | null>(null);
  let dropIdx = $state<number | null>(null);

  function handleHandlePointerDown(e: PointerEvent, idx: number) {
    e.preventDefault();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    dragIdx = idx;
    dropIdx = idx;
  }

  function handleHandlePointerMove(e: PointerEvent) {
    if (dragIdx === null || !listEl) return;
    const items = Array.from(listEl.querySelectorAll<HTMLElement>('li.track'));
    for (let i = 0; i < items.length; i++) {
      const rect = items[i].getBoundingClientRect();
      if (e.clientY < rect.top + rect.height / 2) {
        dropIdx = i;
        return;
      }
    }
    dropIdx = items.length - 1;
  }

  function handleHandlePointerUp() {
    if (dragIdx !== null && dropIdx !== null && dragIdx !== dropIdx) {
      reorderPlaylistTrack(playlist.id, dragIdx, dropIdx);
    }
    dragIdx = null;
    dropIdx = null;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="overlay"
  style="background: {tintColor};"
  onmousedown={handleOverlayMouseDown}
>
  <div class="view">
    <!-- 3D spinning cover -->
    {#if activeCoverSrc}
      <SpinningCover
        src={activeCoverSrc}
        alt={playlist.name}
        spin={isActivePlaylist && $isPlaying}
      />
    {:else}
      <div class="cover-placeholder">
        {#if isFavourites}
          <svg viewBox="0 0 16 16" width="32" height="32" fill="currentColor" style="display: inline-block; vertical-align: middle;">
            <path d="M3.612 15.443c-.386.198-.824-.149-.746-.592l.83-4.73L.173 6.765c-.329-.314-.158-.888.283-.95l4.898-.696L7.538.792c.197-.39.73-.39.927 0l2.184 4.327 4.898.696c.441.062.612.636.282.95l-3.522 3.356.83 4.73c.078.443-.36.79-.746.592L8 13.187l-4.389 2.256z"/>
          </svg>
        {:else}
          <svg viewBox="0 0 16 16" width="32" height="32" fill="currentColor" style="display: inline-block; vertical-align: middle;">
            <path d="M9 13c0 1.105-1.12 2-2.5 2S4 14.105 4 13s1.12-2 2.5-2 2.5.895 2.5 2zM9 3v7h1v-7H9z"/>
            <path d="M9 3v.5a.5.5 0 0 0 .5.5h4a.5.5 0 0 0 .5-.5V3a.5.5 0 0 0-.5-.5h-4A.5.5 0 0 0 9 3z"/>
          </svg>
        {/if}
      </div>
    {/if}

    <!-- Info + tracklist -->
    <div class="panel">
      <div class="playlist-meta">
        <h2 class="playlist-title">{currentPlaylist.name}</h2>
        <p class="playlist-count">{$t('trackCount', currentPlaylist.tracks.length)}</p>
      </div>

      {#if currentPlaylist.tracks.length === 0}
        <p class="empty-hint">{$t('noTracksYet')}</p>
      {:else}
        <ul class="tracklist" bind:this={listEl}>
          {#each currentPlaylist.tracks as track, i (track.id)}
            {@const active = $currentTrack?.id === track.id}
            <li
              class="track"
              class:active
              class:dragging={dragIdx === i}
              class:drop-target={dropIdx === i && dragIdx !== i}
            >
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <span
                class="drag-handle"
                aria-hidden="true"
                onpointerdown={(e) => handleHandlePointerDown(e, i)}
                onpointermove={handleHandlePointerMove}
                onpointerup={handleHandlePointerUp}
              >
                <svg viewBox="0 0 16 16" width="12" height="12" fill="currentColor" style="display: inline-block; vertical-align: middle;">
                  <rect x="4" y="2" width="2" height="2" rx="0.5"/>
                  <rect x="4" y="7" width="2" height="2" rx="0.5"/>
                  <rect x="4" y="12" width="2" height="2" rx="0.5"/>
                  <rect x="10" y="2" width="2" height="2" rx="0.5"/>
                  <rect x="10" y="7" width="2" height="2" rx="0.5"/>
                  <rect x="10" y="12" width="2" height="2" rx="0.5"/>
                </svg>
              </span>
              <button class="track-btn" onclick={() => handleTrackClick(track)}>
                <span class="track-num">
                  {#if active && $isPlaying}
                    <span class="playing-dot">
                      <svg viewBox="0 0 16 16" width="10" height="10" fill="currentColor" style="display: inline-block; vertical-align: middle;">
                        <path d="m11.596 8.697-6.363 3.692c-.54.313-1.233-.066-1.233-.697V4.308c0-.63.692-1.01 1.233-.696l6.363 3.692a.802.802 0 0 1 0 1.393z"/>
                      </svg>
                    </span>
                  {:else}
                    {i + 1}
                  {/if}
                </span>
                <span class="track-title">{track.title}</span>
                <span class="track-artist">{track.artist}</span>
                <span class="track-dur">{formatDuration(track.duration)}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>

  <!-- Bottom hints -->
  <div class="hints">
    <div class="hints-row">
      <button class="hint-btn" onclick={handleClose}>
        <PS2Btn type="circle" />
        <span>{$t('back')}</span>
      </button>
      {#if queueItems.length > 0}
        <button class="hint-btn" onclick={handlePlayPause}>
          <PS2Btn type="cross" />
          <span class="play-pause-text">{isActivePlaylist && $isPlaying ? $t('pause') : $t('play')}</span>
        </button>
        <button class="hint-btn" onclick={handleShuffle}>
          <PS2Btn type="square" />
          <span class:active-shuffle={$isShuffled && isActivePlaylist}>{$t('shuffle')}</span>
        </button>
        <button class="hint-btn" onclick={() => { playUiSfx('confirm'); toggleRepeat(); }}>
          <PS2Btn type="triangle" />
          <span class:active-repeat={$repeatMode !== 'none'} style="display:inline-block;min-width:58px">{$repeatMode === 'one' ? $t('repeatOne') : $repeatMode === 'all' ? $t('repeatAll') : $t('repeat')}</span>
        </button>

        <div class="hints-sep"></div>

        <button class="hint-btn hint-btn--shoulder" onclick={handlePrev} disabled={!$currentTrack}>
          <span class="shoulder-tag">L1</span>
          <span class="nav-icon">&lt;&lt;</span>
          <span>{$t('prev')}</span>
        </button>
        <button class="hint-btn hint-btn--shoulder" onclick={handleNext} disabled={!$currentTrack}>
          <span class="shoulder-tag">R1</span>
          <span class="nav-icon">&gt;&gt;</span>
          <span>{$t('next')}</span>
        </button>
      {/if}
    </div>
    <div class="hints-row hints-row--m3u">
      {#if currentPlaylist.tracks.length > 0}
        <button class="hint-btn hint-btn--m3u" onclick={handleExport}>
          <svg viewBox="0 0 16 16" width="10" height="10" fill="currentColor" style="display: inline-block; vertical-align: middle; margin-right: 2px;">
            <path fill-rule="evenodd" d="M8 1a.5.5 0 0 1 .5.5v11.793l3.146-3.147a.5.5 0 0 1 .708.708l-4 4a.5.5 0 0 1-.708 0l-4-4a.5.5 0 0 1 .708-.708L7.5 13.293V1.5A.5.5 0 0 1 8 1z"/>
          </svg>
          {$t('exportM3U')}
        </button>
        <span class="m3u-sep">·</span>
      {/if}
      <button class="hint-btn hint-btn--m3u" onclick={handleImport}>
        <svg viewBox="0 0 16 16" width="10" height="10" fill="currentColor" style="display: inline-block; vertical-align: middle; margin-right: 2px;">
          <path fill-rule="evenodd" d="M8 15a.5.5 0 0 0 .5-.5V2.707l3.146 3.147a.5.5 0 0 0 .708-.708l-4-4a.5.5 0 0 0-.708 0l-4 4a.5.5 0 1 0 .708.708L7.5 2.707V14.5a.5.5 0 0 0 .5.5z"/>
        </svg>
        {$t('importM3U')}
      </button>
      {#if m3uStatus}
        <span class="m3u-status">{m3uStatus}</span>
      {/if}
    </div>
    <div class="hints-row hints-row--volume">
      <VolumeControl />
    </div>
  </div>
</div>

<style>
  .overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 24px;
    z-index: 100;
    animation: fade-in 0.2s ease;
  }

  @keyframes fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .view {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    animation: slide-in 0.32s cubic-bezier(0.34, 1.4, 0.64, 1);
  }

  @keyframes slide-in {
    from { opacity: 0; transform: perspective(900px) rotateY(-30deg) scale(0.88); }
    to   { opacity: 1; transform: perspective(900px) rotateY(0deg) scale(1); }
  }

  .cover-placeholder {
    width: 260px;
    height: 260px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 64px;
    color: rgba(90, 95, 120, 0.3);
    background: rgba(90, 95, 120, 0.18);
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: 280px;
  }

  .playlist-meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
    align-items: center;
    text-align: center;
  }

  .playlist-title {
    font-size: 19px;
    color: var(--track-active);
    line-height: 1.2;
    margin: 0;
  }

  .playlist-count {
    font-size: 11px;
    color: var(--text-dim);
    margin: 0;
  }

  .empty-hint {
    font-size: 12px;
    color: var(--text-dim);
    text-align: center;
    margin-top: 20px;
  }

  .tracklist {
    list-style: none;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    max-height: 220px;
  }

  .tracklist::-webkit-scrollbar { width: 3px; }
  .tracklist::-webkit-scrollbar-thumb { background: var(--text-dim); }

  .track { display: flex; align-items: center; border-top: 2px solid transparent; }

  .track.dragging { opacity: 0.35; }

  .track.drop-target { border-top-color: var(--track-active); }

  .drag-handle {
    font-size: 11px;
    color: var(--text-dim);
    opacity: 0;
    cursor: grab;
    flex-shrink: 0;
    padding: 0 3px 0 2px;
    user-select: none;
    transition: opacity 0.12s;
  }

  .tracklist:hover .drag-handle { opacity: 0.4; }
  .drag-handle:hover { opacity: 1 !important; color: var(--text-primary); }

  .track-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    cursor: pointer;
    padding: 5px 6px;
    text-align: left;
    transition: background 0.1s;
  }

  .track-btn:hover .track-title,
  .track-btn:hover .track-num,
  .track-btn:hover .track-dur { color: var(--track-hover); }

  .track-num {
    font-size: 10px;
    color: var(--text-dim);
    width: 18px;
    flex-shrink: 0;
    text-align: right;
    transition: color 0.12s;
  }

  .playing-dot { color: var(--track-active); font-size: 9px; }

  .track-title {
    flex: 1;
    font-size: 12px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: color 0.12s;
  }

  .track-artist {
    font-size: 10px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 70px;
    flex-shrink: 0;
    transition: color 0.12s;
  }

  .track.active .track-title,
  .track.active .track-num,
  .track.active .track-dur { color: var(--track-active); }

  .track-dur { font-size: 10px; color: var(--text-dim); flex-shrink: 0; transition: color 0.12s; }

  .hints {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }

  .hints-row {
    display: flex;
    align-items: center;
    gap: 18px;
    justify-content: center;
  }

  .hints-row--volume { gap: 0; }

  .hint-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 12px;
    color: var(--text-secondary);
    padding: 0;
    transition: color 0.15s;
  }

  .hint-btn:hover { color: var(--text-primary); }

  .play-pause-text {
    display: inline-block;
    min-width: 5ch;
    text-align: left;
  }

  .active-shuffle { color: var(--track-active); }
  .active-repeat  { color: var(--track-active); }

  .hints-sep {
    width: 1px;
    height: 20px;
    background: var(--text-dim);
    opacity: 0.3;
  }

  .hint-btn--shoulder {
    gap: 6px;
    padding: 3px 8px;
    border-radius: 999px;
    background: linear-gradient(180deg, rgb(48, 48, 48), rgb(54, 58, 68));
    border: 1px solid rgba(212, 219, 240, 0.1);
    box-shadow:
      0 2px 6px rgba(0, 0, 0, 0.18),
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      inset 0 -1px 2px rgba(0, 0, 0, 0.25);
  }

  .hint-btn:disabled { opacity: 0.35; cursor: default; }

  .hints-row--m3u {
    gap: 10px;
  }

  .hint-btn--m3u {
    font-size: 10px;
    letter-spacing: 0.04em;
    color: var(--text-dim);
    opacity: 0.7;
    transition: color 0.15s, opacity 0.15s;
  }

  .hint-btn--m3u:hover {
    color: var(--text-primary);
    opacity: 1;
  }

  .m3u-sep {
    color: var(--text-dim);
    opacity: 0.4;
    font-size: 10px;
  }

  .m3u-status {
    font-size: 10px;
    color: var(--track-active);
    animation: fade-in 0.15s ease;
  }

  .shoulder-tag {
    font-size: 9px;
    letter-spacing: 0.08em;
    color: rgba(238, 242, 255, 0.82);
    text-shadow: none;
  }

  .nav-icon {
    font-size: 11px;
    font-weight: 900;
    color: var(--track-hover);
    text-shadow: none;
  }
</style>
