<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import ProgressBar from "$lib/components/ProgressBar.svelte";
  import VolumeControl from "$lib/components/VolumeControl.svelte";
  import PS2Btn from "$lib/components/PS2Btn.svelte";
  import { t } from "$lib/stores/i18n";
  import {
    currentTrack,
    currentAlbum,
    isPlaying,
    isShuffled,
    repeatMode,
  } from "$lib/stores/player";
  import type { Album } from "$lib/types";
  import type { Playlist } from "$lib/stores/playlists";

  let {
    selectedAlbum = null,
    selectedPlaylist = null,
    gpNowPlayingFocused = false,
    searchOpen = false,
    onPrev,
    onNext,
    onPlayPause,
    onOpenCurrentContext,
    onOpenNpPicker,
    onToggleSearch,
    onShuffleAll,
    onToggleRepeat,
    onOpenOptions,
  }: {
    selectedAlbum?: Album | null;
    selectedPlaylist?: Playlist | null;
    gpNowPlayingFocused?: boolean;
    searchOpen?: boolean;
    onPrev: () => void;
    onNext: () => void;
    onPlayPause: () => void;
    onOpenCurrentContext: () => void;
    onOpenNpPicker: () => void;
    onToggleSearch: () => void;
    onShuffleAll: () => void;
    onToggleRepeat: () => void;
    onOpenOptions: () => void;
  } = $props();

  let containerWidth = $state(0);
  let textWidth = $state(0);
</script>

<footer class="footer">
  <!-- Row 1: progress -->
  <div class="footer-progress">
    <ProgressBar />
  </div>

  <!-- Row 2: transport + volume -->
  {#if !selectedAlbum && !selectedPlaylist}
    <div class="footer-top">
      <div class="transport">
        <button
          class="transport-btn transport-btn--shoulder"
          onclick={onPrev}
          disabled={!$currentTrack}
          title="Previous"
        >
          <span class="transport-tag">L1</span>
          <span class="transport-icon">&lt;&lt;</span>
          <span class="transport-text">{$t("prev")}</span>
        </button>
        <button
          class="transport-btn play-btn"
          onclick={onPlayPause}
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
          onclick={onNext}
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
  {/if}

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
        onclick={onOpenCurrentContext}
        disabled={!$currentTrack}
      >
        <div class="now-playing-art">
          {#if $currentAlbum?.cover_art}
            <img src={convertFileSrc($currentAlbum.cover_art)} alt="" />
          {:else}
            <span>
              <svg
                viewBox="0 0 16 16"
                width="24"
                height="24"
                fill="currentColor"
                style="display: inline-block; vertical-align: middle;"
              >
                <path
                  d="M9 13c0 1.105-1.12 2-2.5 2S4 14.105 4 13s1.12-2 2.5-2 2.5.895 2.5 2zM9 3v7h1v-7H9z"
                />
                <path
                  d="M9 3v.5a.5.5 0 0 0 .5.5h4a.5.5 0 0 0 .5-.5V3a.5.5 0 0 0-.5-.5h-4A.5.5 0 0 0 9 3z"
                />
              </svg>
            </span>
          {/if}
        </div>
        <div class="now-playing-info">
          <span class="track-name"
            >{$currentTrack?.title ?? $t("noTrackPlaying")}</span
          >
          <div class="artist-marquee" bind:clientWidth={containerWidth}>
            <span
              class="track-artist"
              bind:clientWidth={textWidth}
              class:animate={textWidth > containerWidth}
              style="--scroll-dist: -{textWidth - containerWidth}px"
              >{$currentTrack?.artist ?? "—"}</span
            >
          </div>
        </div>
      </button>
      {#if $currentTrack}
        <button
          class="np-add-btn"
          onclick={onOpenNpPicker}
          title="Add to playlist">+</button
        >
      {/if}
    </div>

    <!-- PS2 action hints -->
    {#if !selectedAlbum && !selectedPlaylist}
      <div class="actions">
        <div class="action-hint">
          <PS2Btn type="cross" />
          <span class="btn-label">{$t("select")}</span>
        </div>
        <button class="action-hint action-btn" onclick={onToggleSearch}>
          <PS2Btn type="circle" />
          <span class="btn-label" class:active-search={searchOpen}
            >{$t("search")}</span
          >
        </button>
        <button class="action-hint action-btn" onclick={onShuffleAll}>
          <PS2Btn type="square" />
          <span class="btn-label" class:active-shuffle={$isShuffled}
            >{$t("shuffle")}</span
          >
        </button>
        <button class="action-hint action-btn" onclick={onToggleRepeat}>
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
          onclick={onOpenOptions}
          title="Options"
        >
          <span class="gear-icon">
            <svg
              viewBox="0 0 16 16"
              width="14"
              height="14"
              fill="currentColor"
              style="display: inline-block; vertical-align: middle;"
            >
              <path
                d="M9.405 1.05c-.413-1.4-2.397-1.4-2.81 0l-.1.34a1.464 1.464 0 0 1-2.105.872l-.31-.17c-1.283-.698-2.686.705-1.987 1.987l.169.311c.446.82.023 1.841-.872 2.105l-.34.1c-1.4.413-1.4 2.397 0 2.81l.34.1a1.464 1.464 0 0 1 .872 2.105l-.17.31c-.698 1.283.705 2.686 1.987 1.987l.311-.169a1.464 1.464 0 0 1 2.105.872l.1.34c.413 1.4 2.397 1.4 2.81 0l.1-.34a1.464 1.464 0 0 1 2.105-.872l.31.17c1.283.698 2.686-.705 1.987-1.987l-.169-.311a1.464 1.464 0 0 1 .872-2.105l.34-.1c1.4-.413 1.4-2.397 0-2.81l-.34-.1a1.464 1.464 0 0 1-.872-2.105l.17-.31c.698-1.283-.705-2.686-1.987-1.987l-.311.169a1.464 1.464 0 0 1-2.105-.872zM8 10.93a2.929 2.929 0 1 1 0-5.86 2.929 2.929 0 0 1 0 5.86z"
              />
            </svg>
          </span>
          <span class="options-gp-tag">L3</span>
        </button>
      </div>
    {/if}
  </div>
  <!-- /footer-bottom -->
</footer>
