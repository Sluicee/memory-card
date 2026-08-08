<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import AlbumGrid from "$lib/components/AlbumGrid.svelte";
  import type AlbumGridType from "$lib/components/AlbumGrid.svelte";
  import AlbumView from "$lib/components/AlbumView.svelte";
  import type AlbumViewType from "$lib/components/AlbumView.svelte";
  import PlaylistGrid from "$lib/components/PlaylistGrid.svelte";
  import PlaylistView from "$lib/components/PlaylistView.svelte";
  import ArtistGrid from "$lib/components/ArtistGrid.svelte";
  import ClipGrid from "$lib/components/ClipGrid.svelte";
  import type ClipGridType from "$lib/components/ClipGrid.svelte";
  import ClipPlayerView from "$lib/components/ClipPlayerView.svelte";
  import type ClipPlayerViewType from "$lib/components/ClipPlayerView.svelte";
  import OptionsMenu from "$lib/components/OptionsMenu.svelte";
  import type OptionsMenuType from "$lib/components/OptionsMenu.svelte";
  import StatsView from "$lib/components/StatsView.svelte";
  import PlaylistPicker from "$lib/components/PlaylistPicker.svelte";
  import ViewModeBar from "$lib/components/ViewModeBar.svelte";
  import FocusView from "$lib/components/FocusView.svelte";
  import MiniPlayer from "$lib/components/MiniPlayer.svelte";
  import QueueView from "$lib/components/QueueView.svelte";
  import Header from "$lib/components/Header.svelte";
  import TabNav from "$lib/components/TabNav.svelte";
  import FooterTransport from "$lib/components/FooterTransport.svelte";
  import EqualizerPanel from "$lib/components/EqualizerPanel.svelte";
  import type EqualizerPanelType from "$lib/components/EqualizerPanel.svelte";
  import { isEqualizerOpen } from "$lib/stores/equalizer";

  import { viewMode } from "$lib/stores/viewMode";
  import { playUiSfx, primeUiSfx } from "$lib/ui-sfx";
  import { onMount } from "svelte";
  import {
    albums,
    isScanning,
    selectedAlbum,
    scanStatus,
    loadCache,
  } from "$lib/stores/library";
  import {
    clips,
    selectedClip,
    loadClipCache,
    isClipScanning,
    clipScanStatus,
  } from "$lib/stores/clips";
  import type { Clip } from "$lib/types";
  import {
    currentTrack,
    currentAlbum,
    isPlaying,
    pause,
    resume,
    playNext,
    playPrev,
    playShuffled,
    playShuffledAll,
    toggleRepeat,
    initVolume,
    loadLastTrack,
    stepVolume,
  } from "$lib/stores/player";
  import { recordListened } from "$lib/stores/stats";
  import { sortMode } from "$lib/stores/sortMode";
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

  import { updateWindowForViewMode } from "$lib/services/windowManager";
  import { setupDragAndDrop } from "$lib/services/dragAndDrop";
  import { handleGlobalKeydown } from "$lib/services/shortcuts";

  import "./page-shell.css";

  let activeTab = $state<"library" | "artists" | "playlists" | "queue" | "clips">(
    "library",
  );
  let initialAlbumPage = $state(0);
  let initialClipPage = $state(0);
  let hoveredAlbum = $state<Album | null>(null);
  let hoveredArtist = $state<Artist | null>(null);
  let hoveredPlaylist = $state<Playlist | null>(null);
  let hoveredClip = $state<Clip | null>(null);
  let selectedPlaylist = $state<Playlist | null>(null);
  let selectedArtistFilter = $state<string | null>(null);
  let optionsOpen = $state(false);
  let statsOpen = $state(false);
  let isDragOver = $state(false);
  let npPickerOpen = $state(false);
  let searchOpen = $state(false);
  let searchQuery = $state("");
  let followPlayback = $state(false);

  $effect(() => {
    if (activeTab !== "library" && selectedArtistFilter !== null) {
      selectedArtistFilter = null;
    }
  });

  // Component refs for gamepad cursor control
  let albumGrid = $state<AlbumGridType | null>(null);
  let clipGrid = $state<ClipGridType | null>(null);
  let clipPlayerView = $state<ClipPlayerViewType | null>(null);
  let albumView = $state<AlbumViewType | null>(null);
  let optionsMenu = $state<OptionsMenuType | null>(null);
  let gpNowPlayingFocused = $state(false);

  const groupedArtists = $derived(
    Object.values(
      $albums.reduce(
        (acc, album) => {
          const artistName = album.artist || "Unknown Artist";
          if (!acc[artistName]) {
            acc[artistName] = { name: artistName, albums: [] };
          }
          acc[artistName].albums.push(album);
          return acc;
        },
        {} as Record<string, Artist>,
      ),
    )
      .filter((a) => {
        if (!searchOpen || !searchQuery.trim()) return true;
        return a.name.toLowerCase().includes(searchQuery.trim().toLowerCase());
      })
      .sort((a, b) => a.name.localeCompare(b.name)),
  );

  const filteredAlbums = $derived(
    (() => {
      let list = selectedArtistFilter
        ? $albums.filter((a) => a.artist === selectedArtistFilter)
        : $albums;

      if (searchOpen && searchQuery.trim()) {
        const q = searchQuery.trim().toLowerCase();
        list = list.filter(
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
      }

      return [...list].sort((a, b) => {
        if ($sortMode === "title") {
          return (a.title || "").localeCompare(b.title || "", undefined, { sensitivity: "base" });
        }
        if ($sortMode === "year") {
          const yDiff = (b.year || 0) - (a.year || 0);
          if (yDiff !== 0) return yDiff;
          return (a.title || "").localeCompare(b.title || "", undefined, { sensitivity: "base" });
        }
        const artistCompare = (a.artist || "").localeCompare(b.artist || "", undefined, { sensitivity: "base" });
        if (artistCompare !== 0) return artistCompare;
        const yearCompare = (a.year || 0) - (b.year || 0);
        if (yearCompare !== 0) return yearCompare;
        return (a.title || "").localeCompare(b.title || "", undefined, { sensitivity: "base" });
      });
    })(),
  );

  const filteredClips = $derived(
    searchOpen && searchQuery.trim()
      ? (() => {
          const q = searchQuery.trim().toLowerCase();
          return $clips.filter(
            (c) =>
              c.title.toLowerCase().includes(q) ||
              c.search_index?.toLowerCase().includes(q),
          );
        })()
      : $clips,
  );

  const filteredPlaylists = $derived(
    searchOpen && searchQuery.trim()
      ? $playlists.filter((p) =>
          p.name.toLowerCase().includes(searchQuery.trim().toLowerCase()),
        )
      : $playlists,
  );

  function toggleSearch() {
    const opening = !searchOpen;
    searchOpen = opening;
    playUiSfx(opening ? "open" : "back");
    if (!searchOpen) {
      searchQuery = "";
    }
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Escape" && searchOpen) {
      playUiSfx("back");
      searchOpen = false;
      searchQuery = "";
    }
  }

  function onKeydown(e: KeyboardEvent) {
    handleGlobalKeydown(e, {
      optionsOpen,
      setOptionsOpen: (v) => (optionsOpen = v),
      statsOpen,
      setStatsOpen: (v) => (statsOpen = v),
      npPickerOpen,
      setNpPickerOpen: (v) => (npPickerOpen = v),
      selectedPlaylist,
      setSelectedPlaylist: (v) => (selectedPlaylist = v),
      searchOpen,
      toggleSearch,
      setSearchOpen: (v) => (searchOpen = v),
      setSearchQuery: (v) => (searchQuery = v),
      selectedArtistFilter,
      setSelectedArtistFilter: (v) => (selectedArtistFilter = v),
      activeTab,
      setActiveTab: (v) => (activeTab = v),
      setFollowPlayback: (v) => (followPlayback = v),
      handleTransportPlayPause,
      handlePrev,
      handleNext,
      handleShuffleAll,
    });
  }

  // Window geometry effect
  $effect(() => {
    updateWindowForViewMode($viewMode);
  });

  onMount(async () => {
    primeUiSfx();

    const discordRpcEnabled =
      localStorage.getItem("mc_discord_rpc_enabled") !== "false";
    invoke("set_discord_rpc_enabled", { enabled: discordRpcEnabled }).catch(
      () => {},
    );

    await initVolume();

    const last = loadLastTrack();
    if (last) {
      ct.set(last.track);
      ca.set(last.album);
      duration.set(last.track.duration);
    }

    await loadCache();
    await loadClipCache();

    initialAlbumPage = 0;

    checkForUpdates();

    setTimeout(() => {
      getCurrentWindow().show();
    }, 35);

    initGamepad();
    addGamepadListener(handleGamepadAction);

    setupDragAndDrop((over) => (isDragOver = over));
  });

  let equalizerPanel = $state<EqualizerPanelType | null>(null);

  function handleGamepadAction(action: GamepadAction) {
    if (!document.hasFocus()) return;
    const tag = document.activeElement?.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA") return;

    if ($isEqualizerOpen) {
      equalizerPanel?.handleGamepadInput(action);
      return;
    }

    if (optionsOpen) {
      handleGamepadOptions(action);
      return;
    }
    if (statsOpen || npPickerOpen) return;

    if ($selectedClip) {
      handleGamepadClipPlayer(action);
    } else if ($selectedAlbum) {
      handleGamepadAlbumView(action);
    } else if (selectedPlaylist) {
      handleGamepadPlaylistView(action);
    } else {
      handleGamepadGrid(action);
    }
  }

  function handleGamepadClipPlayer(action: GamepadAction) {
    switch (action) {
      case "circle":
        playUiSfx("back");
        if ($viewMode !== "normal") viewMode.set("normal");
        selectedClip.set(null);
        break;
      case "cross":
      case "start":
        clipPlayerView?.gamepadTogglePlayPause();
        break;
      case "l1":
        clipPlayerView?.gamepadSeekBy(-5);
        break;
      case "r1":
        clipPlayerView?.gamepadSeekBy(5);
        break;
      case "l2":
        playUiSfx("steps");
        stepVolume(-1);
        break;
      case "r2":
        playUiSfx("steps");
        stepVolume(1);
        break;
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
    if (gpNowPlayingFocused) {
      switch (action) {
        case "cross":
          gpNowPlayingFocused = false;
          openCurrentContext();
          return;
        case "up":
          gpNowPlayingFocused = false;
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
        if (activeTab === "clips") clipGrid?.gamepadConfirm();
        else albumGrid?.gamepadConfirm();
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
        stepVolume(-1);
        break;
      case "r2":
        playUiSfx("steps");
        stepVolume(1);
        break;
      case "start":
        if ($currentTrack) handleTransportPlayPause();
        break;
      case "select":
        activeTab =
          activeTab === "library"
            ? "artists"
            : activeTab === "artists"
              ? "playlists"
              : activeTab === "playlists"
                ? "clips"
                : "library";
        playUiSfx("confirm");
        albumGrid?.gamepadClearCursor();
        clipGrid?.gamepadClearCursor();
        gpNowPlayingFocused = false;
        break;
      case "l3":
        openOptions();
        break;
      case "up":
        if (activeTab === "library") albumGrid?.gamepadNavigate("up");
        else if (activeTab === "clips") clipGrid?.gamepadNavigate("up");
        break;
      case "down":
        if (activeTab === "library") {
          const hitBottom = albumGrid?.gamepadNavigate("down");
          if (hitBottom) {
            albumGrid?.gamepadClearCursor();
            gpNowPlayingFocused = true;
          }
        } else if (activeTab === "clips") {
          clipGrid?.gamepadNavigate("down");
        }
        break;
      case "left":
        if (activeTab === "library") albumGrid?.gamepadNavigate("left");
        else if (activeTab === "clips") clipGrid?.gamepadNavigate("left");
        break;
      case "right":
        if (activeTab === "library") albumGrid?.gamepadNavigate("right");
        else if (activeTab === "clips") clipGrid?.gamepadNavigate("right");
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
        stepVolume(-1);
        break;
      case "r2":
        playUiSfx("steps");
        stepVolume(1);
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
        stepVolume(-1);
        break;
      case "r2":
        playUiSfx("steps");
        stepVolume(1);
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

  function selectClip(clip: Clip) {
    playUiSfx("confirm");
    selectedClip.set(clip);
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

<svelte:window onkeydown={onKeydown} />

<div class="root" class:mode-mini={$viewMode === "mini"} class:clip-open={!!$selectedClip}>
  {#if isDragOver}
    <div class="drag-overlay">
      <div class="drag-overlay-inner">
        <div class="drag-icon">
          <svg viewBox="0 0 16 16" width="24" height="24" fill="currentColor">
            <path
              d="M7.247 11.14 2.451 5.658C1.885 5.013 2.345 4 3.204 4h9.592a1 1 0 0 1 .753 1.659l-4.796 5.48a1 1 0 0 1-1.506 0z"
            />
          </svg>
        </div>
        <span>{$t("dropToAdd")}</span>
      </div>
    </div>
  {/if}

  <ViewModeBar />

  {#if $viewMode === "mini"}
    <MiniPlayer />
  {:else}
    <div class="shell">
      <!-- Header -->
      <Header
        {activeTab}
        bind:searchOpen
        bind:searchQuery
        {hoveredAlbum}
        {hoveredArtist}
        {hoveredPlaylist}
        {hoveredClip}
        bind:selectedArtistFilter
        {onSearchKey}
        onClearArtistFilter={() => {
          playUiSfx("back");
          selectedArtistFilter = null;
        }}
      />

      <!-- Tab switcher -->
      <TabNav
        bind:activeTab
        onSelectTab={(tab) => {
          if (activeTab !== tab) {
            playUiSfx(tab === "library" ? "back" : "confirm");
          }
          activeTab = tab;
          if (tab === "library") selectedArtistFilter = null;
        }}
      />

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
                {#if $scanStatus.totalFiles > 0}
                  {@const pct = Math.round(
                    ($scanStatus.filesScanned / $scanStatus.totalFiles) * 100,
                  )}
                  <div class="scan-bar-track">
                    <div class="scan-bar-fill" style="width: {pct}%"></div>
                  </div>
                  <span class="scan-bar-label"
                    >{pct}% · {$scanStatus.albumsFound}
                    {$t("albumsFound")}</span
                  >
                {:else}
                  <div class="spinner-sm"></div>
                  <span>{$t("startingScan")}</span>
                {/if}
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
                onPageChange={(p) => {
                  initialAlbumPage = p;
                  localStorage.setItem("mp_album_page", p.toString());
                }}
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
                searchOpen = false;
                searchQuery = "";
                activeTab = "library";
              }}
              onhover={(artist) => (hoveredArtist = artist)}
            />
          {/if}
        {:else if activeTab === "playlists"}
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
        {:else if activeTab === "clips"}
          {#if $isClipScanning && $clips.length === 0}
            <div class="state-msg">
              <div class="spinner"></div>
              <p class="scan-info">
                {#if $clipScanStatus.filesScanned > 0}
                  {$t(
                    "scanClipFiles",
                    $clipScanStatus.filesScanned,
                    $clipScanStatus.clipsFound,
                  )}
                {:else}
                  {$t("startingScan")}
                {/if}
              </p>
            </div>
          {:else if $clips.length === 0}
            <div class="state-msg">
              <p class="hint">{$t("noClipsYet")}</p>
            </div>
          {:else if searchOpen && searchQuery && filteredClips.length === 0}
            <div class="state-msg">
              <p class="hint">
                {$t("noResultsFor")} <strong>{searchQuery}</strong>
              </p>
            </div>
          {:else}
            {#if $isClipScanning}
              <div class="scan-bar">
                {#if $clipScanStatus.totalFiles > 0}
                  {@const pct = Math.round(
                    ($clipScanStatus.filesScanned / $clipScanStatus.totalFiles) * 100,
                  )}
                  <div class="scan-bar-track">
                    <div class="scan-bar-fill" style="width: {pct}%"></div>
                  </div>
                  <span class="scan-bar-label"
                    >{pct}% · {$clipScanStatus.clipsFound}
                    {$t("clipsFoundLabel")}</span
                  >
                {:else}
                  <div class="spinner-sm"></div>
                  <span>{$t("startingScan")}</span>
                {/if}
              </div>
            {/if}
            <ClipGrid
              bind:this={clipGrid}
              clips={filteredClips}
              onselect={selectClip}
              onhover={(c) => (hoveredClip = c)}
              initialPage={initialClipPage}
              onPageChange={(p) => (initialClipPage = p)}
            />
          {/if}
        {:else}
          <QueueView />
        {/if}
      </main>

      <!-- Footer -->
      <FooterTransport
        selectedAlbum={$selectedAlbum}
        {selectedPlaylist}
        {gpNowPlayingFocused}
        {searchOpen}
        onPrev={handlePrev}
        onNext={handleNext}
        onPlayPause={handleTransportPlayPause}
        onOpenCurrentContext={openCurrentContext}
        onOpenNpPicker={() => {
          playUiSfx("open");
          npPickerOpen = true;
        }}
        onToggleSearch={toggleSearch}
        onShuffleAll={handleShuffleAll}
        onToggleRepeat={() => {
          playUiSfx("confirm");
          toggleRepeat();
        }}
        onOpenOptions={openOptions}
      />
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

    {#if $selectedClip}
      <ClipPlayerView
        bind:this={clipPlayerView}
        clip={$selectedClip}
        onclose={() => selectedClip.set(null)}
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
        {activeTab}
        onclose={() => (optionsOpen = false)}
        onStats={() => (statsOpen = true)}
      />
    {/if}

    {#if statsOpen}
      <StatsView albums={$albums} onclose={() => (statsOpen = false)} />
    {/if}

    {#if $isEqualizerOpen}
      <EqualizerPanel bind:this={equalizerPanel} />
    {/if}

    {#if $viewMode === "focus" || $viewMode === "fullscreen"}
      <FocusView />
    {/if}
  {/if}
</div>
