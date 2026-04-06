<script lang="ts">
  import { currentTrack, duration, position, seekTo } from '../stores/player';

  let bar = $state<HTMLButtonElement | null>(null);
  let isDragging = $state(false);
  let dragPosition = $state(0);
  let activePointerId = $state<number | null>(null);

  function fmt(value: number): string {
    const totalSeconds = Math.max(0, Math.floor(value));
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  }

  function clampPosition(clientX: number): number {
    if (!bar || $duration <= 0) return 0;

    const rect = bar.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return ratio * $duration;
  }

  function handlePointerDown(event: PointerEvent) {
    if (!$currentTrack || $duration <= 0 || !bar) return;

    activePointerId = event.pointerId;
    isDragging = true;
    dragPosition = clampPosition(event.clientX);
    bar.setPointerCapture(event.pointerId);
  }

  function handlePointerMove(event: PointerEvent) {
    if (!isDragging || activePointerId !== event.pointerId) return;
    dragPosition = clampPosition(event.clientX);
  }

  async function handlePointerUp(event: PointerEvent) {
    if (!isDragging || activePointerId !== event.pointerId) return;

    const nextPosition = clampPosition(event.clientX);
    dragPosition = nextPosition;
    isDragging = false;
    activePointerId = null;

    if (bar?.hasPointerCapture(event.pointerId)) {
      bar.releasePointerCapture(event.pointerId);
    }

    await seekTo(nextPosition);
  }

  function handlePointerCancel(event: PointerEvent) {
    if (activePointerId !== event.pointerId) return;

    isDragging = false;
    activePointerId = null;

    if (bar?.hasPointerCapture(event.pointerId)) {
      bar.releasePointerCapture(event.pointerId);
    }
  }

  function handleLostPointerCapture() {
    isDragging = false;
    activePointerId = null;
  }

  let canSeek = $derived(!!$currentTrack && $duration > 0);
  let displayPosition = $derived(isDragging ? dragPosition : $position);
  let pct = $derived($duration > 0 ? (displayPosition / $duration) * 100 : 0);
</script>

<div class="progress-wrap">
  <span class="time">{fmt(displayPosition)}</span>
  <button
    bind:this={bar}
    type="button"
    class="bar"
    class:bar--interactive={canSeek}
    disabled={!canSeek}
    aria-label="Seek track position"
    onpointerdown={handlePointerDown}
    onpointermove={handlePointerMove}
    onpointerup={handlePointerUp}
    onpointercancel={handlePointerCancel}
    onlostpointercapture={handleLostPointerCapture}
  >
    <div class="fill" class:fill--dragging={isDragging} style={`width:${pct}%`}></div>
    <div class="thumb" class:thumb--dragging={isDragging} style={`left:${pct}%`}></div>
  </button>
  <span class="time">{fmt($duration)}</span>
</div>

<style>
  .progress-wrap {
    display: flex;
    align-items: center;
    gap: 7px;
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
    width: 200px;
    height: 12px;
    padding: 0;
    border: none;
    background: none;
    cursor: default;
    overflow: visible;
    touch-action: none;
  }

  .bar::before {
    content: '';
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
</style>
