<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import SpinningCover from "./SpinningCover.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import PS2Btn from "./PS2Btn.svelte";
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
  } from "$lib/stores/player";
  import { albums } from "$lib/stores/library";
  import { playUiSfx } from "$lib/ui-sfx";
  import { t } from "$lib/stores/i18n";
  import VolumeControl from "./VolumeControl.svelte";

  let bgColor = $state("rgb(14, 16, 28)");

  const coverSrc = $derived(
    $currentAlbum?.cover_art ? convertFileSrc($currentAlbum.cover_art) : null,
  );

  $effect(() => {
    if (coverSrc) {
      extractBg(coverSrc).then((c) => (bgColor = c));
    } else {
      bgColor = "rgb(14, 16, 28)";
    }
  });

  async function extractBg(src: string): Promise<string> {
    try {
      const bitmap = await createImageBitmap(
        await fetch(src).then((r) => r.blob()),
        { resizeWidth: 8, resizeHeight: 8, resizeQuality: "low" },
      );
      const canvas = document.createElement("canvas");
      canvas.width = 8;
      canvas.height = 8;
      const ctx = canvas.getContext("2d")!;
      ctx.drawImage(bitmap, 0, 0);
      bitmap.close();
      const data = ctx.getImageData(0, 0, 8, 8).data;
      let r = 0,
        g = 0,
        b = 0;
      const px = data.length / 4;
      for (let i = 0; i < data.length; i += 4) {
        r += data[i];
        g += data[i + 1];
        b += data[i + 2];
      }
      // Same hue as album art, but brighter and slightly more saturated
      r = Math.round((r / px) * 0.95 + 8);
      g = Math.round((g / px) * 0.95 + 8);
      b = Math.round((b / px) * 0.95 + 8);
      return `rgb(${r}, ${g}, ${b})`;
    } catch {
      return "rgb(14, 16, 28)";
    }
  }

  // Controls auto-hide: show only when mouse moves in bottom 50%
  let controlsVisible = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let rootEl = $state<HTMLDivElement | null>(null);

  function onMouseMove(e: MouseEvent) {
    if (!rootEl) return;
    const rect = rootEl.getBoundingClientRect();
    const relY = e.clientY - rect.top;
    if (relY > rect.height * 0.5) {
      controlsVisible = true;
      if (hideTimer) clearTimeout(hideTimer);
      hideTimer = setTimeout(() => {
        controlsVisible = false;
        hideTimer = null;
      }, 2000);
    } else {
      if (hideTimer) {
        clearTimeout(hideTimer);
        hideTimer = null;
      }
      controlsVisible = false;
    }
  }

  async function handlePlayPause() {
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

  async function handleToggleRepeat() {
    playUiSfx("confirm");
    toggleRepeat();
  }

  async function handleToggleShuffle() {
    playUiSfx("confirm");
    await playShuffledAll($albums);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="focus-root"
  style="background: {bgColor};"
  bind:this={rootEl}
  onmousemove={onMouseMove}
>
  <div class="disc-wrap">
    {#if coverSrc}
      <SpinningCover
        src={coverSrc}
        alt={$currentAlbum?.title ?? ""}
        size={260}
      />
    {:else}
      <div class="disc-placeholder">♪</div>
    {/if}
  </div>

  <div class="track-meta">
    <span class="track-name">{$currentTrack?.title ?? "—"}</span>
    <span class="track-artist">{$currentTrack?.artist ?? ""}</span>
    {#if $currentAlbum}
      <span class="track-album"
        >{$currentAlbum.title}{$currentAlbum.year
          ? ` · ${$currentAlbum.year}`
          : ""}</span
      >
    {/if}
  </div>

  <div class="progress-area">
    <ProgressBar />
  </div>

  <div class="controls" class:controls--visible={controlsVisible}>
    <div class="controls-transport">
      <button
        class="ctrl-btn ctrl-btn--secondary"
        class:active={$isShuffled}
        onclick={handleToggleShuffle}
        disabled={!$currentTrack}
        title="Shuffle"
      >
        <PS2Btn type="square" />
        <span class="ctrl-label">{$t("shuffle")}</span>
      </button>

      <button
        class="ctrl-btn ctrl-btn--shoulder"
        onclick={handlePrev}
        disabled={!$currentTrack}
      >
        <span class="shoulder-tag">L1</span>
        <span class="ctrl-icon">&lt;&lt;</span>
      </button>

      <button
        class="ctrl-btn ctrl-btn--play"
        onclick={handlePlayPause}
        disabled={!$currentTrack}
      >
        <PS2Btn type="start" />
        <span class="ctrl-label">{$isPlaying ? "Pause" : "Play"}</span>
      </button>

      <button
        class="ctrl-btn ctrl-btn--shoulder"
        onclick={handleNext}
        disabled={!$currentTrack}
      >
        <span class="shoulder-tag">R1</span>
        <span class="ctrl-icon">&gt;&gt;</span>
      </button>

      <button
        class="ctrl-btn ctrl-btn--secondary"
        class:active={$repeatMode !== "none"}
        onclick={handleToggleRepeat}
        disabled={!$currentTrack}
        title="Repeat"
      >
        <PS2Btn type="triangle" />
        <span class="ctrl-label"
          >{$repeatMode === "one"
            ? $t("repeatOne")
            : $repeatMode === "all"
              ? $t("repeatAll")
              : $t("repeat")}</span
        >
      </button>
    </div>

    <div class="controls-volume">
      <VolumeControl />
    </div>
  </div>
</div>

<style>
  .focus-root {
    position: absolute;
    inset: 0;
    z-index: 200;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    animation: fade-in 0.3s ease;
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .disc-wrap {
    flex-shrink: 0;
  }

  .disc-placeholder {
    width: 260px;
    height: 260px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 72px;
    color: rgba(90, 95, 120, 0.3);
    background: rgba(90, 95, 120, 0.12);
    border-radius: 50%;
  }

  .track-meta {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    text-align: center;
  }

  .track-name {
    font-size: 22px;
    color: var(--text-primary);
    letter-spacing: 0.01em;
    max-width: 500px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .track-artist {
    font-size: 14px;
    color: var(--text-secondary);
  }

  .track-album {
    font-size: 11px;
    color: var(--text-dim);
  }

  /* Force the ProgressBar's .bar to fill the full container width */
  .progress-area {
    display: flex;
    justify-content: center;
    width: 100%;
  }

  :global(.progress-area .progress-wrap) {
    width: 420px;
    max-width: 80vw;
  }

  :global(.progress-area .bar) {
    flex: 1;
    width: auto !important;
    min-width: 0;
  }

  .controls {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    opacity: 0;
    transform: translateY(6px);
    transition:
      opacity 0.22s ease,
      transform 0.22s ease;
    pointer-events: none;
  }

  .controls-transport {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .controls-volume {
    display: flex;
    justify-content: center;
  }

  .controls--visible {
    opacity: 1;
    transform: translateY(0);
    pointer-events: auto;
  }

  .ctrl-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-secondary);
    transition: color 0.15s;
    padding: 6px 10px;
    border-radius: 999px;
  }

  .ctrl-btn:hover:not(:disabled) {
    color: var(--text-primary);
  }
  .ctrl-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .ctrl-btn--shoulder {
    background: linear-gradient(180deg, rgb(48, 48, 48), rgb(54, 58, 68));
    border: 1px solid rgba(212, 219, 240, 0.1);
    box-shadow:
      0 2px 6px rgba(0, 0, 0, 0.25),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);
  }

  .ctrl-btn--play {
    gap: 6px;
    font-size: 13px;
    padding: 6px 14px;
    background: linear-gradient(180deg, rgb(48, 48, 48), rgb(54, 58, 68));
    border: 1px solid rgba(212, 219, 240, 0.1);
    box-shadow:
      0 2px 6px rgba(0, 0, 0, 0.25),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);
  }

  .ctrl-btn--secondary {
    gap: 6px;
    font-size: 7px;
    padding: 6px 14px;
  }

  .ctrl-btn--secondary.active {
    filter: drop-shadow(0 0 4px var(--track-active));
  }

  .shoulder-tag {
    font-size: 9px;
    letter-spacing: 0.08em;
    color: rgba(238, 242, 255, 0.82);
  }

  .ctrl-icon {
    font-size: 11px;
    font-weight: 900;
    color: var(--track-hover);
  }

  .ctrl-label {
    min-width: 5ch;
    text-align: left;
    font-size: 10px;
  }
</style>
