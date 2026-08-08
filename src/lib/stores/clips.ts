import { writable, get } from 'svelte/store';
import { invoke, Channel } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Clip } from '../types';
import { stop as stopMusic } from './player';

const CLIP_FOLDERS_KEY = 'mp_clip_folders';

// Sanity clamp around whatever ClipPlayerView measures as the actually-
// available display area (see `measureTargetLongSide` there, driven by a
// ResizeObserver on .stage — covers windowed, fullscreen, and mini modes
// uniformly by measuring instead of guessing per view mode). Floor: never
// decode below what even the small windowed view already needs, regardless
// of a transient 0 mid-layout. Ceiling: never chase decode cost for a
// panel bigger than this raw-frame-over-IPC pipeline can actually sustain
// — measured live: ffmpeg + WebKitWebProcess + this app's own backend
// thread all pegged simultaneously once frames grew past ~1920x1440,
// despite the same source decoding at 100+ fps standalone with nowhere
// near that output size (the bottleneck is per-frame copy/IPC bandwidth,
// not decode). Sizing to the *actual* display area (rather than the full
// monitor resolution) already avoids most of the waste that used to push
// fullscreen past this ceiling; the ceiling stays as a last-resort backstop.
export const PLAYBACK_BOUND_MIN = 640;
export const PLAYBACK_BOUND_MAX = 1920;

/**
 * Aspect-fits a clip's source resolution into a `longSide`-capped box,
 * rounded to even pixels (ffmpeg-friendly). Never scales *up* past the
 * clip's native resolution — `Math.min(1, ...)` was missing before, so any
 * clip with a longer side under the bound (a plausible size for a lot of
 * real clip sources) got upscaled by ffmpeg's own `scale=` filter to
 * exactly fill it, decoding and displaying it visibly blockier than the
 * source ever was — most obvious full-window in fullscreen mode, where
 * canvasScale is closer to 1 and doesn't shrink that blockiness back down.
 */
export function computePlaybackBox(clip: Clip, longSide: number): { w: number; h: number } {
  const srcW = clip.width || longSide;
  const srcH = clip.height || Math.round((longSide * 9) / 16);
  const scale = Math.min(1, longSide / Math.max(srcW, srcH));
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

// `generation` is caller-supplied (see ClipPlayerView.svelte) rather than
// generated in here or on the Rust side. The Rust clip-video decode thread
// is a single long-lived, app-lifetime object spawned once — an
// independent counter there would keep climbing across every clip ever
// opened in the session, while a per-ClipPlayerView-instance counter on the
// frontend resets fresh per mount; the two only ever agreed for the very
// first clip played after app start; everything after mismatched and got
// silently ignored (the generation tag never matched, so `started` never
// flipped true — the clip player looked like it had just vanished, stuck
// on its loading placeholder forever). Letting the caller own the number
// and simply echoing it back sidesteps needing either side to
// independently track a shared sequence.
export async function playClip(
  clip: Clip,
  channel: InstanceType<typeof Channel<ArrayBuffer>>,
  generation: number,
  box: { w: number; h: number },
): Promise<void> {
  // Opening a clip stops whatever music was playing — a track can't sanely
  // play under the clip's own audio track — and does NOT auto-resume it on
  // close (surprising). See stopClip below.
  await stopMusic();

  clipPosition.set(0);
  clipDuration.set(clip.duration || 0);
  clipFinished.set(false);

  const { w, h } = box;

  // Fired together, not one `await`ed before the other starting — video
  // decode/seek is inherently slower than audio's, and awaiting audio_play
  // first was adding audio's own latency on top of that gap for no reason,
  // making video's already-later start later still.
  await Promise.all([
    invoke('audio_play', { path: clip.path, duration: clip.duration || 36000 }),
    invoke('clip_video_start', {
      path: clip.path,
      seekSecs: 0,
      maxW: w,
      maxH: h,
      channel,
      generation,
    }),
  ]);
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

// `box` is re-supplied on every seek, not just Start — ClipPlayerView also
// uses a seek (at the current position) to switch decode resolution when
// entering/leaving fullscreen mid-clip, so the backend needs to be told a
// (possibly new) target size here too rather than assuming it's unchanged
// from whatever Start used.
export async function seekClip(secs: number, generation: number, box: { w: number; h: number }): Promise<void> {
  clipPosition.set(secs); // optimistic — resync loop only polls every 500ms
  // Same reasoning as playClip: fire together rather than awaiting audio's
  // seek before even starting video's, which was adding pure dead time to
  // video's already-slower restart.
  await Promise.all([
    invoke('audio_seek', { position: secs }),
    invoke('clip_video_seek', { seekSecs: secs, generation, maxW: box.w, maxH: box.h }),
  ]);
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
