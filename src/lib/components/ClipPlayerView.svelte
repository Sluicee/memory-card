<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import {
    buildClipStreamUrl,
    prepareClipPlayback,
    computePlaybackBox,
    PLAYBACK_BOUND_MIN,
    PLAYBACK_BOUND_MAX,
  } from "$lib/stores/clips";
  import { volume } from "$lib/stores/player";
  import { viewMode } from "$lib/stores/viewMode";
  import VolumeControl from "./VolumeControl.svelte";
  import PS2Btn from "./PS2Btn.svelte";
  import { t } from "$lib/stores/i18n";
  import { playUiSfx } from "$lib/ui-sfx";
  import type { Clip } from "$lib/types";

  let { clip, onclose }: { clip: Clip; onclose: () => void } = $props();

  // svelte-ignore state_referenced_locally -- `clip` is stable for this
  // component's whole lifetime (remounted fresh per clip, see below); a
  // plain const is intentional here, not a missed-reactivity bug.
  const clipDurationVal = clip.duration || 0;

  // Each ClipPlayerView instance is remounted fresh per clip (see
  // +page.svelte's {#if $selectedClip}), so `clip` never changes within a
  // single instance's lifetime. `box` only matters for the *transcode*
  // fallback path (clip_stream_server.rs ignores it for a remux, which
  // always sends the source's own resolution) — it tracks .stage's actual
  // measured size (see measureTargetLongSide/handleStageResize below) so a
  // transcode targets exactly what's going to be displayed, whatever the
  // window size, fullscreen or not.
  //
  // svelte-ignore state_referenced_locally -- see above; only `box` itself
  // needs to be reactive here, which it is ($state, reassigned by
  // handleStageResize).
  let box = $state(computePlaybackBox(clip, PLAYBACK_BOUND_MIN));

  let videoEl: HTMLVideoElement;

  // Absolute position = seekBaseSecs (where the current stream request
  // started) + playedSecs (how far the <video> element has played since,
  // mirrored from its own currentTime via the timeupdate event — every
  // seek/resolution-switch is a *fresh* HTTP request starting its own
  // stream at t=0, so the element's own currentTime alone isn't the real
  // position).
  let seekBaseSecs = $state(0);
  let playedSecs = $state(0);
  let absolutePosition = $derived(seekBaseSecs + playedSecs);

  let isPlaying = $state(true);
  // `started`: true once the video has ever actually started playing
  // (never reset back to false — a later mid-playback stall uses
  // `buffering` instead, not this). `buffering`: true whenever the
  // browser is waiting for more data, whether that's the initial load, a
  // seek, or a real network stall — native <video> `waiting`/`playing`
  // events already carry exactly this distinction. Reassigning `src`
  // (every seek, every resolution switch) cleanly discards whatever was
  // loading before, so — unlike the old raw-frame-over-IPC pipeline —
  // there's no stale-frame race to guard against here at all.
  //
  // Both gate the SAME opaque placeholder in the template, not two
  // different ones — `videoEl.load()` (called on every seek/resolution
  // switch) immediately blanks whatever the element was showing, it
  // doesn't hold the last frame the way the old WebGL canvas pipeline
  // did. A translucent "still buffering" overlay over a video that's
  // just gone black reads as a flash to black, not a light dim — so
  // there's no lighter state to have here, only "showing real video" vs
  // "not," covered by the placeholder either way.
  let started = $state(false);
  let buffering = $state(false);

  async function doSeek(secs: number, targetBox: { w: number; h: number } = box) {
    // Set *before* load(), not left to the native `waiting` event — that
    // event is for stalling mid-playback when data runs out, not
    // reliably fired for the moment load() itself blanks the element.
    // Without this, the placeholder wouldn't show at all for however
    // long it took `waiting` (if ever) to fire, leaving a plain black
    // rectangle where the video used to be — a real bug, not just a
    // cosmetic gap.
    buffering = true;
    seekBaseSecs = secs;
    playedSecs = 0;
    const wasPlaying = !videoEl.paused;
    videoEl.src = await buildClipStreamUrl(clip.path, secs, targetBox);
    videoEl.load();
    if (wasPlaying) {
      try {
        await videoEl.play();
      } catch {
        /* autoplay/interrupted-play rejection — harmless, user can hit play */
      }
    }
  }

  let stageEl = $state<HTMLDivElement | null>(null);
  let resizeObserver: ResizeObserver | null = null;

  // .root's own CSS transform (page-shell.css) — clientWidth/Height are
  // measured in the space *inside* this transform, but what actually hits
  // the screen is that times this scale.
  const ROOT_SCALE = 1.5;

  function measureTargetLongSide(): number {
    const side = stageEl ? Math.max(stageEl.clientWidth, stageEl.clientHeight) * ROOT_SCALE : 0;
    return Math.min(PLAYBACK_BOUND_MAX, Math.max(PLAYBACK_BOUND_MIN, side || PLAYBACK_BOUND_MIN));
  }

  // Re-requests the stream at the current position, at the new target
  // resolution, whenever .stage's measured size changes the *transcode*
  // target (see `box` above) — fires on any resize, e.g. the fullscreen
  // toggle button right in this component. A no-op for a remuxed clip
  // (server-side box is ignored there), but cheap either way, and this
  // component has no way to know in advance which path a given clip will
  // take.
  function handleStageResize() {
    const nextBox = computePlaybackBox(clip, measureTargetLongSide());
    if (nextBox.w === box.w && nextBox.h === box.h) return;
    box = nextBox;
    if (!started) return;
    doSeek(absolutePosition, nextBox);
  }

  $effect(() => {
    if (videoEl) videoEl.volume = $volume;
  });

  // Progress bar — self-contained rather than reusing ProgressBar.svelte,
  // which is tightly coupled to player.ts's currentTrack/duration/position
  // (clip playback deliberately bypasses that store).
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
    if (!barEl || clipDurationVal <= 0) return 0;
    const rect = barEl.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return ratio * clipDurationVal;
  }

  function handlePointerDown(event: PointerEvent) {
    if (clipDurationVal <= 0 || !barEl) return;
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

  let canSeek = $derived(clipDurationVal > 0);
  let displayPosition = $derived(isDragging ? dragPosition : absolutePosition);
  let pct = $derived(clipDurationVal > 0 ? (displayPosition / clipDurationVal) * 100 : 0);

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

  function onTimeUpdate() {
    playedSecs = videoEl.currentTime;
  }

  function onWaiting() {
    buffering = true;
  }

  function onPlaying() {
    buffering = false;
    started = true;
  }

  // Fires once the frame at the current position is actually decoded and
  // ready to paint — the clear signal for a seek made *while paused*
  // (doSeek doesn't call play() then, so `playing` never fires; without
  // this, the placeholder would stay up forever after a paused seek).
  function onLoadedData() {
    buffering = false;
    started = true;
  }

  function onPlayEvt() {
    isPlaying = true;
  }

  function onPauseEvt() {
    isPlaying = false;
  }

  onMount(async () => {
    await prepareClipPlayback();

    // Real measurement now that .stage actually exists and is laid out —
    // the $state initializer above only had a placeholder to work with.
    box = computePlaybackBox(clip, measureTargetLongSide());
    if (stageEl) {
      // Also fires once immediately on observe() with the current size —
      // harmless no-op here since box already matches (just measured
      // above), but is what picks up every later resize (fullscreen
      // toggle, etc.).
      resizeObserver = new ResizeObserver(handleStageResize);
      resizeObserver.observe(stageEl);
    }

    videoEl.volume = $volume;
    videoEl.addEventListener("timeupdate", onTimeUpdate);
    videoEl.addEventListener("waiting", onWaiting);
    videoEl.addEventListener("playing", onPlaying);
    videoEl.addEventListener("loadeddata", onLoadedData);
    videoEl.addEventListener("play", onPlayEvt);
    videoEl.addEventListener("pause", onPauseEvt);
    videoEl.addEventListener("ended", close);

    videoEl.src = await buildClipStreamUrl(clip.path, 0, box);
    videoEl.load();
    try {
      await videoEl.play();
    } catch {
      /* autoplay rejection — user can hit play */
    }

    hideTimer = setTimeout(() => (controlsVisible = false), 2500);
  });

  onDestroy(() => {
    if (hideTimer) clearTimeout(hideTimer);
    resizeObserver?.disconnect();
    if (videoEl) {
      videoEl.removeEventListener("timeupdate", onTimeUpdate);
      videoEl.removeEventListener("waiting", onWaiting);
      videoEl.removeEventListener("playing", onPlaying);
      videoEl.removeEventListener("loadeddata", onLoadedData);
      videoEl.removeEventListener("play", onPlayEvt);
      videoEl.removeEventListener("pause", onPauseEvt);
      videoEl.removeEventListener("ended", close);
      // Forces the in-flight HTTP request to abort immediately (rather
      // than whenever GC gets around to dropping the element), so the
      // server-side ffmpeg process (transcode or remux) is killed
      // promptly — see clip_stream_server.rs's ChildGuard.
      videoEl.pause();
      videoEl.removeAttribute("src");
      videoEl.load();
    }
  });

  function togglePlayPause() {
    playUiSfx("confirm");
    if (videoEl.paused) {
      videoEl.play().catch(() => {});
    } else {
      videoEl.pause();
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
    if (clipDurationVal <= 0) return;
    const next = Math.max(0, Math.min(clipDurationVal, absolutePosition + deltaSecs));
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
    <!-- svelte-ignore a11y_media_has_caption -->
    <video bind:this={videoEl} playsinline></video>

    {#if !started || buffering}
      <div class="load-placeholder">
        {#if clip.thumbnail}
          <img class="load-thumb" src={convertFileSrc(clip.thumbnail)} alt="" />
        {/if}
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
      <span class="time">{fmt(clipDurationVal)}</span>
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

  video {
    max-width: 100%;
    max-height: 100%;
    display: block;
    background: #000;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.5);
  }

  .load-placeholder {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
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
