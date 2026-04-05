import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { Track, Album } from '../types';

const VOL_KEY   = 'mp_volume';
const TRACK_KEY = 'mp_last_track';

function loadVolume(): number {
  const v = parseFloat(localStorage.getItem(VOL_KEY) ?? '');
  return isNaN(v) ? 1.0 : Math.max(0, Math.min(1, v));
}

function saveVolume(v: number) {
  localStorage.setItem(VOL_KEY, String(v));
}

function saveLastTrack(track: Track | null, album: Album | null) {
  if (track && album) {
    localStorage.setItem(TRACK_KEY, JSON.stringify({ track, album }));
  } else {
    localStorage.removeItem(TRACK_KEY);
  }
}

export function loadLastTrack(): { track: Track; album: Album } | null {
  try {
    const raw = localStorage.getItem(TRACK_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch { return null; }
}

// ── Stores ────────────────────────────────────────────────────────────────────

export const currentTrack = writable<Track | null>(null);
export const currentAlbum = writable<Album | null>(null);
export const isPlaying    = writable(false);
export const isPaused     = writable(false);
export const volume       = writable(loadVolume());
export const position     = writable(0);
export const duration     = writable(0);
export const isShuffled   = writable(false);

// ── Shuffle queue (module-level, not reactive) ────────────────────────────────

type QueueItem = { track: Track; album: Album };
let _queue: QueueItem[] = [];
let _qIdx = -1;

function fisherYates<T>(arr: T[]): T[] {
  const a = [...arr];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

// ── Polling ───────────────────────────────────────────────────────────────────

let pollTimer: ReturnType<typeof setInterval> | null = null;

function startPolling() {
  if (pollTimer) return;
  pollTimer = setInterval(async () => {
    if (!get(currentTrack)) return;
    position.set(await invoke<number>('audio_get_position'));
    if (await invoke<boolean>('audio_is_finished')) {
      if (_queue.length > 0) {
        const next = _qIdx + 1;
        if (next < _queue.length) {
          _qIdx = next;
          await playTrack(_queue[_qIdx].track, _queue[_qIdx].album);
        }
      } else {
        const album = get(currentAlbum);
        if (album) await playNext(album);
      }
    }
  }, 1000);
}

function stopPolling() {
  if (pollTimer) { clearInterval(pollTimer); pollTimer = null; }
}

// ── Commands (unchanged from working version) ─────────────────────────────────

export async function playTrack(track: Track, album: Album) {
  try {
    await invoke('audio_play', { path: track.path, duration: track.duration });
    currentTrack.set(track);
    currentAlbum.set(album);
    duration.set(track.duration);
    position.set(0);
    isPlaying.set(true);
    isPaused.set(false);
    saveLastTrack(track, album);
    startPolling();
  } catch (e) {
    console.error('Play failed:', e);
  }
}

export async function pause() {
  await invoke('audio_pause');
  isPlaying.set(false);
  isPaused.set(true);
}

export async function resume() {
  await invoke('audio_resume');
  isPlaying.set(true);
  isPaused.set(false);
}

export async function stop() {
  await invoke('audio_stop');
  isPlaying.set(false);
  isPaused.set(false);
  currentTrack.set(null);
  position.set(0);
  saveLastTrack(null, null);
  _queue = []; _qIdx = -1; isShuffled.set(false);
  stopPolling();
}

export async function setVolume(v: number) {
  volume.set(v);
  saveVolume(v);
  await invoke('audio_set_volume', { volume: v });
}

export async function playNext(album: Album) {
  if (_queue.length > 0) {
    const next = _qIdx + 1;
    if (next < _queue.length) { _qIdx = next; await playTrack(_queue[_qIdx].track, _queue[_qIdx].album); }
    return;
  }
  const track = get(currentTrack);
  if (!track) return;
  const idx = album.tracks.findIndex((t) => t.id === track.id);
  const next = album.tracks[idx + 1];
  if (next) await playTrack(next, album);
}

export async function playPrev(album: Album) {
  const track = get(currentTrack);
  if (!track) return;
  if (get(position) > 3) {
    await invoke('audio_play', { path: track.path, duration: track.duration });
    position.set(0);
    return;
  }
  if (_queue.length > 0) {
    const prev = _qIdx - 1;
    if (prev >= 0) { _qIdx = prev; await playTrack(_queue[_qIdx].track, _queue[_qIdx].album); }
    return;
  }
  const idx = album.tracks.findIndex((t) => t.id === track.id);
  const prev = album.tracks[idx - 1];
  if (prev) await playTrack(prev, album);
}

// ── Shuffle ───────────────────────────────────────────────────────────────────

export async function playShuffledAll(albums: Album[]) {
  const all: QueueItem[] = albums.flatMap(a => a.tracks.map(t => ({ track: t, album: a })));
  _queue = fisherYates(all);
  _qIdx = 0;
  isShuffled.set(true);
  if (_queue[0]) await playTrack(_queue[0].track, _queue[0].album);
}

export async function playShuffled(album: Album) {
  _queue = fisherYates(album.tracks.map(t => ({ track: t, album })));
  _qIdx = 0;
  isShuffled.set(true);
  if (_queue[0]) await playTrack(_queue[0].track, _queue[0].album);
}

export async function initVolume() {
  const v = get(volume);
  await invoke('audio_set_volume', { volume: v });
}
