<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { Album, Track } from '../types';
  import {
    currentTrack,
    isPlaying,
    playTrack,
    playShuffled,
    playNext,
    playPrev,
    pause,
    resume,
    isShuffled,
    repeatMode,
    toggleRepeat,
  } from '../stores/player';
  import VolumeControl from './VolumeControl.svelte';
  import SpinningCover from './SpinningCover.svelte';
  import PS2Btn from './PS2Btn.svelte';
  import PlaylistPicker from './PlaylistPicker.svelte';
  import { playUiSfx } from '$lib/ui-sfx';
  import { t } from '$lib/stores/i18n';

  let {
    album,
    onclose,
  }: {
    album: Album;
    onclose: () => void;
  } = $props();

  let tintColor = $state('rgba(120, 120, 140, 0.28)');

  const coverSrc = $derived(album.cover_art ? convertFileSrc(album.cover_art) : null);

  $effect(() => {
    if (coverSrc) {
      extractDominantColor(coverSrc).then((c) => (tintColor = c));
    } else {
      tintColor = 'rgba(120, 120, 140, 0.28)';
    }
  });

  async function extractDominantColor(src: string): Promise<string> {
    const fallback = 'rgba(120, 120, 140, 0.28)';
    try {
      // createImageBitmap with resize decodes off the main thread — no freeze
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
      return `rgba(${Math.round(r / px)}, ${Math.round(g / px)}, ${Math.round(b / px)}, 0.5)`;
    } catch {
      return fallback;
    }
  }

  let isActiveAlbum = $derived($currentTrack && album.tracks.some(t => t.id === $currentTrack!.id));

  // Group tracks by disc number — used to render disc separators
  let discGroups = $derived((() => {
    const groups: { disc: number; tracks: Track[] }[] = [];
    for (const track of album.tracks) {
      const last = groups[groups.length - 1];
      if (last && last.disc === track.disc_number) {
        last.tracks.push(track);
      } else {
        groups.push({ disc: track.disc_number, tracks: [track] });
      }
    }
    return groups;
  })());
  let hasMultipleDiscs = $derived(discGroups.length > 1);

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
      await playTrack(track, album);
    }
  }

  async function handlePlayPause() {
    if (!isActiveAlbum) await playTrack(album.tracks[0], album);
    else if ($isPlaying) await pause();
    else await resume();
  }

  async function handlePrev() {
    playUiSfx('nextPrev');
    await playPrev(album);
  }

  async function handleNext() {
    playUiSfx('nextPrev');
    await playNext(album);
  }

  async function handleShuffle() {
    playUiSfx('confirm');
    await playShuffled(album);
  }

  function handleClose() {
    playUiSfx('back');
    onclose();
  }

  function handleOverlayMouseDown(e: MouseEvent) {
    if (e.target === e.currentTarget) handleClose();
  }

  let pickerTrack = $state<Track | null>(null);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="overlay"
  style="background: {tintColor};"
  onmousedown={handleOverlayMouseDown}
>
  <div class="view">

    <!-- Always-spinning cover -->
    {#if coverSrc}
      <SpinningCover src={coverSrc} alt={album.title} />
    {:else}
      <div class="cover-placeholder">♪</div>
    {/if}

    <!-- Info + tracklist only -->
    <div class="panel">
      <div class="album-meta">
        <h2 class="album-title">{album.title}</h2>
        <p class="album-artist">{album.artist}</p>
        {#if album.year}<p class="album-year">{album.year}</p>{/if}
      </div>

      <ul class="tracklist">
        {#each discGroups as group}
          {#if hasMultipleDiscs}
            <li class="disc-header">{$t('disc', group.disc)}</li>
          {/if}
          {#each group.tracks as track (track.id)}
            {@const active = $currentTrack?.id === track.id}
            <li class="track" class:active>
              <button class="track-btn" onclick={() => handleTrackClick(track)}>
                <span class="track-num">
                  {#if active && $isPlaying}
                    <span class="playing-dot">▶</span>
                  {:else}
                    {track.track_number || '—'}
                  {/if}
                </span>
                <span class="track-title">{track.title}</span>
                <span class="track-dur">{formatDuration(track.duration)}</span>
              </button>
              <button
                class="track-add-btn"
                onclick={(e) => { e.stopPropagation(); pickerTrack = track; }}
                title="Add to playlist"
              >+</button>
            </li>
          {/each}
        {/each}
      </ul>
    </div>

  </div>

  {#if pickerTrack}
    <PlaylistPicker track={pickerTrack} onclose={() => pickerTrack = null} />
  {/if}

  <!-- Bottom: gamepad hints (functional) + volume -->
  <div class="hints">
    <div class="hints-row">
      <button class="hint-btn" onclick={handleClose}>
        <PS2Btn type="circle" />
        <span>{$t('back')}</span>
      </button>
      <button class="hint-btn" onclick={handlePlayPause}>
        <PS2Btn type="cross" />
        <span class="play-pause-text">{isActiveAlbum && $isPlaying ? $t('pause') : $t('play')}</span>
      </button>
      <button class="hint-btn" onclick={handleShuffle}>
        <PS2Btn type="square" />
        <span class:active-shuffle={$isShuffled}>{$t('shuffle')}</span>
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

  /* ── Cover placeholder (no art) ── */
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

  /* ── Panel ── */
  .panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: 280px;
  }

  .album-meta { display: flex; flex-direction: column; gap: 3px; align-items: center; text-align: center; }

  .album-title {
    font-size: 19px;
    color: var(--track-active);
    line-height: 1.2;
    margin: 0;
  }

  .album-artist { font-size: 13px; color: var(--text-secondary); margin: 0; }
  .album-year   { font-size: 11px; color: var(--text-dim); margin: 0; }

  /* ── Disc header ── */
  .disc-header {
    font-size: 9px;
    letter-spacing: 0.18em;
    color: var(--text-dim);
    padding: 8px 6px 3px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    margin-top: 2px;
    list-style: none;
  }
  .disc-header:first-child {
    border-top: none;
    margin-top: 0;
    padding-top: 2px;
  }

  /* ── Tracklist ── */
  .tracklist {
    list-style: none;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    max-height: 220px;
  }

  .tracklist::-webkit-scrollbar { width: 3px; }
  .tracklist::-webkit-scrollbar-thumb { background: var(--text-dim); }

  .track {
    display: flex;
    align-items: center;
  }

  .track-btn {
    display: flex;
    align-items: center;
    gap: 10px;
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

  .track.active .track-btn { background: none; }

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

  .track.active .track-title,
  .track.active .track-num,
  .track.active .track-dur { color: var(--track-active); }

  .track-dur { font-size: 10px; color: var(--text-dim); flex-shrink: 0; transition: color 0.12s; }

  .track-add-btn {
    flex-shrink: 0;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 14px;
    font-weight: 800;
    color: var(--text-dim);
    padding: 0 6px 0 2px;
    line-height: 1;
    opacity: 0;
    transition: opacity 0.12s, color 0.12s;
  }

  .track:hover .track-add-btn {
    opacity: 1;
  }

  .track-add-btn:hover { color: var(--track-hover); }

  /* ── Bottom hints ── */
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
    flex-wrap: wrap;
  }

  .hints-row--volume {
    gap: 0;
  }

  .hints-sep {
    width: 1px;
    height: 20px;
    background: var(--text-dim);
    opacity: 0.3;
  }

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

  .hint-btn:hover:not(:disabled) { color: var(--text-primary); }
  .hint-btn:disabled { opacity: 0.35; cursor: default; }

  .play-pause-text {
    display: inline-block;
    min-width: 5ch;
    text-align: left;
  }

  .active-shuffle { color: var(--track-active); }
  .active-repeat  { color: var(--track-active); }

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

  .shoulder-tag,
  .nav-icon {
    text-shadow: none;
  }

  .shoulder-tag {
    font-size: 9px;
    letter-spacing: 0.08em;
    color: rgba(238, 242, 255, 0.82);
  }

  .nav-icon {
    font-size: 11px;
    font-weight: 900;
    color: var(--track-hover);
  }

</style>
