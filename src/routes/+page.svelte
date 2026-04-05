<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import AlbumGrid from '$lib/components/AlbumGrid.svelte';
  import AlbumView from '$lib/components/AlbumView.svelte';
  import VolumeControl from '$lib/components/VolumeControl.svelte';
  import {
    albums,
    isScanning,
    librarySize,
    selectedAlbum,
    scanStatus,
    scanFolder,
  } from '$lib/stores/library';
  import {
    currentTrack,
    currentAlbum,
    isPlaying,
    isPaused,
    pause,
    resume,
  } from '$lib/stores/player';
  import type { Album } from '$lib/types';

  let hoveredAlbum = $state<Album | null>(null);

  async function pickFolder() {
    const path = await invoke<string | null>('pick_folder');
    if (path) await scanFolder(path);
  }

  function selectAlbum(album: Album) {
    selectedAlbum.set(album);
  }

  function openCurrentAlbum() {
    if ($currentAlbum) selectedAlbum.set($currentAlbum);
  }
</script>

<div class="shell">

  <!-- Header -->
  <header class="header">
    <div class="header-left">
      <button class="folder-btn" onclick={pickFolder} title="Choose music folder">
        <span class="folder-icon">⊞</span>
        <span class="memory-label">Memory Card</span>
      </button>
      {#if $librarySize !== '0 MB'}
        <span class="lib-size">{$librarySize}</span>
      {/if}
    </div>

    <div class="header-right">
      {#if $isScanning}
        <span class="scanning">Scanning…</span>
      {/if}
      {#if hoveredAlbum}
        <span class="hovered-title">{hoveredAlbum.title}</span>
      {/if}
    </div>
  </header>

  <!-- Album grid -->
  <main class="content">
    {#if $isScanning && $albums.length === 0}
      <div class="state-msg">
        <div class="spinner"></div>
        <p class="scan-info">
          {#if $scanStatus.filesScanned > 0}
            {$scanStatus.filesScanned} files · {$scanStatus.albumsFound} albums
          {:else}
            Starting scan…
          {/if}
        </p>
      </div>
    {:else if $albums.length === 0}
      <div class="state-msg">
        <p class="hint">Click <strong>Memory Card</strong> to choose a music folder</p>
      </div>
    {:else}
      {#if $isScanning}
        <div class="scan-bar">
          <div class="spinner-sm"></div>
          <span>{$scanStatus.filesScanned} files · {$scanStatus.albumsFound} albums found</span>
        </div>
      {/if}
      <AlbumGrid albums={$albums} onselect={selectAlbum} onhover={(a) => (hoveredAlbum = a)} />
    {/if}
  </main>

  <!-- Footer -->
  <footer class="footer">
    <!-- Now playing -->
    <button
      class="now-playing"
      class:active={!!$currentTrack}
      onclick={openCurrentAlbum}
      disabled={!$currentTrack}
    >
      <div class="now-playing-art">
        {#if $currentAlbum?.cover_art}
          <img src={$currentAlbum.cover_art} alt="" />
        {:else}
          <span>♪</span>
        {/if}
      </div>
      <div class="now-playing-info">
        <span class="track-name">{$currentTrack?.title ?? 'No track playing'}</span>
        <span class="track-artist">{$currentTrack?.artist ?? '—'}</span>
      </div>
      {#if $currentTrack}
        <span class="play-indicator">{$isPlaying ? '⏸' : '▶'}</span>
      {/if}
    </button>

    <!-- Volume -->
    <VolumeControl />

    <!-- PS2 action hints -->
    <div class="actions">
      <div class="action-hint">
        <span class="btn-icon cross">✕</span>
        <span class="btn-label">Select</span>
      </div>
      <div class="action-hint">
        <span class="btn-icon circle">○</span>
        <span class="btn-label">Back</span>
      </div>
      <div class="action-hint">
        <span class="btn-icon square">□</span>
        <span class="btn-label">Shuffle</span>
      </div>
      <div class="action-hint">
        <span class="btn-icon triangle">△</span>
        <span class="btn-label">Options</span>
      </div>
    </div>
  </footer>

</div>

{#if $selectedAlbum}
  <AlbumView album={$selectedAlbum} onclose={() => selectedAlbum.set(null)} />
{/if}

<style>
  .shell {
    width: 100vw;
    height: 100vh;
    display: grid;
    grid-template-rows: auto 1fr auto;
    padding: 18px 24px 14px;
  }

  /* ── Header ── */
  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-bottom: 4px;
  }

  .header-left {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }

  .folder-btn {
    background: none;
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0;
    transition: opacity 0.15s;
  }

  .folder-btn:hover { opacity: 0.7; }

  .folder-icon {
    font-size: 16px;
    color: var(--text-secondary);
  }

  .memory-label {
    font-size: 22px;
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: 0.01em;
  }

  .lib-size {
    font-size: 13px;
    color: var(--text-secondary);
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .scanning {
    font-size: 12px;
    color: var(--text-dim);
    letter-spacing: 0.05em;
  }

  .hovered-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: 0.01em;
    animation: fadein 0.15s ease;
  }

  @keyframes fadein {
    from { opacity: 0; transform: translateY(-3px); }
    to   { opacity: 1; transform: translateY(0); }
  }

  /* ── Content ── */
  .content {
    overflow: hidden;
    padding: 10px 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .state-msg {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    color: var(--text-dim);
  }

  .hint { font-size: 14px; }
  .hint strong { color: var(--text-secondary); }

  .scan-info {
    font-size: 12px;
    color: var(--text-dim);
    letter-spacing: 0.04em;
  }

  .scan-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 2px 12px;
    font-size: 12px;
    color: var(--text-dim);
  }

  .spinner-sm {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(90, 95, 120, 0.2);
    border-top-color: var(--text-secondary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  .spinner {
    width: 28px;
    height: 28px;
    border: 2px solid rgba(90, 95, 120, 0.2);
    border-top-color: var(--text-secondary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Footer ── */
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 8px;
  }

  .now-playing {
    display: flex;
    align-items: center;
    gap: 12px;
    background: var(--card-bg);
    border: none;
    border-radius: 12px;
    padding: 10px 14px 10px 10px;
    cursor: pointer;
    box-shadow: var(--btn-shadow);
    backdrop-filter: blur(12px);
    transition: box-shadow 0.2s, transform 0.15s, opacity 0.2s;
    max-width: 280px;
  }

  .now-playing:disabled { opacity: 0.45; cursor: default; }
  .now-playing:not(:disabled):hover {
    box-shadow: var(--card-shadow-hover);
    transform: translateY(-1px);
  }

  .now-playing-art {
    width: 40px;
    height: 40px;
    border-radius: 7px;
    background: rgba(90, 95, 120, 0.15);
    flex-shrink: 0;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 18px;
    color: var(--text-dim);
  }

  .now-playing-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .now-playing-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .track-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 160px;
  }

  .track-artist {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .play-indicator {
    font-size: 14px;
    color: var(--text-secondary);
    margin-left: 4px;
    flex-shrink: 0;
  }

  /* PS2 buttons */
  .actions {
    display: flex;
    align-items: center;
    gap: 20px;
  }

  .action-hint {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .btn-icon {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 600;
    flex-shrink: 0;
    box-shadow: 0 1px 4px rgba(0,0,0,0.18);
  }

  .btn-label {
    font-size: 11px;
    color: var(--text-secondary);
    letter-spacing: 0.03em;
  }

  .cross    { background: #4a90d9; color: #fff; }
  .circle   { background: #d94a4a; color: #fff; }
  .square   { background: #d94aaa; color: #fff; }
  .triangle { background: #4aad6e; color: #fff; }
</style>
