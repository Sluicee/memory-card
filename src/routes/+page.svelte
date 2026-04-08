<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import AlbumGrid from '$lib/components/AlbumGrid.svelte';
  import AlbumView from '$lib/components/AlbumView.svelte';
  import PlaylistGrid from '$lib/components/PlaylistGrid.svelte';
  import PlaylistView from '$lib/components/PlaylistView.svelte';
  import VolumeControl from '$lib/components/VolumeControl.svelte';
  import ProgressBar from '$lib/components/ProgressBar.svelte';
  import PS2Btn from '$lib/components/PS2Btn.svelte';
  import OptionsMenu from '$lib/components/OptionsMenu.svelte';
  import StatsView from '$lib/components/StatsView.svelte';
  import PlaylistPicker from '$lib/components/PlaylistPicker.svelte';
  import { playUiSfx, primeUiSfx } from '$lib/ui-sfx';
  import { onMount } from 'svelte';
  import {
    albums,
    isScanning,
    librarySize,
    selectedAlbum,
    scanStatus,
    loadCache,
  } from '$lib/stores/library';
  import {
    currentTrack,
    currentAlbum,
    isPlaying,
    isPaused,
    pause,
    resume,
    playNext,
    playPrev,
    playShuffledAll,
    isShuffled,
    initVolume,
    loadLastTrack,
  } from '$lib/stores/player';
  import { checkForUpdates } from '$lib/stores/updates';
  import { currentTrack as ct, currentAlbum as ca, duration } from '$lib/stores/player';
  import { playlists } from '$lib/stores/playlists';
  import { currentPlaylistId } from '$lib/stores/player';
  import type { Album } from '$lib/types';
  import type { Playlist } from '$lib/stores/playlists';

  let activeTab         = $state<'library' | 'playlists'>('library');
  let hoveredAlbum      = $state<Album | null>(null);
  let hoveredPlaylist   = $state<Playlist | null>(null);
  let selectedPlaylist  = $state<Playlist | null>(null);
  let optionsOpen       = $state(false);
  let statsOpen         = $state(false);
  let npPickerOpen      = $state(false);
  let searchOpen        = $state(false);
  let searchQuery       = $state('');
  let searchInput       = $state<HTMLInputElement | null>(null);
  let followPlayback    = $state(false);

  const filteredAlbums = $derived(
    searchOpen && searchQuery.trim()
      ? (() => {
          const q = searchQuery.trim().toLowerCase();
          return $albums.filter(a =>
            a.title.toLowerCase().includes(q) ||
            a.artist.toLowerCase().includes(q) ||
            a.tracks.some(t =>
              t.title.toLowerCase().includes(q) ||
              t.artist.toLowerCase().includes(q)
            )
          );
        })()
      : $albums
  );

  function toggleSearch() {
    const opening = !searchOpen;
    searchOpen = opening;
    playUiSfx(opening ? 'open' : 'back');
    if (!searchOpen) { searchQuery = ''; }
    else setTimeout(() => searchInput?.focus(), 30);
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && searchOpen) {
      playUiSfx('back');
      searchOpen = false;
      searchQuery = '';
    }
  }

  onMount(async () => {
    primeUiSfx();

    // Restore volume to audio backend
    await initVolume();

    // Restore last track display (no autoplay)
    const last = loadLastTrack();
    if (last) {
      ct.set(last.track);
      ca.set(last.album);
      duration.set(last.track.duration);
    }

    // Load cached library (no rescan)
    await loadCache();

    // Check for updates in background
    checkForUpdates();
  });

  function selectAlbum(album: Album) {
    playUiSfx('confirm');
    followPlayback = false;
    selectedAlbum.set(album);
  }

  async function handleTransportPlayPause() {
    if ($isPlaying) await pause();
    else await resume();
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

  async function handleShuffleAll() {
    playUiSfx('confirm');
    await playShuffledAll($albums);
  }

  function openOptions() {
    playUiSfx('open');
    optionsOpen = true;
  }

  function openCurrentContext() {
    if ($currentPlaylistId) {
      const pl = $playlists.find((p) => p.id === $currentPlaylistId);
      if (pl) {
        playUiSfx('confirm');
        activeTab = 'playlists';
        selectedPlaylist = pl;
      }
    } else if ($currentAlbum) {
      playUiSfx('confirm');
      followPlayback = true;
      selectedAlbum.set($currentAlbum);
    }
  }

  $effect(() => {
    if (followPlayback && $selectedAlbum && $currentAlbum && $selectedAlbum !== $currentAlbum) {
      selectedAlbum.set($currentAlbum);
    }
  });
</script>

<div class="root">
<div class="shell">

  <!-- Header -->
  <header class="header">
    <div class="header-left">
      <div class="mc-card"></div>
      <div class="memory-block">
        <span class="memory-label">Memory Card</span>
        {#if $librarySize !== '0 MB'}
          <span class="lib-size">{$librarySize}</span>
        {/if}
      </div>
    </div>

    <div class="header-right">
      {#if searchOpen && activeTab === 'library'}
        <input
          bind:this={searchInput}
          bind:value={searchQuery}
          onkeydown={onSearchKey}
          class="search-input"
          placeholder="Search…"
          autocomplete="off"
          spellcheck="false"
        />
      {:else if $isScanning}
        <span class="scanning">Scanning…</span>
      {/if}
      {#if activeTab === 'library' && hoveredAlbum}
        <span class="hovered-title" class:hovered-title--small={searchOpen}>{hoveredAlbum.title}</span>
      {:else if activeTab === 'playlists' && hoveredPlaylist}
        <span class="hovered-title">{hoveredPlaylist.name}</span>
      {/if}
    </div>
  </header>

  <!-- Tab switcher -->
  <div class="tab-toggle">
    <div class="tab-thumb" class:tab-thumb--right={activeTab === 'playlists'}></div>
    <button
      class="tab-opt"
      class:tab-opt--active={activeTab === 'library'}
      onclick={() => { activeTab = 'library'; playUiSfx('back'); }}
    >Library</button>
    <button
      class="tab-opt"
      class:tab-opt--active={activeTab === 'playlists'}
      onclick={() => { activeTab = 'playlists'; playUiSfx('confirm'); }}
    >Playlists</button>
  </div>

  <!-- Content -->
  <main class="content">
    {#if activeTab === 'library'}
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
          <p class="hint">Select <strong>Options</strong> to choose a music folder</p>
        </div>
      {:else}
        {#if $isScanning}
          <div class="scan-bar">
            <div class="spinner-sm"></div>
            <span>{$scanStatus.filesScanned} files · {$scanStatus.albumsFound} albums found</span>
          </div>
        {/if}
        {#if searchOpen && searchQuery && filteredAlbums.length === 0}
          <div class="state-msg"><p class="hint">No results for <strong>{searchQuery}</strong></p></div>
        {:else}
          <AlbumGrid albums={filteredAlbums} onselect={selectAlbum} onhover={(a) => (hoveredAlbum = a)} />
        {/if}
      {/if}
    {:else}
      <PlaylistGrid
        playlists={$playlists}
        onselect={(pl) => { playUiSfx('confirm'); selectedPlaylist = pl; }}
        onhover={(pl) => (hoveredPlaylist = pl)}
      />
    {/if}
  </main>

  <!-- Footer -->
  <footer class="footer">
    <!-- Row 1: progress -->
    <div class="footer-progress">
      <ProgressBar />
    </div>

    <!-- Row 2: transport + volume -->
    <div class="footer-top">
      <div class="transport">
        <button
          class="transport-btn transport-btn--shoulder"
          onclick={handlePrev}
          disabled={!$currentTrack}
          title="Previous"
        >
          <span class="transport-tag">L1</span>
          <span class="transport-icon">&lt;&lt;</span>
          <span class="transport-text">Prev</span>
        </button>
        <button
          class="transport-btn play-btn"
          onclick={handleTransportPlayPause}
          disabled={!$currentTrack}
          title={$isPlaying ? 'Pause' : 'Play'}
        >
          <PS2Btn type="start" />
          <span class="transport-text play-pause-text">{$isPlaying ? 'Pause' : 'Play'}</span>
        </button>
        <button
          class="transport-btn transport-btn--shoulder"
          onclick={handleNext}
          disabled={!$currentTrack}
          title="Next"
        >
          <span class="transport-tag">R1</span>
          <span class="transport-icon">&gt;&gt;</span>
          <span class="transport-text">Next</span>
        </button>
      </div>
      <VolumeControl />
    </div>

    <!-- Row 3: now-playing | volume | hints -->
    <div class="footer-bottom">
      <!-- Now playing -->
      <div class="now-playing" class:active={!!$currentTrack}>
        <button
          class="now-playing-main"
          onclick={openCurrentContext}
          disabled={!$currentTrack}
        >
          <div class="now-playing-art">
            {#if $currentAlbum?.cover_art}
              <img src={convertFileSrc($currentAlbum.cover_art)} alt="" />
            {:else}
              <span>♪</span>
            {/if}
          </div>
          <div class="now-playing-info">
            <span class="track-name">{$currentTrack?.title ?? 'No track playing'}</span>
            <span class="track-artist">{$currentTrack?.artist ?? '—'}</span>
          </div>
        </button>
        {#if $currentTrack}
          <button
            class="np-add-btn"
            onclick={() => { playUiSfx('open'); npPickerOpen = true; }}
            title="Add to playlist"
          >+</button>
        {/if}
      </div>

    <!-- PS2 action hints -->
      <div class="actions">
      <div class="action-hint">
        <PS2Btn type="cross" />
        <span class="btn-label">Select</span>
      </div>
      <button class="action-hint action-btn" onclick={toggleSearch}>
        <PS2Btn type="circle" />
        <span class="btn-label" class:active-search={searchOpen}>Search</span>
      </button>
      <button class="action-hint action-btn" onclick={handleShuffleAll}>
        <PS2Btn type="square" />
        <span class="btn-label" class:active-shuffle={$isShuffled}>Shuffle</span>
      </button>
      <button class="action-hint action-btn" onclick={openOptions}>
        <PS2Btn type="triangle" />
        <span class="btn-label">Options</span>
      </button>
    </div>
    </div><!-- /footer-bottom -->
  </footer>

</div>

{#if $selectedAlbum}
  <AlbumView album={$selectedAlbum} onclose={() => { selectedAlbum.set(null); followPlayback = false; }} />
{/if}

{#if selectedPlaylist}
  <PlaylistView playlist={selectedPlaylist} onclose={() => selectedPlaylist = null} />
{/if}

{#if npPickerOpen && $currentTrack}
  <PlaylistPicker track={$currentTrack} onclose={() => npPickerOpen = false} />
{/if}

{#if optionsOpen}
  <OptionsMenu onclose={() => optionsOpen = false} onStats={() => statsOpen = true} />
{/if}

{#if statsOpen}
  <StatsView albums={$albums} onclose={() => statsOpen = false} />
{/if}

</div><!-- /root -->

<style>
  .root {
    width: calc(100vw / 1.5);
    height: calc(100vh / 1.5);
    transform: scale(1.5);
    transform-origin: top left;
    position: relative;
    overflow: hidden;
    filter: saturate(0.82) contrast(1.08) brightness(0.97) blur(0.4px);
    image-rendering: crisp-edges;
  }

  .shell {
    width: 100%;
    height: 100%;
    display: grid;
    grid-template-rows: auto auto 1fr auto;
    padding: 12px 16px 10px;
  }

  /* ── Header ── */
  .header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding-bottom: 4px;
    height: 68px;
    flex-shrink: 0;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  /* ── PS2 Memory Card ── */
  .mc-card {
    width: 33px;
    height: 44px;
    background: #474747;
    border-radius: 2px;
    flex-shrink: 0;
    box-shadow: 1px 1px 3px rgba(0,0,0,0.5);
  }

  .memory-block {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .action-btn {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    transition: opacity 0.15s;
  }
  .action-btn:hover { opacity: 0.75; }

  .memory-label {
    font-size: 22px;

    color: var(--text-primary);
    letter-spacing: 0.01em;
  }

  .lib-size {
    font-size: 13px;
    color: var(--text-secondary);
  }

  .header-right {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    justify-content: center;
    gap: 4px;
    min-width: 0;
    max-width: 50%;
  }

  .scanning {
    font-size: 12px;
    color: var(--text-dim);
    letter-spacing: 0.05em;
    padding-top: 4px;
  }

  .hovered-title {
    font-size: 28px;
    color: var(--track-active);
    letter-spacing: 0.01em;
    text-align: right;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    min-width: 0;
    line-height: 1.15;
    animation: fadein 0.15s ease;
  }

  @keyframes fadein {
    from { opacity: 0; transform: translateY(-3px); }
    to   { opacity: 1; transform: translateY(0); }
  }

  /* ── Tab toggle ── */
  .tab-toggle {
    position: relative;
    display: inline-flex;
    justify-self: start;
    background: linear-gradient(180deg, rgb(22, 23, 32), rgb(28, 30, 42));
    border: 1px solid rgba(212, 219, 240, 0.08);
    border-radius: 5px;
    padding: 2px;
    box-shadow:
      inset 0 2px 4px rgba(0,0,0,0.5),
      inset 0 1px 0 rgba(0,0,0,0.3);
  }

  .tab-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    height: calc(100% - 4px);
    width: calc(50% - 2px);
    background: linear-gradient(180deg, rgb(54, 56, 70), rgb(60, 64, 80));
    border-radius: 3px;
    border: 1px solid rgba(212, 219, 240, 0.1);
    box-shadow:
      0 2px 4px rgba(0,0,0,0.35),
      inset 0 1px 0 rgba(255,255,255,0.07);
    transition: transform 0.18s cubic-bezier(0.34, 1.2, 0.64, 1);
    pointer-events: none;
  }

  .tab-thumb--right {
    transform: translateX(100%);
  }

  .tab-opt {
    position: relative;
    z-index: 1;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 10px;
    font-family: inherit;
    font-weight: 800;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--text-dim);
    padding: 3px 10px;
    border-radius: 3px;
    transition: color 0.15s;
    text-shadow: var(--text-shadow);
    white-space: nowrap;
  }

  .tab-opt:hover { color: var(--text-secondary); }
  .tab-opt--active { color: var(--text-primary); }

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
    flex-direction: column;
    gap: 6px;
    padding-top: 8px;
  }

  .footer-progress {
    display: flex;
    justify-content: center;
    padding: 0 0 2px;
  }

  .footer-top {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding: 4px 0 2px;
  }

  .footer-bottom {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .now-playing {
    display: flex;
    align-items: center;
    background: linear-gradient(180deg, rgb(48, 48, 48), rgb(54, 58, 68));
    border-radius: 8px;
    box-shadow: var(--btn-shadow);
    backdrop-filter: blur(12px);
    max-width: 210px;
    overflow: hidden;
    transition: box-shadow 0.2s, transform 0.15s;
  }

  .now-playing.active:hover {
    box-shadow: var(--card-shadow-hover);
    transform: translateY(-1px);
  }

  .now-playing-main {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    cursor: pointer;
    padding: 6px 6px 6px 6px;
    transition: opacity 0.2s;
  }

  .now-playing-main:disabled { opacity: 0.45; cursor: default; }

  .np-add-btn {
    flex-shrink: 0;
    background: none;
    border: none;
    border-left: 1px solid rgba(255, 255, 255, 0.07);
    cursor: pointer;
    font-size: 16px;
    font-weight: 800;
    color: var(--text-dim);
    padding: 0 9px;
    align-self: stretch;
    display: flex;
    align-items: center;
    transition: color 0.12s, background 0.12s;
  }

  .np-add-btn:hover { color: var(--track-hover); background: rgba(255,255,255,0.05); }

  .now-playing-art {
    width: 26px;
    height: 26px;
    border-radius: 4px;
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

  /* Transport controls */
  .transport {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .transport-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 25px;
    padding: 3px 8px;
    background: linear-gradient(180deg, rgb(48, 48, 48), rgb(54, 58, 68));
    border: 1px solid rgba(212, 219, 240, 0.12);
    border-radius: 999px;
    box-shadow:
      0 2px 6px rgba(0, 0, 0, 0.2),
      inset 0 1px 0 rgba(255, 255, 255, 0.1),
      inset 0 -1px 2px rgba(0, 0, 0, 0.28);
    cursor: pointer;
    color: var(--text-secondary);
    transition: color 0.12s, opacity 0.12s, transform 0.12s, filter 0.12s;
  }

  .transport-btn:hover:not(:disabled) {
    color: var(--text-primary);
    transform: translateY(-1px);
    filter: brightness(1.06);
  }

  .transport-btn:disabled { opacity: 0.38; cursor: default; }

  .transport-btn--shoulder {
    justify-content: center;
  }

  .transport-tag,
  .transport-icon,
  .transport-text {
    text-shadow: none;
  }

  .play-pause-text {
    display: inline-block;
    min-width: 6ch;
    text-align: center;
  }

  .transport-tag {
    font-size: 9px;
    letter-spacing: 0.08em;
    color: rgba(238, 242, 255, 0.82);
  }

  .transport-icon {
    font-size: 10px;
    font-weight: 900;
    color: var(--track-hover);
  }

  .transport-text {
    font-size: 11px;
    letter-spacing: 0.05em;
  }

  .play-btn {
    justify-content: center;
    padding-inline: 7px 9px;
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

  .btn-label {
    font-size: 11px;
    color: var(--text-secondary);
    letter-spacing: 0.03em;
  }

  .active-shuffle { color: var(--track-active); }
  .active-search  { color: var(--track-active); }

  /* ── Search ── */
  .search-input {
    width: 100%;
    background: rgba(10, 10, 22, 0.55);
    border: 1px solid rgba(90, 95, 120, 0.35);
    border-radius: 4px;
    color: var(--track-active);
    font-family: inherit;
    font-size: 22px;
    letter-spacing: 0.01em;
    padding: 3px 8px;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
    text-align: right;
  }

  .search-input:focus {
    border-color: rgba(90, 95, 180, 0.65);
    box-shadow: 0 0 10px rgba(80, 100, 200, 0.15);
  }

  .search-input::placeholder {
    color: var(--text-dim);
    font-size: 16px;
  }

  /* Compact single-line title shown below the search input */
  .hovered-title--small {
    font-size: 16px;
    -webkit-line-clamp: 1;
    line-clamp: 1;
    line-height: 1.2;
  }
</style>
