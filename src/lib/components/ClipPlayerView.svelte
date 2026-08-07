<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Channel } from "@tauri-apps/api/core";
  import { FrameRenderer } from "$lib/webgl/FrameRenderer";
  import {
    playClip,
    pauseClip,
    resumeClip,
    seekClip,
    stopClip,
    clipPosition,
    clipDuration,
    clipFinished,
    computePlaybackBox,
  } from "$lib/stores/clips";
  import { viewMode } from "$lib/stores/viewMode";
  import VolumeControl from "./VolumeControl.svelte";
  import PS2Btn from "./PS2Btn.svelte";
  import { t } from "$lib/stores/i18n";
  import { playUiSfx } from "$lib/ui-sfx";
  import type { Clip } from "$lib/types";

  let { clip, onclose }: { clip: Clip; onclose: () => void } = $props();

  // Each ClipPlayerView instance is remounted fresh per clip (see
  // +page.svelte's {#if $selectedClip}), so `clip` never changes within a
  // single instance's lifetime — $derived here just silences the
  // state_referenced_locally lint, not for reactivity.
  const box = $derived(computePlaybackBox(clip));

  let canvas: HTMLCanvasElement;
  let renderer: FrameRenderer | null = null;
  let isPlaying = $state(true);
  let rafId = 0;

  // Diagnosed directly (see clips.ts's PLAYBACK_BOUND comment for the full
  // trail): on this WebKitGTK+NVIDIA combination, canvas compositing
  // throughput craters somewhere between ~230K and ~507K displayed pixels,
  // regardless of *how* that pixel count is reached — a bigger backing
  // texture and CSS-scaling a small texture up (whether via width/height+
  // object-fit or via `transform: scale()`) all throttled equally in
  // controlled tests. There is currently no way to display this clip
  // larger than its native decoded size without reintroducing the
  // throttle, so canvasScale only ever shrinks to fit (never grows beyond
  // 1) — the video is centered at its natural size rather than stretched
  // to fill the window on this platform.
  let stageEl = $state<HTMLDivElement | null>(null);
  let canvasScale = $state(1);
  let resizeObserver: ResizeObserver | null = null;

  function updateCanvasScale() {
    if (!stageEl) return;
    // clientWidth/Height (NOT getBoundingClientRect) — the app's .root has
    // its own `transform: scale(1.5)`, and getBoundingClientRect reports
    // POST-ancestor-transform viewport pixels while box.w/h are in the
    // canvas's LOCAL (pre-transform) box space. clientWidth/Height stay in
    // the local, untransformed coordinate space, matching box.w/h.
    const cw = stageEl.clientWidth;
    const ch = stageEl.clientHeight;
    if (cw <= 0 || ch <= 0) return;
    canvasScale = Math.min(1, cw / box.w, ch / box.h);
  }

  // Progress bar — self-contained rather than reusing ProgressBar.svelte,
  // which is tightly coupled to player.ts's currentTrack/duration/position
  // (clip playback deliberately bypasses that store, see clips.ts).
  let barEl = $state<HTMLButtonElement | null>(null);
  let isDragging = $state(false);
  let dragPosition = $state(0);
  let activePointerId = $state<number | null>(null);

  function fmt(value: number): string {
    const totalSeconds = Math.max(0, Math.floor(value));
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, "0")}`;
  }

  function clampPosition(clientX: number): number {
    if (!barEl || $clipDuration <= 0) return 0;
    const rect = barEl.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return ratio * $clipDuration;
  }

  function handlePointerDown(event: PointerEvent) {
    if ($clipDuration <= 0 || !barEl) return;
    activePointerId = event.pointerId;
    isDragging = true;
    dragPosition = clampPosition(event.clientX);
    barEl.setPointerCapture(event.pointerId);
  }

  function handlePointerMove(event: PointerEvent) {
    if (!isDragging || activePointerId !== event.pointerId) return;
    dragPosition = clampPosition(event.clientX);
  }

  async function handlePointerUp(event: PointerEvent) {
    if (!isDragging || activePointerId !== event.pointerId) return;
    const next = clampPosition(event.clientX);
    isDragging = false;
    activePointerId = null;
    if (barEl?.hasPointerCapture(event.pointerId)) {
      barEl.releasePointerCapture(event.pointerId);
    }
    await seekClip(next);
  }

  let canSeek = $derived($clipDuration > 0);
  let displayPosition = $derived(isDragging ? dragPosition : $clipPosition);
  let pct = $derived($clipDuration > 0 ? (displayPosition / $clipDuration) * 100 : 0);

  // Controls (including the close button, top-right) auto-hide on
  // inactivity and reappear on ANY mouse movement — a bottom-half-only
  // trigger zone (as FocusView uses) left the close button unreachable
  // once hidden, since hovering directly over it (top corner) never
  // re-triggered visibility.
  let controlsVisible = $state(true);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let rootEl = $state<HTMLDivElement | null>(null);

  function onMouseMove() {
    controlsVisible = true;
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => (controlsVisible = false), 2500);
  }

  function draw() {
    renderer?.draw();
    rafId = requestAnimationFrame(draw);
  }

  onMount(async () => {
    renderer = new FrameRenderer(canvas, box.w, box.h);

    updateCanvasScale();
    if (stageEl) {
      resizeObserver = new ResizeObserver(updateCanvasScale);
      resizeObserver.observe(stageEl);
    }

    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (data) => {
      renderer?.uploadFrame(data);
    };

    await playClip(clip, channel);
    isPlaying = true;
    rafId = requestAnimationFrame(draw);

    hideTimer = setTimeout(() => (controlsVisible = false), 2500);
  });

  onDestroy(() => {
    if (rafId) cancelAnimationFrame(rafId);
    if (hideTimer) clearTimeout(hideTimer);
    resizeObserver?.disconnect();
    stopClip();
    renderer?.dispose();
  });

  // clip_video/audio report end-of-stream via clipFinished — close automatically.
  $effect(() => {
    if ($clipFinished) close();
  });

  async function togglePlayPause() {
    playUiSfx("confirm");
    if (isPlaying) {
      await pauseClip();
      isPlaying = false;
    } else {
      await resumeClip();
      isPlaying = true;
    }
  }

  function close() {
    playUiSfx("back");
    if ($viewMode !== "normal") viewMode.set("normal");
    onclose();
  }

  // ── Gamepad API (called from +page.svelte's handleGamepadClipPlayer) ──────
  export function gamepadTogglePlayPause() {
    togglePlayPause();
  }

  export async function gamepadSeekBy(deltaSecs: number) {
    if ($clipDuration <= 0) return;
    const next = Math.max(0, Math.min($clipDuration, $clipPosition + deltaSecs));
    await seekClip(next);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="clip-player-root" bind:this={rootEl} onmousemove={onMouseMove}>
  <button
    class="close-btn"
    class:close-btn--visible={controlsVisible}
    onclick={close}
    aria-label="Close"
  >
    ✕
  </button>

  <div class="stage" bind:this={stageEl}>
    <canvas
      bind:this={canvas}
      width={box.w}
      height={box.h}
      style={canvasScale < 0.999 ? `transform: scale(${canvasScale})` : ""}
    ></canvas>
  </div>

  <div class="title-bar" class:title-bar--visible={controlsVisible}>
    <span class="title">{clip.title}</span>
  </div>

  <div class="controls" class:controls--visible={controlsVisible}>
    <div class="progress-wrap">
      <span class="time">{fmt(displayPosition)}</span>
      <button
        bind:this={barEl}
        type="button"
        class="bar"
        class:bar--interactive={canSeek}
        disabled={!canSeek}
        aria-label="Seek clip position"
        onpointerdown={handlePointerDown}
        onpointermove={handlePointerMove}
        onpointerup={handlePointerUp}
      >
        <div class="fill" class:fill--dragging={isDragging} style={`width:${pct}%`}></div>
        <div class="thumb" class:thumb--dragging={isDragging} style={`left:${pct}%`}></div>
      </button>
      <span class="time">{fmt($clipDuration)}</span>
    </div>

    <div class="controls-row">
      <button class="ctrl-btn ctrl-btn--play" onclick={togglePlayPause}>
        <PS2Btn type="start" />
        <span class="ctrl-label">{isPlaying ? $t("pause") : $t("play")}</span>
      </button>
      <div class="controls-volume">
        <VolumeControl />
      </div>
    </div>
  </div>
</div>

<style>
  .clip-player-root {
    position: absolute;
    inset: 0;
    z-index: 250;
    background: #0a0a10;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    animation: fade-in 0.25s ease;
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .close-btn {
    position: absolute;
    top: 14px;
    right: 14px;
    z-index: 1;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 1px solid rgba(212, 219, 240, 0.15);
    background: linear-gradient(180deg, rgb(48, 48, 48), rgb(30, 30, 34));
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 13px;
    line-height: 1;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.22s ease;
  }

  .close-btn--visible {
    opacity: 1;
    pointer-events: auto;
  }

  .close-btn:hover {
    color: var(--text-primary);
  }

  .stage {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    flex: 1;
    min-height: 0;
  }

  canvas {
    /* Rendered at its native backing size (box.w/box.h) and fit into .stage
       via a JS-computed `transform: scale()` (see canvasScale/
       updateCanvasScale) rather than CSS width/height + object-fit — see
       the canvasScale comment above for why. */
    display: block;
    background: #000;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.5);
  }

  .title-bar {
    position: absolute;
    top: 14px;
    left: 20px;
    right: 60px;
    opacity: 0;
    transition: opacity 0.22s ease;
    pointer-events: none;
  }

  .title-bar--visible {
    opacity: 1;
  }

  .title {
    font-size: 13px;
    color: var(--text-primary);
    text-shadow: 0 1px 3px rgba(0, 0, 0, 0.8);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    display: block;
  }

  .controls {
    /* Absolutely positioned (like .title-bar) rather than a normal flex
       child of .clip-player-root — a flex child still reserves its layout
       height even at opacity:0, which was silently shrinking .stage's
       available height (and showing as a black strip at the bottom, since
       the reserved-but-invisible controls area sits on the page's own
       black background). */
    position: absolute;
    left: 50%;
    bottom: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 10px 0 18px;
    width: 100%;
    max-width: min(90%, 1100px);
    opacity: 0;
    transform: translateX(-50%) translateY(6px);
    transition:
      opacity 0.22s ease,
      transform 0.22s ease;
    pointer-events: none;
  }

  .controls--visible {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
    pointer-events: auto;
  }

  .progress-wrap {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 0 12px;
  }

  .time {
    font-size: 13px;
    color: var(--text-dim);
    min-width: 34px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .bar {
    position: relative;
    flex: 1;
    height: 12px;
    padding: 0;
    border: none;
    background: none;
    cursor: default;
    overflow: visible;
    touch-action: none;
  }

  .bar::before {
    content: "";
    position: absolute;
    inset: 4px 0;
    background: rgba(90, 95, 120, 0.25);
    border-radius: 999px;
  }

  .bar:disabled {
    opacity: 0.6;
  }

  .bar--interactive {
    cursor: pointer;
  }

  .fill {
    position: absolute;
    left: 0;
    top: 4px;
    height: 4px;
    background: var(--text-secondary);
    border-radius: 999px;
    transition: width 0.9s linear;
  }

  .fill--dragging {
    transition: none;
  }

  .thumb {
    position: absolute;
    top: 50%;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--track-active);
    box-shadow: 0 0 10px rgba(168, 194, 255, 0.35);
    transform: translate(-50%, -50%) scale(0.9);
    opacity: 0;
    transition: left 0.9s linear, opacity 0.15s ease, transform 0.15s ease;
  }

  .bar--interactive:hover .thumb,
  .thumb--dragging {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1);
  }

  .thumb--dragging {
    transition: opacity 0.1s ease, transform 0.1s ease;
  }

  .controls-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 24px;
  }

  .ctrl-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: linear-gradient(180deg, rgb(48, 48, 48), rgb(54, 58, 68));
    border: 1px solid rgba(212, 219, 240, 0.1);
    box-shadow:
      0 2px 6px rgba(0, 0, 0, 0.25),
      inset 0 1px 0 rgba(255, 255, 255, 0.08);
    cursor: pointer;
    color: var(--text-secondary);
    padding: 6px 14px;
    border-radius: 999px;
    font-size: 13px;
    transition: color 0.15s;
  }

  .ctrl-btn:hover {
    color: var(--text-primary);
  }

  .ctrl-label {
    min-width: 5ch;
    text-align: left;
    font-size: 10px;
  }
</style>
