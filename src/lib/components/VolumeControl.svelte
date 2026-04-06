<script lang="ts">
  import { volume, setVolume } from '../stores/player';
  import { playUiSfx } from '$lib/ui-sfx';

  const STEP_SIZE = 5; 
  const STEPS = 100 / STEP_SIZE; // Это 20
  const VISUAL_BARS = 10; // Сколько палок видим
  const STEPS_PER_BAR = STEPS / VISUAL_BARS; // Это 2

  let level = $derived(Math.round($volume * STEPS));

  function setLevel(n: number) {
    if (n === level) return;
    playUiSfx('steps');
    setVolume(n / STEPS);
  }

  function stepLevel(delta: number) {
    const next = Math.max(0, Math.min(STEPS, level + delta));
    setLevel(next);
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    stepLevel(e.deltaY < 0 ? 1 : -1);
  }
</script>

<div class="vol" onwheel={onWheel}>
  <span class="vol-label">VOL</span>
  
  <button
    class="trigger-btn"
    onclick={() => stepLevel(-1)}
    disabled={level === 0}
    aria-label="Decrease volume"
  >
    <span class="trigger-tag">L2</span>
    <span class="trigger-mark">-</span>
  </button>

  <div class="bars">
    {#each Array(VISUAL_BARS) as _, i}
      <button
        class="bar"
        /* Полоска светится, если текущий уровень покрывает её хотя бы частично */
        class:filled={level >= (i + 1) * STEPS_PER_BAR}
        /* При клике ставим уровень, соответствующий этой полоске */
        onclick={() => setLevel((i + 1) * STEPS_PER_BAR)}
        style="height: {5 + i * 2.2}px" 
        aria-label="Volume bar {i + 1}"
      ></button>
    {/each}
  </div>

  <button
    class="trigger-btn"
    onclick={() => stepLevel(1)}
    disabled={level === STEPS}
    aria-label="Increase volume"
  >
    <span class="trigger-tag">R2</span>
    <span class="trigger-mark">+</span>
  </button>

  <span class="vol-num">{level * STEP_SIZE}</span>
</div>

<style>
  /* Стили остаются почти такими же, поправлю только высоту и шаг */
  .vol {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .vol-label {
    font-size: 10px;
    font-weight: 800;
    color: var(--text-dim);
    text-shadow: var(--text-shadow);
    letter-spacing: 0.08em;
  }

  .trigger-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 30px;
    padding: 2px 7px;
    border: 1px solid rgba(212, 219, 240, 0.12);
    border-radius: 999px;
    background: linear-gradient(180deg, rgb(48, 48, 48), rgb(54, 58, 68));
    color: var(--text-secondary);
    cursor: pointer;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.2), inset 0 1px 0 rgba(255, 255, 255, 0.1);
    transition: all 0.12s;
  }

  .trigger-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .bars {
    display: flex;
    align-items: flex-end;
    gap: 2px; /* Чуть увеличил зазор, так как палок меньше */
  }

  .bar {
    width: 4px; /* Сделал чуть шире для красоты */
    background: var(--text-dim);
    border: none;
    cursor: pointer;
    padding: 0;
    opacity: 0.3;
    transition: opacity 0.1s, background 0.1s;
    border-radius: 2px;
  }

  .bar.filled {
    opacity: 1;
    background: linear-gradient(180deg, #7fd0ff, #3b79ff);
    box-shadow: 0 0 5px rgba(86, 143, 255, 0.28);
  }

  .vol-num {
    font-size: 10px;
    font-weight: 800;
    color: var(--text-dim);
    min-width: 20px;
  }
</style>