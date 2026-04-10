<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
  import { LogicalSize, LogicalPosition } from "@tauri-apps/api/dpi";
  import AlbumGrid from "$lib/components/AlbumGrid.svelte";
  import type AlbumGridType from "$lib/components/AlbumGrid.svelte";
  import AlbumView from "$lib/components/AlbumView.svelte";
  import type AlbumViewType from "$lib/components/AlbumView.svelte";
  import PlaylistGrid from "$lib/components/PlaylistGrid.svelte";
  import PlaylistView from "$lib/components/PlaylistView.svelte";
  import ArtistGrid from "$lib/components/ArtistGrid.svelte";
  import VolumeControl from "$lib/components/VolumeControl.svelte";
  import ProgressBar from "$lib/components/ProgressBar.svelte";
  import PS2Btn from "$lib/components/PS2Btn.svelte";
  import OptionsMenu from "$lib/components/OptionsMenu.svelte";
  import type OptionsMenuType from "$lib/components/OptionsMenu.svelte";
  import StatsView from "$lib/components/StatsView.svelte";
  import PlaylistPicker from "$lib/components/PlaylistPicker.svelte";
  import ViewModeBar from "$lib/components/ViewModeBar.svelte";
  import FocusView from "$lib/components/FocusView.svelte";
  import MiniPlayer from "$lib/components/MiniPlayer.svelte";
  import { viewMode } from "$lib/stores/viewMode";
  import { playUiSfx, primeUiSfx } from "$lib/ui-sfx";
  import { onMount } from "svelte";
  import {
    albums,
    isScanning,
    librarySize,
    selectedAlbum,
    scanStatus,
    loadCache,
  } from "$lib/stores/library";
  import {
    currentTrack,
    currentAlbum,
    isPlaying,
    isPaused,
    pause,
    resume,
    playNext,
    playPrev,
    playShuffled,
    playShuffledAll,
    isShuffled,
    repeatMode,
    toggleRepeat,
    initVolume,
    loadLastTrack,
    volume,
    setVolume,
  } from "$lib/stores/player";
  import { checkForUpdates } from "$lib/stores/updates";
  import { t } from "$lib/stores/i18n";
  import {
    initGamepad,
    addGamepadListener,
    type GamepadAction,
  } from "$lib/gamepad";
  import {
    currentTrack as ct,
    currentAlbum as ca,
    duration,
  } from "$lib/stores/player";
  import { playlists } from "$lib/stores/playlists";
  import { currentPlaylistId } from "$lib/stores/player";
  import type { Album, Artist } from "$lib/types";
  import type { Playlist } from "$lib/stores/playlists";

  let activeTab = $state<"library" | "artists" | "playlists">("library");
  let initialAlbumPage = $state(0);
  let hoveredAlbum = $state<Album | null>(null);
  let hoveredArtist = $state<Artist | null>(null);
  let hoveredPlaylist = $state<Playlist | null>(null);
  let selectedPlaylist = $state<Playlist | null>(null);
  let selectedArtistFilter = $state<string | null>(null);
  let optionsOpen = $state(false);
  let statsOpen = $state(false);
  let npPickerOpen = $state(false);
  let searchOpen = $state(false);
  let searchQuery = $state("");
  let searchInput = $state<HTMLInputElement | null>(null);
  let followPlayback = $state(false);

  $effect(() => {
    if (activeTab !== "library" && selectedArtistFilter !== null) {
      selectedArtistFilter = null;
    }
  });

  // Component refs for gamepad cursor control
  let albumGrid = $state<AlbumGridType | null>(null);
  let albumView = $state<AlbumViewType | null>(null);
  let optionsMenu = $state<OptionsMenuType | null>(null);
  // Gamepad focus on the now-playing footer block
  let gpNowPlayingFocused = $state(false);

  const groupedArtists = $derived(
    Object.values(
      $albums.reduce((acc, album) => {
        const artistName = album.artist || 'Unknown Artist';
        if (!acc[artistName]) {
          acc[artistName] = { name: artistName, albums: [] };
        }
        acc[artistName].albums.push(album);
        return acc;
      }, {} as Record<string, Artist>)
    )
    .filter(a => {
      if (!searchOpen || !searchQuery.trim()) return true;
      return a.name.toLowerCase().includes(searchQuery.trim().toLowerCase());
    })
    .sort((a, b) => a.name.localeCompare(b.name))
  );

  const filteredAlbums = $derived(
    searchOpen && searchQuery.trim()
      ? (() => {
          const q = searchQuery.trim().toLowerCase();
          return $albums.filter(
            (a) =>
              a.title.toLowerCase().includes(q) ||
              a.artist.toLowerCase().includes(q) ||
              a.search_index?.toLowerCase().includes(q) ||
              a.tracks.some(
                (t) =>
                  t.title.toLowerCase().includes(q) ||
                  t.artist.toLowerCase().includes(q) ||
                  t.search_index?.toLowerCase().includes(q),
              ),
          );
        })()
      : selectedArtistFilter
        ? $albums.filter(a => a.artist === selectedArtistFilter)
        : $albums,
  );

  const filteredPlaylists = $derived(
    searchOpen && searchQuery.trim()
      ? $playlists.filter(p => p.name.toLowerCase().includes(searchQuery.trim().toLowerCase()))
      : $playlists
  );

  function toggleSearch() {
    const opening = !searchOpen;
    searchOpen = opening;
    playUiSfx(opening ? "open" : "back");
    if (!searchOpen) {
      searchQuery = "";
    } else setTimeout(() => searchInput?.focus(), 30);
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Escape" && searchOpen) {
      playUiSfx("back");
      searchOpen = false;
      searchQuery = "";
    }
  }

  let mutedVolume = 0;

  function handleGlobalKeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;

    const VOL_STEP = 1 / 20;

    switch (e.code) {
      case "Space":
        e.preventDefault();
        if ($currentTrack) handleTransportPlayPause();
        break;
      case "ArrowLeft":
        e.preventDefault();
        if ($currentAlbum) handlePrev();
        break;
      case "ArrowRight":
        e.preventDefault();
        if ($currentAlbum) handleNext();
        break;
      case "ArrowUp":
        e.preventDefault();
        playUiSfx("steps");
        setVolume(Math.min(1, $volume + VOL_STEP));
        break;
      case "ArrowDown":
        e.preventDefault();
        playUiSfx("steps");
        setVolume(Math.max(0, $volume - VOL_STEP));
        break;
      case "KeyS":
        handleShuffleAll();
        break;
      case "KeyR":
        playUiSfx("confirm");
        toggleRepeat();
        break;
      case "KeyF":
      case "Slash":
        toggleSearch();
        break;
      case "KeyM":
        if ($volume > 0) {
          mutedVolume = $volume;
          setVolume(0);
        } else {
          setVolume(mutedVolume || 1);
        }
        playUiSfx("steps");
        break;
      case "Escape":
        if ($viewMode !== "normal") {
          playUiSfx("back");
          viewMode.set("normal");
        } else if (optionsOpen) {
          optionsOpen = false;
        } else if (statsOpen) {
          statsOpen = false;
        } else if (npPickerOpen) {
          npPickerOpen = false;
        } else if (selectedPlaylist) {
          selectedPlaylist = null;
        } else if ($selectedAlbum) {
          selectedAlbum.set(null);
          followPlayback = false;
        } else if (searchOpen) {
          playUiSfx("back");
          searchOpen = false;
          searchQuery = "";
        } else if (selectedArtistFilter) {
          playUiSfx("back");
          selectedArtistFilter = null;
        }
        break;
      case "Digit1":
        activeTab = "library";
        playUiSfx("back");
        break;
      case "Digit2":
        activeTab = "artists";
        playUiSfx("confirm");
        break;
      case "Digit3":
        activeTab = "playlists";
        playUiSfx("confirm");
        break;
    }
  }

  // Resize/reposition window when view mode changes
  $effect(() => {
    const mode = $viewMode;
    const win = getCurrentWindow();
    (async () => {
      switch (mode) {
        case "normal":
        case "focus": {
          await win.setFullscreen(false);
          await win.setAlwaysOnTop(false);
          const targetW = 950;
          const targetH = 900;
          await win.setSize(new LogicalSize(targetW, targetH));
          // After resize, clamp position so window stays fully on-screen
          try {
            const monitor = await currentMonitor();
            if (monitor) {
              const sf = monitor.scaleFactor;
              // monitor.position and size are in physical pixels
              const monX = monitor.position.x / sf;
              const monY = monitor.position.y / sf;
              const monW = monitor.size.width / sf;
              const monH = monitor.size.height / sf;
              const pos = await win.outerPosition();
              const curX = pos.x / sf;
              const curY = pos.y / sf;
              const clampedX = Math.max(monX, Math.min(curX, monX + monW - targetW));
              const clampedY = Math.max(monY, Math.min(curY, monY + monH - targetH));
              if (clampedX !== curX || clampedY !== curY) {
                await win.setPosition(new LogicalPosition(clampedX, clampedY));
              }
            }
          } catch { /* ignore position errors */ }
          break;
        }
        case "fullscreen":
          await win.setAlwaysOnTop(false);
          await win.setFullscreen(true);
          break;
        case "mini":
          await win.setFullscreen(false);
          await win.setAlwaysOnTop(false);
          await win.setSize(new LogicalSize(400, 160));
          break;
      }
    })();
  });

  onMount(async () => {
    primeUiSfx();

    // Sync Discord RPC setting to backend
    const discordRpcEnabled =
      localStorage.getItem("mc_discord_rpc_enabled") !== "false";
    invoke("set_discord_rpc_enabled", { enabled: discordRpcEnabled }).catch(
      () => {},
    );

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

    // Pick a random starting page now that the full library is loaded
    const totalPages = Math.max(1, Math.ceil($albums.length / 12));
    initialAlbumPage = Math.floor(Math.random() * totalPages);

    // Check for updates in background
    checkForUpdates();

    // Show window once initialization is complete
    setTimeout(() => {
      getCurrentWindow().show();
    }, 35);

    // Gamepad support
    initGamepad();
    addGamepadListener(handleGamepadAction);
  });

  const VOL_STEP = 1 / 20;

  function handleGamepadAction(action: GamepadAction) {
    // Don't handle gamepad when typing
    const tag = document.activeElement?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;

    // Route to options menu when open
    if (optionsOpen) {
      handleGamepadOptions(action);
      return;
    }
    if (statsOpen || npPickerOpen) return;

    if ($selectedAlbum) {
      handleGamepadAlbumView(action);
    } else if (selectedPlaylist) {
      handleGamepadPlaylistView(action);
    } else {
      handleGamepadGrid(action);
    }
  }

  function handleGamepadOptions(action: GamepadAction) {
    switch (action) {
      case "circle":
      case "l3":
        optionsMenu?.gamepadClearCursor();
        optionsOpen = false;
        playUiSfx("back");
        break;
      case "cross":
        optionsMenu?.gamepadConfirm();
        break;
      case "up":
        optionsMenu?.gamepadNavigate("up");
        break;
      case "down":
        optionsMenu?.gamepadNavigate("down");
        break;
      case "left":
        optionsMenu?.gamepadNavigate("left");
        break;
      case "right":
        optionsMenu?.gamepadNavigate("right");
        break;
    }
  }

  function handleGamepadGrid(action: GamepadAction) {
    // When now-playing block is focused, reroute relevant actions
    if (gpNowPlayingFocused) {
      switch (action) {
        case "cross":
          gpNowPlayingFocused = false;
          openCurrentContext();
          return;
        case "up":
          gpNowPlayingFocused = false;
          // Return cursor to bottom row of grid
          albumGrid?.gamepadNavigate("up");
          return;
        case "circle":
        case "down":
        case "left":
        case "right":
          gpNowPlayingFocused = false;
          return;
        case "l3":
          gpNowPlayingFocused = false;
          openOptions();
          return;
        default:
          break;
      }
    }

    switch (action) {
      case "cross":
        albumGrid?.gamepadConfirm();
        break;
      case "circle":
        toggleSearch();
        break;
      case "square":
        handleShuffleAll();
        break;
      case "triangle":
        playUiSfx("confirm");
        toggleRepeat();
        break;
      case "l1":
        if ($currentAlbum) handlePrev();
        break;
      case "r1":
        if ($currentAlbum) handleNext();
        break;
      case "l2":
        playUiSfx("steps");
        setVolume(Math.max(0, $volume - VOL_STEP));
        break;
      case "r2":
        playUiSfx("steps");
        setVolume(Math.min(1, $volume + VOL_STEP));
        break;
      case "start":
        if ($currentTrack) handleTransportPlayPause();
        break;
      case "select":
        activeTab = activeTab === "library" ? "artists" : activeTab === "artists" ? "playlists" : "library";
        playUiSfx("confirm");
        albumGrid?.gamepadClearCursor();
        gpNowPlayingFocused = false;
        break;
      case "l3":
        openOptions();
        break;
      case "up":
        if (activeTab === "library") albumGrid?.gamepadNavigate("up");
        break;
      case "down":
        if (activeTab === "library") {
          const hitBottom = albumGrid?.gamepadNavigate("down");
          if (hitBottom) {
            albumGrid?.gamepadClearCursor();
            gpNowPlayingFocused = true;
          }
        }
        break;
      case "left":
        if (activeTab === "library") albumGrid?.gamepadNavigate("left");
        break;
      case "right":
        if (activeTab === "library") albumGrid?.gamepadNavigate("right");
        break;
    }
  }

  function handleGamepadAlbumView(action: GamepadAction) {
    switch (action) {
      case "circle":
        selectedAlbum.set(null);
        followPlayback = false;
        playUiSfx("back");
        break;
      case "cross":
        albumView?.gamepadConfirm();
        break;
      case "square":
        if ($selectedAlbum) handleShuffle($selectedAlbum);
        break;
      case "triangle":
        playUiSfx("confirm");
        toggleRepeat();
        break;
      case "l1":
        if ($currentAlbum) handlePrev();
        break;
      case "r1":
        if ($currentAlbum) handleNext();
        break;
      case "l2":
        playUiSfx("steps");
        setVolume(Math.max(0, $volume - VOL_STEP));
        break;
      case "r2":
        playUiSfx("steps");
        setVolume(Math.min(1, $volume + VOL_STEP));
        break;
      case "start":
        if ($currentTrack) handleTransportPlayPause();
        break;
      case "l3":
        openOptions();
        break;
      case "up":
        albumView?.gamepadNavigate("up");
        break;
      case "down":
        albumView?.gamepadNavigate("down");
        break;
    }
  }

  function handleGamepadPlaylistView(action: GamepadAction) {
    switch (action) {
      case "circle":
        selectedPlaylist = null;
        playUiSfx("back");
        break;
      case "l1":
        if ($currentAlbum) handlePrev();
        break;
      case "r1":
        if ($currentAlbum) handleNext();
        break;
      case "l2":
        playUiSfx("steps");
        setVolume(Math.max(0, $volume - VOL_STEP));
        break;
      case "r2":
        playUiSfx("steps");
        setVolume(Math.min(1, $volume + VOL_STEP));
        break;
      case "start":
        if ($currentTrack) handleTransportPlayPause();
        break;
      case "l3":
        openOptions();
        break;
    }
  }

  async function handleShuffle(album: Album) {
    playUiSfx("confirm");
    await playShuffled(album);
  }

  function selectAlbum(album: Album) {
    playUiSfx("confirm");
    followPlayback = false;
    selectedAlbum.set(album);
  }

  async function handleTransportPlayPause() {
    if ($isPlaying) await pause();
    else await resume();
  }

  async function handlePrev() {
    if (!$currentAlbum) return;
    playUiSfx("nextPrev");
    await playPrev($currentAlbum);
  }

  async function handleNext() {
    if (!$currentAlbum) return;
    playUiSfx("nextPrev");
    await playNext($currentAlbum);
  }

  async function handleShuffleAll() {
    playUiSfx("confirm");
    await playShuffledAll($albums);
  }

  function openOptions() {
    playUiSfx("open");
    optionsOpen = true;
  }

  function openCurrentContext() {
    if ($currentPlaylistId) {
      const pl = $playlists.find((p) => p.id === $currentPlaylistId);
      if (pl) {
        playUiSfx("confirm");
        activeTab = "playlists";
        selectedPlaylist = pl;
      }
    } else if ($currentAlbum) {
      playUiSfx("confirm");
      followPlayback = true;
      selectedAlbum.set($currentAlbum);
    }
  }

  $effect(() => {
    if (
      followPlayback &&
      $selectedAlbum &&
      $currentAlbum &&
      $selectedAlbum !== $currentAlbum
    ) {
      selectedAlbum.set($currentAlbum);
    }
  });
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="root" class:mode-mini={$viewMode === "mini"}>
  <ViewModeBar />

  {#if $viewMode === "mini"}
    <MiniPlayer />
  {:else}
    <div class="shell">
      <!-- Header -->
      <header class="header">
        <div class="header-left">
          <div class="mc-card"></div>
          <div class="memory-block">
            <span class="memory-label">{$t("memoryCard")}</span>
            {#if $librarySize !== "0 MB"}
              <span class="lib-size">{$librarySize}</span>
            {/if}
          </div>
        </div>

        <div class="header-right">
          {#if searchOpen}
            <input
              bind:this={searchInput}
              bind:value={searchQuery}
              onkeydown={onSearchKey}
              class="search-input"
              placeholder={$t("searchPlaceholder")}
              autocomplete="off"
              spellcheck="false"
            />
          {:else if $isScanning}
            <span class="scanning">{$t("scanning")}</span>
          {/if}
          {#if activeTab === "library" && hoveredAlbum && !selectedArtistFilter}
            <span class="hovered-title" class:hovered-title--small={searchOpen}
              >{hoveredAlbum.title}</span
            >
          {:else if selectedArtistFilter && activeTab === "library"}
            <span class="hovered-title" style="display:flex; align-items:center; gap: 8px;">
              {selectedArtistFilter}
              <button class="action-btn" onclick={() => { playUiSfx("back"); selectedArtistFilter = null; }} style="font-size: 20px; color: var(--text-dim)">&times;</button>
            </span>
          {:else if activeTab === "artists" && hoveredArtist}
            <span class="hovered-title">{hoveredArtist.name}</span>
          {:else if activeTab === "playlists" && hoveredPlaylist}
            <span class="hovered-title">{hoveredPlaylist.name}</span>
          {/if}
        </div>
      </header>

      <!-- Tab switcher -->
      <div class="tab-toggle">
        <div
          class="tab-thumb"
          class:tab-thumb--artists={activeTab === "artists"}
          class:tab-thumb--playlists={activeTab === "playlists"}
        ></div>
        <button
          class="tab-opt"
          class:tab-opt--active={activeTab === "library"}
          onclick={() => {
            activeTab = "library";
            playUiSfx("back");
          }}>{$t("library")}</button
        >
        <button
          class="tab-opt"
          class:tab-opt--active={activeTab === "artists"}
          onclick={() => {
            activeTab = "artists";
            playUiSfx("confirm");
          }}>{$t("artistsTab")}</button
        >
        <button
          class="tab-opt"
          class:tab-opt--active={activeTab === "playlists"}
          onclick={() => {
            activeTab = "playlists";
            playUiSfx("confirm");
          }}>{$t("playlists")}</button
        >
      </div>

      <!-- Content -->
      <main class="content">
        {#if activeTab === "library"}
          {#if $isScanning && $albums.length === 0}
            <div class="state-msg">
              <div class="spinner"></div>
              <p class="scan-info">
                {#if $scanStatus.filesScanned > 0}
                  {$t(
                    "scanFiles",
                    $scanStatus.filesScanned,
                    $scanStatus.albumsFound,
                  )}
                {:else}
                  {$t("startingScan")}
                {/if}
              </p>
            </div>
          {:else if $albums.length === 0}
            <div class="state-msg">
              <p class="hint">
                {$t("selectHint")} <strong>{$t("optionsWord")}</strong>
                {$t("selectHintSuffix")}
              </p>
            </div>
          {:else}
            {#if $isScanning}
              <div class="scan-bar">
                <div class="spinner-sm"></div>
                <span
                  >{$t(
                    "scanFilesFound",
                    $scanStatus.filesScanned,
                    $scanStatus.albumsFound,
                  )}</span
                >
              </div>
            {/if}
            {#if searchOpen && searchQuery && filteredAlbums.length === 0}
              <div class="state-msg">
                <p class="hint">
                  {$t("noResultsFor")} <strong>{searchQuery}</strong>
                </p>
              </div>
            {:else}
              <AlbumGrid
                bind:this={albumGrid}
                albums={filteredAlbums}
                onselect={selectAlbum}
                onhover={(a) => (hoveredAlbum = a)}
                initialPage={initialAlbumPage}
              />
            {/if}
          {/if}
        {:else if activeTab === "artists"}
          {#if searchOpen && searchQuery && groupedArtists.length === 0}
            <div class="state-msg">
              <p class="hint">
                {$t("noResultsFor")} <strong>{searchQuery}</strong>
              </p>
            </div>
          {:else}
            <ArtistGrid
              artists={groupedArtists}
              onselect={(artist) => {
                playUiSfx("confirm");
                selectedArtistFilter = artist.name;
                activeTab = "library";
              }}
              onhover={(artist) => (hoveredArtist = artist)}
            />
          {/if}
        {:else}
          {#if searchOpen && searchQuery && filteredPlaylists.length === 0}
            <div class="state-msg">
              <p class="hint">
                {$t("noResultsFor")} <strong>{searchQuery}</strong>
              </p>
            </div>
          {:else}
            <PlaylistGrid
              playlists={filteredPlaylists}
              onselect={(pl) => {
                playUiSfx("confirm");
                selectedPlaylist = pl;
              }}
              onhover={(pl) => (hoveredPlaylist = pl)}
            />
          {/if}
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
              <span class="transport-text">{$t("prev")}</span>
            </button>
            <button
              class="transport-btn play-btn"
              onclick={handleTransportPlayPause}
              disabled={!$currentTrack}
              title={$isPlaying ? "Pause" : "Play"}
            >
              <PS2Btn type="start" />
              <span class="transport-text play-pause-text"
                >{$isPlaying ? $t("pause") : $t("play")}</span
              >
            </button>
            <button
              class="transport-btn transport-btn--shoulder"
              onclick={handleNext}
              disabled={!$currentTrack}
              title="Next"
            >
              <span class="transport-tag">R1</span>
              <span class="transport-icon">&gt;&gt;</span>
              <span class="transport-text">{$t("next")}</span>
            </button>
          </div>
          <VolumeControl />
        </div>

        <!-- Row 3: now-playing | volume | hints -->
        <div class="footer-bottom">
          <!-- Now playing -->
          <div
            class="now-playing"
            class:active={!!$currentTrack}
            class:gp-focused={gpNowPlayingFocused}
          >
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
                <span class="track-name"
                  >{$currentTrack?.title ?? $t("noTrackPlaying")}</span
                >
                <span class="track-artist">{$currentTrack?.artist ?? "—"}</span>
              </div>
            </button>
            {#if $currentTrack}
              <button
                class="np-add-btn"
                onclick={() => {
                  playUiSfx("open");
                  npPickerOpen = true;
                }}
                title="Add to playlist">+</button
              >
            {/if}
          </div>

          <!-- PS2 action hints -->
          <div class="actions">
            <div class="action-hint">
              <PS2Btn type="cross" />
              <span class="btn-label">{$t("select")}</span>
            </div>
            <button class="action-hint action-btn" onclick={toggleSearch}>
              <PS2Btn type="circle" />
              <span class="btn-label" class:active-search={searchOpen}
                >{$t("search")}</span
              >
            </button>
            <button class="action-hint action-btn" onclick={handleShuffleAll}>
              <PS2Btn type="square" />
              <span class="btn-label" class:active-shuffle={$isShuffled}
                >{$t("shuffle")}</span
              >
            </button>
            <button
              class="action-hint action-btn"
              onclick={() => {
                playUiSfx("confirm");
                toggleRepeat();
              }}
            >
              <PS2Btn type="triangle" />
              <span
                class="btn-label repeat-label"
                class:active-repeat={$repeatMode !== "none"}
                >{$repeatMode === "one"
                  ? $t("repeatOne")
                  : $repeatMode === "all"
                    ? $t("repeatAll")
                    : $t("repeat")}</span
              >
            </button>
            <button
              class="action-hint action-btn options-btn"
              onclick={openOptions}
              title="Options"
            >
              <span class="gear-icon">⚙</span>
              <span class="options-gp-tag">L3</span>
            </button>
          </div>
        </div>
        <!-- /footer-bottom -->
      </footer>
    </div>

    {#if $selectedAlbum}
      <AlbumView
        bind:this={albumView}
        album={$selectedAlbum}
        onclose={() => {
          selectedAlbum.set(null);
          followPlayback = false;
        }}
      />
    {/if}

    {#if selectedPlaylist}
      <PlaylistView
        playlist={selectedPlaylist}
        onclose={() => (selectedPlaylist = null)}
      />
    {/if}

    {#if npPickerOpen && $currentTrack}
      <PlaylistPicker
        track={$currentTrack}
        onclose={() => (npPickerOpen = false)}
      />
    {/if}

    {#if optionsOpen}
      <OptionsMenu
        bind:this={optionsMenu}
        onclose={() => (optionsOpen = false)}
        onStats={() => (statsOpen = true)}
      />
    {/if}

    {#if statsOpen}
      <StatsView albums={$albums} onclose={() => (statsOpen = false)} />
    {/if}

    {#if $viewMode === "focus" || $viewMode === "fullscreen"}
      <FocusView />
    {/if}
  {/if}
</div>

<!-- /root -->

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

  .root.mode-mini {
    background: transparent;
    filter: none;
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
    box-shadow: 1px 1px 3px rgba(0, 0, 0, 0.5);
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
  .action-btn:hover {
    opacity: 0.75;
  }

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
    from {
      opacity: 0;
      transform: translateY(-3px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  /* ── Tab toggle ── */
  .tab-toggle {
    position: relative;
    display: inline-grid;
    grid-template-columns: repeat(3, 80px);
    justify-self: start;
    background: linear-gradient(180deg, rgb(38, 38, 42), rgb(54, 58, 68));
    border: 1px solid rgba(212, 219, 240, 0.12);
    border-radius: 6px;
    padding: 2px;
    box-shadow:
      inset 0 2px 4px rgba(0, 0, 0, 0.5),
      0 1px 0 rgba(255, 255, 255, 0.05);
    margin-bottom: 5px;
  }

  .tab-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    height: calc(100% - 4px);
    width: 80px;
    background: linear-gradient(180deg, #4a4a4f, #2a2a2f);
    border-radius: 4px;
    border: 1px solid rgba(212, 219, 240, 0.15);
    box-shadow:
      0 2px 5px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);
    transition: transform 0.22s cubic-bezier(0.34, 1.35, 0.64, 1);
    pointer-events: none;
  }

  .tab-thumb--artists {
    transform: translateX(100%);
  }

  .tab-thumb--playlists {
    transform: translateX(200%);
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
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-dim);
    padding: 4px 0;
    border-radius: 4px;
    transition: all 0.2s ease;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.8);
    white-space: nowrap;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
  }

  .tab-opt:hover {
    color: var(--text-secondary);
  }
  .tab-opt--active {
    color: var(--text-primary);
  }

  /* ── Content ── */
  .content {
    overflow: hidden;
    padding: 6px 0;
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

  .hint {
    font-size: 14px;
  }
  .hint strong {
    color: var(--text-secondary);
  }

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

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

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
    transition:
      box-shadow 0.2s,
      transform 0.15s;
  }

  .now-playing.active:hover {
    box-shadow: var(--card-shadow-hover);
    transform: translateY(-1px);
  }

  .now-playing.gp-focused {
    box-shadow:
      0 0 0 2px rgba(100, 140, 255, 0.7),
      var(--btn-shadow);
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

  .now-playing-main:disabled {
    opacity: 0.45;
    cursor: default;
  }

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
    transition:
      color 0.12s,
      background 0.12s;
  }

  .np-add-btn:hover {
    color: var(--track-hover);
    background: rgba(255, 255, 255, 0.05);
  }

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
    transition:
      color 0.12s,
      opacity 0.12s,
      transform 0.12s,
      filter 0.12s;
  }

  .transport-btn:hover:not(:disabled) {
    color: var(--text-primary);
    transform: translateY(-1px);
    filter: brightness(1.06);
  }

  .transport-btn:disabled {
    opacity: 0.38;
    cursor: default;
  }

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
    gap: 12px;
  }

  .gear-icon {
    font-size: 16px;
    color: var(--text-secondary);
    line-height: 1;
    opacity: 0.75;
    transition:
      opacity 0.15s,
      color 0.15s;
  }

  .options-btn {
    flex-direction: column;
    gap: 2px;
  }

  .options-btn:hover .gear-icon {
    opacity: 1;
    color: var(--text-primary);
  }

  .options-gp-tag {
    font-size: 8px;
    letter-spacing: 0.1em;
    color: var(--text-dim);
    line-height: 1;
    text-shadow: none;
  }

  .options-btn:hover .options-gp-tag {
    color: var(--text-secondary);
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

  .active-shuffle {
    color: var(--track-active);
  }
  .active-search {
    color: var(--track-active);
  }
  .active-repeat {
    color: var(--track-active);
  }
  .repeat-label {
    display: inline-block;
    min-width: 58px;
    text-align: start;
  }

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
    transition:
      border-color 0.15s,
      box-shadow 0.15s;
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
