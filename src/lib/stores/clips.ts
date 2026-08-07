import { writable, get } from 'svelte/store';
import { invoke, Channel } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Clip } from '../types';
import { stop as stopMusic } from './player';

const CLIP_FOLDERS_KEY = 'mp_clip_folders';

// Longer-side cap in pixels. The backend's ffmpeg pipe (clip_video.rs)
// fits+pads whatever box it's given, so compute a per-clip box that matches
// the clip's own aspect ratio (from the scanner-probed width/height),
// capped at this longer side.
//
// 640 is a deliberate, diagnosed choice, not an arbitrary bandwidth-safe
// number. On this dev machine's WebKitGTK+NVIDIA combination, canvas
// compositing throughput craters somewhere between ~230K px (640x360,
// consistently smooth ~24fps across every isolated test) and ~507K px
// (950x534, consistently throttled to ~11fps) — independent of *how* the
// canvas reaches that pixel count (larger backing texture and CSS-scaling
// a small texture up both triggered it equally in controlled tests). This
// is a platform/driver-level compositor limit, not something fixable from
// app code — see the memory/plan notes for the full diagnostic trail.
// Revisit this bound if a fix surfaces upstream, or per-platform if this
// turns out to be Linux/WebKitGTK-specific (Windows uses WebView2, a
// completely different engine, and should be tested separately).
const PLAYBACK_BOUND = 640;

/** Aspect-fits a clip's source resolution into a PLAYBACK_BOUND-capped box, rounded to even pixels (ffmpeg-friendly). */
export function computePlaybackBox(clip: Clip): { w: number; h: number } {
  const srcW = clip.width || 16;
  const srcH = clip.height || 9;
  const scale = PLAYBACK_BOUND / Math.max(srcW, srcH);
  const w = Math.max(2, Math.round((srcW * scale) / 2) * 2);
  const h = Math.max(2, Math.round((srcH * scale) / 2) * 2);
  return { w, h };
}

function loadClipFolders(): string[] {
  try { return JSON.parse(localStorage.getItem(CLIP_FOLDERS_KEY) ?? '[]'); } catch { return []; }
}
function saveClipFolders(paths: string[]) {
  localStorage.setItem(CLIP_FOLDERS_KEY, JSON.stringify(paths));
}

export const clipFolderPaths = writable<string[]>(loadClipFolders());
export const clips           = writable<Clip[]>([]);
export const isClipScanning  = writable(false);
export const clipScanStatus  = writable({ filesScanned: 0, clipsFound: 0, totalFiles: 0 });
export const selectedClip    = writable<Clip | null>(null);

// ── Cache persistence ─────────────────────────────────────────────────────────

async function saveCache() {
  const data = JSON.stringify(get(clips));
  try { await invoke('save_clip_cache', { data }); } catch (e) { console.error(e); }
}

export async function loadClipCache(): Promise<boolean> {
  return new Promise(async (resolve) => {
    const unlisten: Array<() => void> = [];

    unlisten.push(await listen<Clip[]>('clip:scan:clips', (e) => {
      clips.update((c) => [...c, ...e.payload]);
    }));

    unlisten.push(await listen<void>('clip:scan:done', () => {
      unlisten.forEach(u => u());
      resolve(true);
    }));

    try {
      const found = await invoke<boolean>('load_clip_cache');
      if (!found) {
        unlisten.forEach(u => u());
        resolve(false);
      }
    } catch (e) {
      console.error('Clip cache load failed:', e);
      unlisten.forEach(u => u());
      resolve(false);
    }
  });
}

// ── Scan helpers ──────────────────────────────────────────────────────────────

async function scanOne(path: string): Promise<void> {
  const unlisten: Array<() => void> = [];
  let clipsFound = 0;

  await new Promise<void>(async (resolve) => {
    unlisten.push(await listen<{ total: number }>('clip:scan:start', (e) => {
      clipScanStatus.update(s => ({ ...s, totalFiles: e.payload.total, filesScanned: 0, clipsFound: 0 }));
      clipsFound = 0;
    }));
    unlisten.push(await listen<{ files_scanned: number }>(
      'clip:scan:progress',
      (e) => clipScanStatus.update(s => ({ ...s, filesScanned: e.payload.files_scanned }))
    ));
    unlisten.push(await listen<Clip[]>('clip:scan:clips', (e) => {
      clipsFound += e.payload.length;
      clipScanStatus.update(s => ({ ...s, clipsFound }));
      // Upsert by id so overlapping/re-scanned folders don't duplicate entries.
      clips.update((c) => {
        const copy = [...c];
        for (const item of e.payload) {
          const idx = copy.findIndex(x => x.id === item.id);
          if (idx >= 0) copy[idx] = item;
          else copy.push(item);
        }
        return copy;
      });
    }));
    unlisten.push(await listen<void>('clip:scan:done', () => {
      unlisten.forEach(u => u());
      resolve();
    }));

    try {
      await invoke('scan_clip_folder', { path });
    } catch (e) {
      console.error('Clip scan failed:', e);
      unlisten.forEach(u => u());
      resolve();
    }
  });
}

// ── Public scan API ────────────────────────────────────────────────────────────

// Serialises scanClipFolder/refreshClipLibrary calls, same reasoning as
// library.ts's scanLock: avoids overlapping Tauri event listeners.
let scanLock: Promise<void> = Promise.resolve();

export async function scanClipFolder(path: string) {
  scanLock = scanLock.then(async () => {
    const paths = get(clipFolderPaths);
    const norm = (p: string) => p.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase();
    const normPath = norm(path);
    const exists = paths.some(p => norm(p) === normPath);
    if (!exists) {
      const next = [...paths, path];
      clipFolderPaths.set(next);
      saveClipFolders(next);
    }

    isClipScanning.set(true);
    clipScanStatus.set({ filesScanned: 0, clipsFound: 0, totalFiles: 0 });

    await scanOne(path);

    isClipScanning.set(false);
    await saveCache();
  }).catch((e) => { console.error('scanClipFolder error:', e); });
  return scanLock;
}

export async function refreshClipLibrary() {
  scanLock = scanLock.then(async () => {
    const paths = get(clipFolderPaths);
    if (!paths.length) return;

    isClipScanning.set(true);
    clips.set([]);
    clipScanStatus.set({ filesScanned: 0, clipsFound: 0, totalFiles: 0 });

    for (const path of paths) {
      await scanOne(path);
    }

    isClipScanning.set(false);
    await saveCache();
  }).catch((e) => { console.error('refreshClipLibrary error:', e); });
  return scanLock;
}

export function clearClipLibrary() {
  clips.set([]);
  clipFolderPaths.set([]);
  saveClipFolders([]);
  clipScanStatus.set({ filesScanned: 0, clipsFound: 0, totalFiles: 0 });
  invoke('save_clip_cache', { data: '[]' }).catch(() => {});
  invoke('clear_clip_thumbs').catch(() => {});
}

// ── Playback (paired audio + video transport) ─────────────────────────────────
//
// Wraps the audio (existing audio_* commands, reused as-is) and video
// (clip_video_* commands) invokes together so callers never drive the two
// transports separately and risk them drifting out of sync.

export const clipPosition = writable(0);
export const clipDuration = writable(0);
export const clipFinished = writable(false);

let resyncTimer: ReturnType<typeof setInterval> | null = null;

function startResyncLoop() {
  stopResyncLoop();
  resyncTimer = setInterval(async () => {
    try {
      const pos = await invoke<number>('audio_get_position');
      clipPosition.set(pos);
      await invoke('clip_video_resync', { audioPos: pos });
      if (await invoke<boolean>('audio_is_finished')) {
        clipFinished.set(true);
      }
    } catch {
      // audio may have stopped on its own — treat as finished
      clipFinished.set(true);
    }
  }, 500);
}

function stopResyncLoop() {
  if (resyncTimer) {
    clearInterval(resyncTimer);
    resyncTimer = null;
  }
}

export async function playClip(clip: Clip, channel: InstanceType<typeof Channel<ArrayBuffer>>): Promise<void> {
  // Opening a clip stops whatever music was playing — a track can't sanely
  // play under the clip's own audio track — and does NOT auto-resume it on
  // close (surprising). See stopClip below.
  await stopMusic();

  clipPosition.set(0);
  clipDuration.set(clip.duration || 0);
  clipFinished.set(false);

  const { w, h } = computePlaybackBox(clip);

  await invoke('audio_play', { path: clip.path, duration: clip.duration || 36000 });
  await invoke('clip_video_start', {
    path: clip.path,
    seekSecs: 0,
    maxW: w,
    maxH: h,
    channel,
  });
  startResyncLoop();
}

export async function pauseClip(): Promise<void> {
  await invoke('audio_pause');
  await invoke('clip_video_pause');
}

export async function resumeClip(): Promise<void> {
  await invoke('audio_resume');
  await invoke('clip_video_resume');
}

export async function seekClip(secs: number): Promise<void> {
  clipPosition.set(secs); // optimistic — resync loop only polls every 500ms
  await invoke('audio_seek', { position: secs });
  await invoke('clip_video_seek', { seekSecs: secs });
}

export async function stopClip(): Promise<void> {
  stopResyncLoop();
  await invoke('clip_video_stop');
  await invoke('audio_stop');
  selectedClip.set(null);
  clipPosition.set(0);
  clipDuration.set(0);
  clipFinished.set(false);
}
