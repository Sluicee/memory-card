<script lang="ts">
  import {
    equalizerStore,
    isEqualizerOpen,
    setBandGain,
    setPreampGain,
    applyPreset,
    resetEqualizer,
    FACTORY_PRESETS,
    BAND_LABELS,
  } from '$lib/stores/equalizer';
  import PS2Btn from './PS2Btn.svelte';
  import { playUiSfx } from '$lib/ui-sfx';
  import { t } from '$lib/stores/i18n';

  // 0 = Preamp, 1..10 = 10 Band gains
  let focusedIdx = $state<number>(0);

  export function handleGamepadInput(action: string) {
    if (action === 'circle' || action === 'select') {
      closeModal();
      return;
    }

    if (action === 'left') {
      playUiSfx('steps');
      focusedIdx = (focusedIdx - 1 + 11) % 11;
      return;
    }

    if (action === 'right') {
      playUiSfx('steps');
      focusedIdx = (focusedIdx + 1) % 11;
      return;
    }

    if (action === 'up') {
      playUiSfx('steps');
      if (focusedIdx === 0) {
        setPreampGain($equalizerStore.preamp + 1.0, true);
      } else {
        const band = focusedIdx - 1;
        setBandGain(band, $equalizerStore.gains[band] + 1.0, true);
      }
      return;
    }

    if (action === 'down') {
      playUiSfx('steps');
      if (focusedIdx === 0) {
        setPreampGain($equalizerStore.preamp - 1.0, true);
      } else {
        const band = focusedIdx - 1;
        setBandGain(band, $equalizerStore.gains[band] - 1.0, true);
      }
      return;
    }

    if (action === 'triangle') {
      playUiSfx('confirm');
      resetEqualizer();
      return;
    }

    if (action === 'square' || action === 'r1') {
      playUiSfx('confirm');
      cycleNextPreset();
      return;
    }

    if (action === 'l1') {
      playUiSfx('confirm');
      cyclePrevPreset();
      return;
    }
  }

  function cycleNextPreset() {
    const currentId = $equalizerStore.activePresetId;
    const idx = FACTORY_PRESETS.findIndex((p) => p.id === currentId);
    const nextIdx = (idx + 1) % FACTORY_PRESETS.length;
    applyPreset(FACTORY_PRESETS[nextIdx].id);
  }

  function cyclePrevPreset() {
    const currentId = $equalizerStore.activePresetId;
    const idx = FACTORY_PRESETS.findIndex((p) => p.id === currentId);
    const prevIdx = (idx - 1 + FACTORY_PRESETS.length) % FACTORY_PRESETS.length;
    applyPreset(FACTORY_PRESETS[prevIdx].id);
  }

  function handleOverlayMouseDown(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      closeModal();
    }
  }

  function closeModal() {
    playUiSfx('back');
    isEqualizerOpen.set(false);
  }

  function handleSelectPreset(presetId: string) {
    playUiSfx('confirm');
    applyPreset(presetId);
  }

  function handleReset() {
    playUiSfx('confirm');
    resetEqualizer();
  }

  function formatGain(val: number): string {
    if (val > 0) return `+${val.toFixed(1)} dB`;
    return `${val.toFixed(1)} dB`;
  }

  let bassAvg = $derived.by(() => {
    const g = $equalizerStore.gains;
    const avg = (g[0] + g[1] + g[2] + g[3]) / 4;
    return formatGain(avg);
  });

  let midAvg = $derived.by(() => {
    const g = $equalizerStore.gains;
    const avg = (g[4] + g[5] + g[6]) / 3;
    return formatGain(avg);
  });

  let trebleAvg = $derived.by(() => {
    const g = $equalizerStore.gains;
    const avg = (g[7] + g[8] + g[9]) / 3;
    return formatGain(avg);
  });

  // Smooth SVG curve path calculation
  let curvePath = $derived.by(() => {
    const gains = $equalizerStore.gains;
    const width = 450;
    const height = 44;
    const midY = height / 2;
    const points: [number, number][] = gains.map((g, i) => {
      const x = (i / (gains.length - 1)) * (width - 24) + 12;
      const y = midY - (g / 12) * (midY - 5);
      return [x, y];
    });

    if (points.length === 0) return '';

    let path = `M ${points[0][0]},${points[0][1]}`;
    for (let i = 0; i < points.length - 1; i++) {
      const p0 = points[i];
      const p1 = points[i + 1];
      const cpX = (p0[0] + p1[0]) / 2;
      path += ` C ${cpX},${p0[1]} ${cpX},${p1[1]} ${p1[0]},${p1[1]}`;
    }
    return path;
  });

  let fillPath = $derived.by(() => {
    if (!curvePath) return '';
    return `${curvePath} L 438,44 L 12,44 Z`;
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" onmousedown={handleOverlayMouseDown}>
  <div class="panel">
    <!-- Summary Row: Acoustic Balance Info -->
    <div class="summary">
      <div class="stat-card">
        <span class="stat-val">{formatGain($equalizerStore.preamp)}</span>
        <span class="stat-lbl">{$t('preamp')}</span>
      </div>
      <div class="stat-card">
        <span class="stat-val">{bassAvg}</span>
        <span class="stat-lbl">{$t('eqBass')}</span>
      </div>
      <div class="stat-card">
        <span class="stat-val">{midAvg}</span>
        <span class="stat-lbl">{$t('eqMid')}</span>
      </div>
      <div class="stat-card">
        <span class="stat-val">{trebleAvg}</span>
        <span class="stat-lbl">{$t('eqTreble')}</span>
      </div>
    </div>

    <!-- Presets Tabs (Stats Tabs Style) -->
    <div class="tabs">
      {#each FACTORY_PRESETS as p}
        <button
          class="tab"
          class:active={$equalizerStore.activePresetId === p.id}
          onclick={() => handleSelectPreset(p.id)}
        >
          {p.name}
        </button>
      {/each}
    </div>

    <!-- Main Card Container -->
    <div class="eq-card">
      <!-- Frequency Curve Visualizer -->
      <div class="curve-box">
        <svg viewBox="0 0 450 44" preserveAspectRatio="none">
          <defs>
            <linearGradient id="eqCurveGrad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="var(--track-hover)" stop-opacity="0.35" />
              <stop offset="100%" stop-color="var(--track-active)" stop-opacity="0.02" />
            </linearGradient>
          </defs>

          <line x1="0" y1="22" x2="450" y2="22" class="grid-center" />
          <line x1="0" y1="9" x2="450" y2="9" class="grid-line" />
          <line x1="0" y1="35" x2="450" y2="35" class="grid-line" />

          <path d={fillPath} fill="url(#eqCurveGrad)" />
          <path d={curvePath} class="curve-line" />

          {#each $equalizerStore.gains as gain, i}
            {@const x = (i / 9) * 426 + 12}
            {@const y = 22 - (gain / 12) * 17}
            <circle cx={x} cy={y} r="3" class="curve-pt" class:focused={focusedIdx === i + 1} />
          {/each}
        </svg>
      </div>

      <!-- Faders Grid -->
      <div class="faders-row">
        <!-- Preamp Fader -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="fader-col preamp-col"
          class:focused={focusedIdx === 0}
          onclick={() => (focusedIdx = 0)}
        >
          <span class="gain-val">{formatGain($equalizerStore.preamp)}</span>
          <div class="slider-box">
            <input
              type="range"
              min="-12"
              max="12"
              step="0.5"
              value={$equalizerStore.preamp}
              oninput={(e) => setPreampGain(parseFloat((e.target as HTMLInputElement).value), false)}
              onchange={(e) => setPreampGain(parseFloat((e.target as HTMLInputElement).value), true)}
              class="v-slider preamp-slider"
            />
          </div>
          <span class="band-title band-title--caps">{$t('preamp')}</span>
        </div>

        <div class="v-divider"></div>

        <!-- 10 Band Faders -->
        <div class="bands-row">
          {#each BAND_LABELS as label, i}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              class="fader-col"
              class:focused={focusedIdx === i + 1}
              onclick={() => (focusedIdx = i + 1)}
            >
              <span class="gain-val">{formatGain($equalizerStore.gains[i])}</span>
              <div class="slider-box">
                <input
                  type="range"
                  min="-12"
                  max="12"
                  step="0.5"
                  value={$equalizerStore.gains[i]}
                  oninput={(e) => setBandGain(i, parseFloat((e.target as HTMLInputElement).value), false)}
                  onchange={(e) => setBandGain(i, parseFloat((e.target as HTMLInputElement).value), true)}
                  class="v-slider"
                />
              </div>
              <span class="band-title">{label}</span>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <!-- Footer Bar -->
    <div class="footer">
      <button class="hint-btn" onclick={closeModal}>
        <PS2Btn type="circle" />
        <span>{$t('back')}</span>
      </button>

      <button class="clear-btn" onclick={handleReset}>
        <PS2Btn type="triangle" />
        <span>{$t('resetEq')}</span>
      </button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.78);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: fade-in 0.18s ease;
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .panel {
    width: 520px;
    max-height: 85%;
    display: flex;
    flex-direction: column;
    gap: 10px;
    animation: slide-in 0.25s cubic-bezier(0.34, 1.4, 0.64, 1);
  }

  @keyframes slide-in {
    from {
      opacity: 0;
      transform: translateY(16px) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  /* ── Summary Cards (Stats Style) ── */
  .summary {
    display: flex;
    gap: 8px;
  }

  .stat-card {
    flex: 1;
    background: var(--card-bg);
    border-radius: 8px;
    padding: 8px 6px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    box-shadow: var(--btn-shadow);
  }

  .stat-val {
    font-size: 16px;
    font-weight: 700;
    color: var(--track-active);
    white-space: nowrap;
  }

  .stat-lbl {
    font-size: 9px;
    color: var(--text-dim);
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  /* ── Tabs (Stats Tabs Style) ── */
  .tabs {
    display: flex;
    gap: 2px;
    background: rgba(10, 10, 22, 0.5);
    border-radius: 8px;
    padding: 3px;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .tabs::-webkit-scrollbar {
    display: none;
  }

  .tab {
    flex: 1;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 10px;
    font-family: inherit;
    color: var(--text-secondary);
    padding: 5px 4px;
    border-radius: 6px;
    letter-spacing: 0.02em;
    white-space: nowrap;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .tab.active {
    background: var(--card-bg);
    color: var(--track-active);
    box-shadow: var(--btn-shadow);
  }

  /* ── Main EQ Card Container ── */
  .eq-card {
    background: var(--card-bg);
    border-radius: 8px;
    padding: 12px;
    box-shadow: var(--btn-shadow);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* ── Curve Box ── */
  .curve-box {
    height: 44px;
    background: rgba(10, 10, 22, 0.35);
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .curve-box svg {
    width: 100%;
    height: 100%;
  }

  .grid-center {
    stroke: rgba(255, 255, 255, 0.2);
    stroke-dasharray: 4;
  }

  .grid-line {
    stroke: rgba(255, 255, 255, 0.06);
  }

  .curve-line {
    fill: none;
    stroke: var(--track-hover);
    stroke-width: 2;
  }

  .curve-pt {
    fill: var(--track-active);
    stroke: #ffffff;
    stroke-width: 1;
  }

  .curve-pt.focused {
    r: 5;
    fill: var(--track-hover);
    stroke: var(--track-active);
    stroke-width: 2;
  }

  /* ── Faders Grid ── */
  .faders-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .fader-col {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    flex: 1;
    padding: 4px 0;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.12s;
  }

  .fader-col.focused {
    background: rgba(255, 255, 255, 0.12);
    outline: 1px solid var(--track-active);
  }

  .preamp-col {
    flex: 0 0 46px;
  }

  .v-divider {
    width: 1px;
    height: 120px;
    background: rgba(255, 255, 255, 0.15);
  }

  .bands-row {
    display: flex;
    flex: 1;
    justify-content: space-between;
  }

  .gain-val {
    font-size: 10px;
    font-weight: 700;
    color: var(--track-active);
    min-height: 14px;
  }

  .slider-box {
    height: 90px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .v-slider {
    writing-mode: bt-lr;
    appearance: slider-vertical;
    width: 18px;
    height: 84px;
    background: transparent;
    cursor: pointer;
    accent-color: var(--track-hover);
  }

  .preamp-slider {
    accent-color: var(--track-active);
  }

  .band-title {
    font-size: 9px;
    color: var(--text-dim);
    letter-spacing: 0.02em;
  }

  .band-title--caps {
    text-transform: uppercase;
  }

  /* ── Footer ── */
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 2px;
  }

  .hint-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 12px;
    color: var(--text-secondary);
    padding: 0;
    transition: color 0.15s;
  }

  .hint-btn:hover {
    color: var(--text-primary);
  }

  .clear-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    cursor: pointer;
    font-size: 11px;
    font-family: inherit;
    color: var(--text-dim);
    padding: 4px 8px;
    transition: color 0.15s;
    letter-spacing: 0.04em;
  }

  .clear-btn:hover {
    color: var(--text-secondary);
  }
</style>
