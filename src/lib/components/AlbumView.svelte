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
  } from '../stores/player';
  import VolumeControl from './VolumeControl.svelte';
  import SpinningCover from './SpinningCover.svelte';
  import PS2Btn from './PS2Btn.svelte';

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

  function extractDominantColor(src: string): Promise<string> {
    return new Promise((resolve) => {
      const img = new Image();
      img.crossOrigin = 'anonymous';
      img.onload = () => {
        const canvas = document.createElement('canvas');
        canvas.width = 8;
        canvas.height = 8;
        const ctx = canvas.getContext('2d')!;
        ctx.drawImage(img, 0, 0, 8, 8);
        const data = ctx.getImageData(0, 0, 8, 8).data;
        let r = 0, g = 0, b = 0;
        const px = data.length / 4;
        for (let i = 0; i < data.length; i += 4) {
          r += data[i]; g += data[i + 1]; b += data[i + 2];
        }
        resolve(`rgba(${Math.round(r/px)}, ${Math.round(g/px)}, ${Math.round(b/px)}, 0.5)`);
      };
      img.src = src;
    });
  }

  let isActiveAlbum = $derived($currentTrack && album.tracks.some(t => t.id === $currentTrack!.id));

  function formatDuration(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  async function handleTrackClick(track: Track) {
    if ($currentTrack?.id === track.id) {
      if ($isPlaying) await pause(); else await resume();
    } else {
      await playTrack(track, album);
    }
  }

  async function handlePlayPause() {
    if (!isActiveAlbum) await playTrack(album.tracks[0], album);
    else if ($isPlaying) await pause();
    else await resume();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="overlay"
  style="background: {tintColor};"
  onmousedown={(e) => e.target === e.currentTarget && onclose()}
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
        {#each album.tracks as track (track.id)}
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
          </li>
        {/each}
      </ul>
    </div>

  </div>

  <!-- Bottom: gamepad hints (functional) + volume -->
  <div class="hints">
    <button class="hint-btn" onclick={onclose}>
      <PS2Btn type="circle" />
      <span>Back</span>
    </button>
    <button class="hint-btn" onclick={handlePlayPause}>
      <PS2Btn type="cross" />
      <span>{isActiveAlbum && $isPlaying ? 'Pause' : 'Play'}</span>
    </button>
    <button class="hint-btn" onclick={() => playShuffled(album)}>
      <PS2Btn type="square" />
      <span>Shuffle</span>
    </button>

    <div class="hints-sep"></div>

    <button class="hint-btn" onclick={() => playPrev(album)} disabled={!$currentTrack}>
      <span class="nav-icon">⏮</span>
      <span>Prev</span>
    </button>
    <button class="hint-btn" onclick={() => playNext(album)} disabled={!$currentTrack}>
      <span class="nav-icon">⏭</span>
      <span>Next</span>
    </button>

    <div class="hints-sep"></div>
    <VolumeControl />
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

  .track-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
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

  /* ── Bottom hints ── */
  .hints {
    display: flex;
    align-items: center;
    gap: 18px;
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

  .nav-icon { font-size: 13px; }

</style>
