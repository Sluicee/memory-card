import { browser } from '$app/environment';
import { writable, get } from 'svelte/store';

const SOURCES = {
  confirm: '/sfx/confirm.wav',
  back: '/sfx/back.wav',
  open: '/sfx/open.wav',
  nextPrev: '/sfx/next_prev.wav',
  steps: '/sfx/steps.wav',
  scan: '/sfx/scan.wav',
} as const;

const DEFAULT_VOLUMES: Record<UiSfxName, number> = {
  confirm: 0.225,
  back: 0.1,
  open: 0.11,
  nextPrev: 0.10,
  steps: 0.14,
  scan: 0.01,
};

export type UiSfxName = keyof typeof SOURCES;

// Persistence logic
const STORAGE_KEY = 'mc_sfx_enabled';

function getInitialState() {
  if (browser) {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored !== null) {
      return stored === 'true';
    }
  }
  return true;
}

export const sfxEnabled = writable(getInitialState());

if (browser) {
  sfxEnabled.subscribe(value => {
    localStorage.setItem(STORAGE_KEY, value.toString());
  });
}

const templates = new Map<UiSfxName, string>();

async function loadSfxBlob(name: UiSfxName): Promise<string> {
  const cached = templates.get(name);
  if (cached) return cached;

  try {
    const res = await fetch(SOURCES[name]);
    const blob = await res.blob();
    const blobUrl = URL.createObjectURL(blob);
    templates.set(name, blobUrl);
    return blobUrl;
  } catch (e) {
    console.error(`Failed to load SFX ${name}:`, e);
    return SOURCES[name];
  }
}

export function primeUiSfx() {
  if (!browser) return;
  for (const name of Object.keys(SOURCES) as UiSfxName[]) {
    void loadSfxBlob(name);
  }
}

export async function playUiSfx(name: UiSfxName, volume = DEFAULT_VOLUMES[name]) {
  if (!get(sfxEnabled)) return;
  if (typeof Audio === 'undefined') return;

  let blobUrl = templates.get(name);
  if (!blobUrl) {
    blobUrl = await loadSfxBlob(name);
  }

  const instance = new Audio(blobUrl);
  instance.volume = volume;
  instance.currentTime = 0;
  void instance.play().catch((e) => {
    console.error('Play sfx error:', e);
  });
}
