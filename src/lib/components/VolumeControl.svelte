<script lang="ts">
  import { volume, setVolume } from '../stores/player';

  const STEPS = 10;

  let level = $derived(Math.round($volume * STEPS));

  function setLevel(n: number) {
    setVolume(n / STEPS);
  }
</script>

<div class="vol">
  <span class="vol-label">VOL</span>
  <div class="bars">
    {#each Array(STEPS) as _, i}
      <button
        class="bar"
        class:filled={i < level}
        onclick={() => setLevel(i + 1)}
        style="height: {7 + i * 2.2}px"
        aria-label="Volume {i + 1}"
      ></button>
    {/each}
  </div>
  <span class="vol-num">{level * 10}</span>
</div>

<style>
  .vol {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }

  .vol-label {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-dim);
    letter-spacing: 0.08em;
    padding-bottom: 1px;
  }

  .bars {
    display: flex;
    align-items: flex-end;
    gap: 2px;
  }

  .bar {
    width: 5px;
    background: var(--text-dim);
    border: none;
    cursor: pointer;
    padding: 0;
    opacity: 0.3;
    transition: opacity 0.1s;
  }

  .bar.filled {
    opacity: 1;
    background: var(--text-secondary);
  }

  .bar:hover {
    opacity: 0.8;
  }

  .vol-num {
    font-size: 10px;
    color: var(--text-dim);
    padding-bottom: 1px;
    min-width: 20px;
  }
</style>
