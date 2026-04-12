<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import ProgressBar from './ProgressBar.svelte';
  import {
    currentTrack,
    currentAlbum,
    isPlaying,
    isShuffled,
    repeatMode,
    toggleRepeat,
    playShuffledAll,
    pause,
    resume,
    playNext,
    playPrev,
    volume,
    setVolume,
  } from '$lib/stores/player';
  import { albums } from '$lib/stores/library';
  import { playUiSfx } from '$lib/ui-sfx';

  let pinned = $state(false);

  const coverSrc = $derived(
    $currentAlbum?.cover_art ? convertFileSrc($currentAlbum.cover_art) : null
  );

  let bgColor = $state('rgb(14, 16, 28)');

  $effect(() => {
    if (coverSrc) {
      extractBg(coverSrc).then((c) => (bgColor = c));
    } else {
      bgColor = 'rgb(14, 16, 28)';
    }
  });

  async function extractBg(src: string): Promise<string> {
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
      r = Math.round(r / px * 0.35 + 8);
      g = Math.round(g / px * 0.35 + 8);
      b = Math.round(b / px * 0.35 + 8);
      return `rgb(${r}, ${g}, ${b})`;
    } catch {
      return 'rgb(14, 16, 28)';
    }
  }

  async function handlePlayPause() {
    if ($isPlaying) await pause(); else await resume();
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

  async function togglePin() {
    pinned = !pinned;
    await getCurrentWindow().setAlwaysOnTop(pinned);
    playUiSfx('confirm');
  }

  async function handleToggleRepeat() {
    playUiSfx('confirm');
    toggleRepeat();
  }

  async function handleToggleShuffle() {
    playUiSfx('confirm');
    await playShuffledAll($albums);
  }

  // Compact volume — same curve as VolumeControl
  const VOL_STEPS = 20;
  const VOL_CURVE = 1.7;
  const VOL_BARS  = 5;

  function stepToVol(n: number): number {
    if (n === 0) return 0;
    return Math.pow(n / VOL_STEPS, VOL_CURVE);
  }

  function volToStep(v: number): number {
    if (v <= 0) return 0;
    return Math.round(Math.pow(v, 1 / VOL_CURVE) * VOL_STEPS);
  }

  let volLevel = $derived(volToStep($volume));

  function setVolLevel(n: number) {
    setVolume(stepToVol(Math.max(0, Math.min(VOL_STEPS, n))));
  }

  function onVolWheel(e: WheelEvent) {
    e.preventDefault();
    setVolLevel(volLevel + (e.deltaY < 0 ? 1 : -1));
  }
</script>

<div class="mini-root" style="background: {bgColor}">

  <!-- ── Top: art + info column (title / artist / seek bar) ── -->
  <div class="mini-top" data-tauri-drag-region>
    <div class="mini-art">
      {#if coverSrc}
        <img src={coverSrc} alt="" />
      {:else}
        <span class="mini-art-ph">♪</span>
      {/if}
    </div>

    <div class="mini-info-col">
      <div class="mini-meta" data-tauri-drag-region>
        <span class="mini-track">{$currentTrack?.title ?? '—'}</span>
        <span class="mini-artist">{$currentTrack?.artist ?? ''}</span>
      </div>
      <div class="mini-seek">
        <ProgressBar />
      </div>
    </div>
  </div>

  <!-- ── Bottom: transport controls + volume + pin ── -->
  <div class="mini-bottom">

    <!-- Transport -->
    <div class="mini-controls">

      <!-- Shuffle -->
      <button
        class="mini-btn"
        class:active={$isShuffled}
        onclick={handleToggleShuffle}
        disabled={!$currentTrack}
        title="Shuffle"
      >
        <svg viewBox="0 0 10 10">
          <path d="M1,3.5 H3.5 L7,6.5 H9.5" stroke="currentColor" fill="none" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
          <polyline points="8,5.5 9.5,6.5 8,7.5" stroke="currentColor" fill="none" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M1,6.5 H3.5 L7,3.5 H9.5" stroke="currentColor" fill="none" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
          <polyline points="8,2.5 9.5,3.5 8,4.5" stroke="currentColor" fill="none" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>

      <!-- Prev -->
      <button class="mini-btn" onclick={handlePrev} disabled={!$currentTrack} title="Previous">
        <svg viewBox="0 0 10 10" fill="currentColor">
          <rect x="1" y="1.5" width="1.5" height="7" rx="0.5"/>
          <polygon points="3.5,5 9,1.5 9,8.5"/>
        </svg>
      </button>

      <!-- Play / Pause -->
      <button class="mini-btn mini-btn--play" onclick={handlePlayPause} disabled={!$currentTrack}>
        {#if $isPlaying}
          <svg viewBox="0 0 10 10" fill="currentColor">
            <rect x="1.5" y="1" width="2.5" height="8" rx="0.5"/>
            <rect x="5.5" y="1" width="2.5" height="8" rx="0.5"/>
          </svg>
        {:else}
          <svg viewBox="0 0 10 10" fill="currentColor">
            <polygon points="2,1 9,5 2,9"/>
          </svg>
        {/if}
      </button>

      <!-- Next -->
      <button class="mini-btn" onclick={handleNext} disabled={!$currentTrack} title="Next">
        <svg viewBox="0 0 10 10" fill="currentColor">
          <polygon points="1,1.5 6.5,5 1,8.5"/>
          <rect x="7.5" y="1.5" width="1.5" height="7" rx="0.5"/>
        </svg>
      </button>

      <!-- Repeat -->
      <button
        class="mini-btn"
        class:active={$repeatMode !== 'none'}
        onclick={handleToggleRepeat}
        disabled={!$currentTrack}
        title="Repeat"
      >
        <svg viewBox="0 0 10 10">
          <path d="M2,3 H7.5" stroke="currentColor" fill="none" stroke-width="1.2" stroke-linecap="round"/>
          <polyline points="6.5,1.8 7.5,3 6.5,4.2" stroke="currentColor" fill="none" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M8,7 H2.5" stroke="currentColor" fill="none" stroke-width="1.2" stroke-linecap="round"/>
          <polyline points="3.5,5.8 2.5,7 3.5,8.2" stroke="currentColor" fill="none" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
          {#if $repeatMode === 'one'}
            <text x="5" y="5.8" font-size="3.5" text-anchor="middle" fill="currentColor" stroke="none">1</text>
          {/if}
        </svg>
      </button>
    </div>

    <!-- Compact volume (wheel + click on bars) -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="mini-vol" onwheel={onVolWheel}>
      <span class="mini-vol-label">VOL</span>
      <div class="mini-vol-bars">
        {#each Array(VOL_BARS) as _, i}
          {@const barFull = (i + 1) * 4}
          {@const barHalfMin = i * 4 + 1}
          <button
            class="mini-vol-bar"
            class:filled={volLevel >= barFull}
            class:half={volLevel >= barHalfMin && volLevel < barFull}
            onclick={() => setVolLevel(barFull)}
            style="height: {5 + i * 2.8}px"
            aria-label="Volume bar {i + 1}"
          ></button>
        {/each}
      </div>
    </div>

    <!-- Pin / always-on-top -->
    <button class="mini-pin-btn" class:pinned onclick={togglePin} title={pinned ? 'Unpin' : 'Pin on top'}>
      <svg viewBox="0 0 10 10" fill="currentColor">
        <path d="M5,0.5 L7.5,3 L6.5,4 L8.5,7 L6,7 L6,9.5 L4,9.5 L4,7 L1.5,7 L3.5,4 L2.5,3 Z"/>
      </svg>
    </button>
  </div>
</div>

<style>
  .mini-root {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    transition: background 0.4s ease;
  }

  /* ── Top section ── */
  .mini-top {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px 4px;
    min-height: 0;
  }

  .mini-art {
    width: 52px;
    height: 52px;
    flex-shrink: 0;
    border-radius: 3px;
    overflow: hidden;
    background: rgba(90, 95, 120, 0.2);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .mini-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .mini-art-ph {
    font-size: 20px;
    color: rgba(90, 95, 120, 0.5);
  }

  .mini-info-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .mini-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .mini-track {
    font-size: 11px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .mini-artist {
    font-size: 9px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Seek bar — make it fill the available column width */
  .mini-seek :global(.progress-wrap) {
    width: 100%;
    gap: 4px;
  }

  .mini-seek :global(.bar) {
    flex: 1;
    width: auto !important;
    min-width: 0;
  }

  .mini-seek :global(.time) {
    font-size: 9px;
    min-width: 24px;
  }

  /* ── Bottom section ── */
  .mini-bottom {
    display: flex;
    align-items: center;
    padding: 0 8px 6px;
    gap: 4px;
  }

  .mini-controls {
    display: flex;
    align-items: center;
    gap: 1px;
    margin-right: auto;
  }

  .mini-btn {
    background: none;
    border: none;
    cursor: pointer;
    padding: 5px;
    color: rgba(255, 255, 255, 0.5);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color 0.12s, background 0.12s;
  }

  .mini-btn svg {
    width: 13px;
    height: 13px;
    display: block;
  }

  .mini-btn:hover:not(:disabled) {
    color: rgba(255, 255, 255, 0.95);
    background: rgba(255, 255, 255, 0.08);
  }

  .mini-btn:disabled {
    opacity: 0.15;
    cursor: default;
  }

  .mini-btn.active {
    color: var(--track-active);
    filter: drop-shadow(0 0 2px var(--track-active));
  }

  .mini-btn--play {
    color: var(--track-active);
  }

  .mini-btn--play svg {
    width: 15px;
    height: 15px;
  }

  /* ── Compact volume ── */
  .mini-vol {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 5px;
    cursor: default;
    flex-shrink: 0;
  }

  .mini-vol-label {
    font-size: 8px;
    font-weight: 800;
    color: var(--text-dim);
    letter-spacing: 0.08em;
  }

  .mini-vol-bars {
    display: flex;
    align-items: flex-end;
    gap: 2px;
  }

  .mini-vol-bar {
    width: 4px;
    background: var(--text-dim);
    border: none;
    cursor: pointer;
    padding: 0;
    opacity: 0.25;
    transition: opacity 0.1s, background 0.1s;
    border-radius: 1px;
  }

  .mini-vol-bar.filled {
    opacity: 1;
    background: linear-gradient(180deg, #7fd0ff, #3b79ff);
    box-shadow: 0 0 4px rgba(86, 143, 255, 0.25);
  }

  .mini-vol-bar.half {
    opacity: 0.45;
    background: linear-gradient(180deg, #7fd0ff, #3b79ff);
  }

  /* ── Pin button ── */
  .mini-pin-btn {
    background: none;
    border: none;
    cursor: pointer;
    padding: 5px;
    color: rgba(255, 255, 255, 0.28);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color 0.12s;
    flex-shrink: 0;
  }

  .mini-pin-btn svg {
    width: 11px;
    height: 11px;
    display: block;
  }

  .mini-pin-btn:hover {
    color: rgba(255, 255, 255, 0.7);
  }

  .mini-pin-btn.pinned {
    color: var(--track-active);
  }
</style>
