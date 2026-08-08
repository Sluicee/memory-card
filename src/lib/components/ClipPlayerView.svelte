<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { Channel, convertFileSrc } from "@tauri-apps/api/core";
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
    PLAYBACK_BOUND_MIN,
    PLAYBACK_BOUND_MAX,
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
  // single instance's lifetime. `box` DOES change within a lifetime though
  // — the decode target tracks the *actual* measured size of .stage (see
  // measureTargetLongSide/handleStageResize below), not a per-view-mode
  // guess, so it naturally covers windowed, fullscreen, and mini modes the
  // same way. The fullscreen toggle button right in this component (see
  // toggleFullscreen below) makes switching mode — and so resizing .stage
  // — mid-clip a normal thing to do, not an edge case.
  //
  // Real measurement only happens once .stage exists (onMount) — this
  // initial value is just a placeholder until then.
  //
  // svelte-ignore state_referenced_locally -- `clip` is a $props() value,
  // stable for this component's whole lifetime (see above); only `box`
  // itself needs to be reactive here, which it is ($state, reassigned by
  // handleStageResize).
  let box = $state(computePlaybackBox(clip, PLAYBACK_BOUND_MIN));

  let canvas: HTMLCanvasElement;
  let renderer: FrameRenderer | null = null;
  let isPlaying = $state(true);
  let rafId = 0;

  // canvasScale only ever shrinks the canvas to fit .stage, never grows it
  // past 1 — the canvas is already decoded at up to `box`'s target (see
  // clips.ts), so stretching it further via CSS would just upscale and
  // blur/block it for no benefit (ffmpeg's own scale= filter is what's
  // responsible for filling the target box, per clip, up to that bound).
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
    await doSeek(next);
  }

  // `started`: true once a frame belonging to the current decode has
  // actually arrived (gates the initial-load placeholder). `seeking`: true
  // from the moment a seek is requested until a frame belonging to *that*
  // seek arrives (gates the lighter buffering overlay).
  //
  // Every frame's bytes are prefixed on the Rust side with an 8-byte
  // generation tag (see clip_video.rs's tick()) that channel.onmessage
  // checks against this component's own `generation` counter before
  // accepting it. Needed because frames stream continuously during
  // playback, so one from the *previous* position is essentially always
  // still in flight over IPC at the moment a seek is requested — without
  // the tag there's no way to tell it apart from a genuinely new one, and
  // treating "any frame arrived" as confirmation cleared `seeking` almost
  // instantly while the real seek was still 1-3s out, which just read as
  // "the picture hangs." `generation` is bumped synchronously (no `await`
  // in between) right before each Start/Seek call and passed through as a
  // parameter, so channel.onmessage always compares against the request
  // that's actually in flight, not a stale snapshot.
  let started = $state(false);
  let seeking = $state(false);
  let generation = 0;

  async function doSeek(secs: number, targetBox: { w: number; h: number } = box) {
    generation++;
    seeking = true;
    await seekClip(secs, generation, targetBox);
  }

  // .root's own CSS transform (page-shell.css) — clientWidth/Height are
  // measured in the space *inside* this transform, but what actually hits
  // the screen is that times this scale. Missing this the first time
  // under-targeted the decode resolution by exactly this factor (measured
  // ~633px internal for the windowed case, displayed at ~950px physical) —
  // visible softness, not just "slightly conservative."
  const ROOT_SCALE = 1.5;

  // Longer side of .stage's *actual* current layout box, converted to real
  // screen pixels — measuring instead of guessing per view mode (was: 1280
  // windowed / screen resolution fullscreen) targets exactly what's going
  // to be displayed, whatever the window size, fullscreen or not.
  // clientWidth/Height (not getBoundingClientRect) for the same reason
  // updateCanvasScale uses them — .root's transform makes
  // getBoundingClientRect report post-transform viewport pixels directly,
  // but mixing that with box.w/h (which canvasScale math needs in the
  // *local* pre-transform space) elsewhere is what this file avoids
  // throughout; ROOT_SCALE applies the same correction explicitly, in one
  // place, only for the decode-resolution target.
  function measureTargetLongSide(): number {
    const side = stageEl ? Math.max(stageEl.clientWidth, stageEl.clientHeight) * ROOT_SCALE : 0;
    return Math.min(PLAYBACK_BOUND_MAX, Math.max(PLAYBACK_BOUND_MIN, side || PLAYBACK_BOUND_MIN));
  }

  // Re-seeks at the current position, at the new target resolution,
  // whenever .stage's measured size actually changes the decode target
  // (see `box` above) — fires on any resize, e.g. the fullscreen toggle
  // button right in this component. Unconditional rather than gated on
  // readiness — a Seek always tears down and restarts whatever decode is
  // currently in flight on the backend (clip_video.rs), so it's safe to
  // fire even mid-initial-buffering; it just means the fresh box wins over
  // whatever the original Start was using.
  function handleStageResize() {
    updateCanvasScale();
    const nextBox = computePlaybackBox(clip, measureTargetLongSide());
    if (nextBox.w === box.w && nextBox.h === box.h) return;
    box = nextBox;
    if (!renderer) return;
    renderer.resize(nextBox.w, nextBox.h);
    updateCanvasScale();
    doSeek($clipPosition, nextBox);
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
    // Real measurement now that .stage actually exists and is laid out —
    // the $state initializer above only had a placeholder to work with.
    box = computePlaybackBox(clip, measureTargetLongSide());
    renderer = new FrameRenderer(canvas, box.w, box.h);

    updateCanvasScale();
    if (stageEl) {
      // Also fires once immediately on observe() with the current size —
      // harmless no-op here since box already matches (just measured
      // above), but is what picks up every later resize (fullscreen
      // toggle, etc.).
      resizeObserver = new ResizeObserver(handleStageResize);
      resizeObserver.observe(stageEl);
    }

    const channel = new Channel<ArrayBuffer>();
    channel.onmessage = (data) => {
      // First 8 bytes are the generation tag (little-endian u64); the rest
      // is the raw RGBA payload FrameRenderer expects. Discard anything
      // not tagged for the request currently in flight — see the
      // `generation` comment above.
      const frameGen = new DataView(data).getBigUint64(0, true);
      if (frameGen !== BigInt(generation)) return;
      renderer?.uploadFrame(data, 8);
      started = true;
      seeking = false;
    };

    generation++;
    await playClip(clip, channel, generation, box);
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

  // Duplicates ViewModeBar's fullscreen toggle (the top auto-hiding strip)
  // right next to the close button — that strip is technically still
  // reachable while a clip plays (higher z-index than .clip-player-root),
  // but hovering the top-center edge while focused on a video isn't
  // discoverable. Toggles between fullscreen and normal specifically,
  // regardless of which of those two the app was in before the clip opened.
  function toggleFullscreen() {
    playUiSfx("confirm");
    viewMode.set($viewMode === "fullscreen" ? "normal" : "fullscreen");
  }

  // ── Gamepad API (called from +page.svelte's handleGamepadClipPlayer) ──────
  export function gamepadTogglePlayPause() {
    togglePlayPause();
  }

  export async function gamepadSeekBy(deltaSecs: number) {
    if ($clipDuration <= 0) return;
    const next = Math.max(0, Math.min($clipDuration, $clipPosition + deltaSecs));
    await doSeek(next);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="clip-player-root" bind:this={rootEl} onmousemove={onMouseMove}>
  <button
    class="icon-btn fullscreen-btn"
    class:icon-btn--visible={controlsVisible}
    onclick={toggleFullscreen}
    aria-label={$viewMode === "fullscreen" ? "Exit fullscreen" : "Enter fullscreen"}
    title={$viewMode === "fullscreen" ? "Exit fullscreen" : "Enter fullscreen"}
  >
    {#if $viewMode === "fullscreen"}
      <!-- collapse arrows -->
      <svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round">
        <polyline points="3,0 3,3 0,3" />
        <polyline points="7,0 7,3 10,3" />
        <polyline points="10,7 7,7 7,10" />
        <polyline points="0,7 3,7 3,10" />
      </svg>
    {:else}
      <!-- expand arrows -->
      <svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round">
        <polyline points="0,3 0,0 3,0" />
        <polyline points="7,0 10,0 10,3" />
        <polyline points="10,7 10,10 7,10" />
        <polyline points="3,10 0,10 0,7" />
      </svg>
    {/if}
  </button>

  <button
    class="icon-btn close-btn"
    class:icon-btn--visible={controlsVisible}
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

    {#if !started}
      <div class="load-placeholder">
        {#if clip.thumbnail}
          <img class="load-thumb" src={convertFileSrc(clip.thumbnail)} alt="" />
        {/if}
        <div class="spinner"></div>
      </div>
    {:else if seeking}
      <div class="seek-overlay">
        <div class="spinner"></div>
      </div>
    {/if}
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

  .icon-btn {
    position: absolute;
    top: 14px;
    z-index: 1;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
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

  .icon-btn svg {
    width: 12px;
    height: 12px;
    display: block;
  }

  .icon-btn--visible {
    opacity: 1;
    pointer-events: auto;
  }

  .icon-btn:hover {
    color: var(--text-primary);
  }

  .close-btn {
    right: 14px;
  }

  .fullscreen-btn {
    right: 50px;
  }

  .stage {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    flex: 1;
    min-height: 0;
  }

  .load-placeholder,
  .seek-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .load-placeholder {
    background: #0a0a10;
  }

  .load-thumb {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    filter: blur(10px) brightness(0.5);
    transform: scale(1.05);
  }

  .seek-overlay {
    background: rgba(10, 10, 16, 0.45);
  }

  .spinner {
    position: relative;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    border: 3px solid rgba(255, 255, 255, 0.15);
    border-top-color: var(--track-active);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
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
