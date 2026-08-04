import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { EqualizerSettings, EqualizerPreset } from '$lib/types';

export const BAND_FREQS = [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
export const BAND_LABELS = ['31Hz', '63Hz', '125Hz', '250Hz', '500Hz', '1kHz', '2kHz', '4kHz', '8kHz', '16kHz'];

export const FACTORY_PRESETS: EqualizerPreset[] = [
  { id: 'flat', name: 'Flat', preamp: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  { id: 'bass', name: 'Bass Boost', preamp: -2, gains: [6, 5, 4, 2, 0, 0, 0, 0, 0, 0] },
  { id: 'treble', name: 'Treble Boost', preamp: -2, gains: [0, 0, 0, 0, 0, 1, 3, 5, 6, 7] },
  { id: 'rock', name: 'Rock', preamp: -2, gains: [4, 3, 1, 0, -1, 0, 1, 2, 3, 4] },
  { id: 'pop', name: 'Pop', preamp: -1, gains: [-1, 1, 3, 4, 3, 0, -1, -1, 1, 2] },
  { id: 'classical', name: 'Classical', preamp: -1, gains: [4, 3, 2, 2, -1, -1, 0, 2, 3, 4] },
  { id: 'vocal', name: 'Vocal', preamp: -1, gains: [-2, -1, 1, 3, 4, 4, 3, 1, 0, -1] },
  { id: 'electronic', name: 'Electronic', preamp: -2, gains: [5, 4, 2, 0, -2, 2, 1, 2, 4, 5] },
  { id: 'metal', name: 'Metal', preamp: -2, gains: [5, 4, 2, 0, -2, 1, 3, 4, 3, 2] },
  { id: 'custom', name: 'Custom', preamp: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
];

export interface EqualizerStoreState {
  enabled: boolean;
  preamp: number;
  gains: number[];
  customPreamp: number;
  customGains: number[];
  activePresetId: string;
}

const STORAGE_KEY = 'mp_equalizer_settings_v2';

function loadInitialState(): EqualizerStoreState {
  const defaultCustomGains = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  const defaultState: EqualizerStoreState = {
    enabled: true,
    preamp: 0,
    gains: defaultCustomGains,
    customPreamp: 0,
    customGains: defaultCustomGains,
    activePresetId: 'flat',
  };

  try {
    const json = localStorage.getItem(STORAGE_KEY);
    if (json) {
      const parsed = JSON.parse(json);
      const customGains = Array.isArray(parsed.customGains) && parsed.customGains.length === 10
        ? parsed.customGains
        : defaultCustomGains;
      const customPreamp = typeof parsed.customPreamp === 'number' ? parsed.customPreamp : 0;
      const activePresetId = parsed.activePresetId || 'flat';

      let gains = Array.isArray(parsed.gains) && parsed.gains.length === 10
        ? parsed.gains
        : defaultCustomGains;
      let preamp = typeof parsed.preamp === 'number' ? parsed.preamp : 0;

      if (activePresetId === 'custom') {
        gains = [...customGains];
        preamp = customPreamp;
      } else {
        const preset = FACTORY_PRESETS.find((p) => p.id === activePresetId);
        if (preset) {
          gains = [...preset.gains];
          preamp = preset.preamp;
        }
      }

      return {
        enabled: true,
        preamp,
        gains,
        customPreamp,
        customGains,
        activePresetId,
      };
    }
  } catch (e) {
    console.error('Failed to load equalizer settings from localStorage:', e);
  }

  return defaultState;
}

export const equalizerStore = writable<EqualizerStoreState>(loadInitialState());
export const isEqualizerOpen = writable<boolean>(false);

let syncTimeout: ReturnType<typeof setTimeout> | null = null;

export function syncBackend(state: EqualizerStoreState, immediate = false) {
  const payload: EqualizerSettings = {
    enabled: true,
    preamp: state.preamp,
    gains: state.gains,
  };

  if (syncTimeout) {
    clearTimeout(syncTimeout);
    syncTimeout = null;
  }

  const doSync = () => {
    invoke('audio_set_equalizer', { settings: payload }).catch((err) => {
      console.error('Failed to update equalizer in backend:', err);
    });
  };

  if (immediate) {
    doSync();
  } else {
    syncTimeout = setTimeout(doSync, 350);
  }
}

// Subscribe to persist state
equalizerStore.subscribe((state) => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch (e) {
    console.error('Failed to save equalizer settings:', e);
  }
});

export function setBandGain(index: number, gain: number, immediate = false) {
  equalizerStore.update((curr) => {
    const newGains = [...curr.gains];
    const clampedGain = Math.max(-12, Math.min(12, gain));
    newGains[index] = clampedGain;

    const newCustomGains = [...curr.customGains];
    newCustomGains[index] = clampedGain;

    const next: EqualizerStoreState = {
      ...curr,
      gains: newGains,
      customGains: newCustomGains,
      customPreamp: curr.preamp,
      activePresetId: 'custom',
    };
    syncBackend(next, immediate);
    return next;
  });
}

export function setPreampGain(preamp: number, immediate = false) {
  equalizerStore.update((curr) => {
    const clampedPreamp = Math.max(-12, Math.min(12, preamp));
    const next: EqualizerStoreState = {
      ...curr,
      preamp: clampedPreamp,
      customPreamp: clampedPreamp,
      activePresetId: 'custom',
    };
    syncBackend(next, immediate);
    return next;
  });
}

export function applyPreset(presetId: string) {
  equalizerStore.update((curr) => {
    if (presetId === 'custom') {
      const next: EqualizerStoreState = {
        ...curr,
        preamp: curr.customPreamp,
        gains: [...curr.customGains],
        activePresetId: 'custom',
      };
      syncBackend(next, true);
      return next;
    }

    const preset = FACTORY_PRESETS.find((p) => p.id === presetId);
    if (!preset) return curr;

    const next: EqualizerStoreState = {
      ...curr,
      preamp: preset.preamp,
      gains: [...preset.gains],
      activePresetId: preset.id,
    };
    syncBackend(next, true);
    return next;
  });
}

export function resetEqualizer() {
  applyPreset('flat');
}
