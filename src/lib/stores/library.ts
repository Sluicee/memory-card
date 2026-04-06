import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Album } from '../types';
import { stop } from './player';

const FOLDERS_KEY = 'mp_folders';
const SIZE_KEY    = 'mp_lib_size';

function loadFolders(): string[] {
  try { return JSON.parse(localStorage.getItem(FOLDERS_KEY) ?? '[]'); } catch { return []; }
}
function saveFolders(paths: string[]) {
  localStorage.setItem(FOLDERS_KEY, JSON.stringify(paths));
}

export const folderPaths   = writable<string[]>(loadFolders());
export const albums        = writable<Album[]>([]);
export const isScanning    = writable(false);
export const librarySize   = writable(localStorage.getItem(SIZE_KEY) ?? '');
export const selectedAlbum = writable<Album | null>(null);
export const scanStatus    = writable({ filesScanned: 0, albumsFound: 0 });

export const albumCount = derived(albums, ($a) => $a.length);

// Persistent listener: cover art found online after initial scan
listen<{ id: string; cover_art: string }>('cover:update', (e) => {
  albums.update((a) =>
    a.map((album) => album.id === e.payload.id ? { ...album, cover_art: e.payload.cover_art } : album)
  );
});

// ── Cache persistence ─────────────────────────────────────────────────────────

async function saveCache() {
  const data = JSON.stringify(get(albums));
  try { await invoke('save_library_cache', { data }); } catch (e) { console.error(e); }
}

export async function loadCache(): Promise<boolean> {
  return new Promise(async (resolve) => {
    const unlisten: Array<() => void> = [];

    unlisten.push(await listen<Album>('scan:album', (e) => {
      albums.update((a) => [...a, e.payload]);
    }));

    unlisten.push(await listen<void>('scan:done', () => {
      unlisten.forEach(u => u());
      resolve(true);
    }));

    try {
      const found = await invoke<boolean>('load_library_cache');
      if (!found) {
        unlisten.forEach(u => u());
        resolve(false);
      }
    } catch (e) {
      console.error('Cache load failed:', e);
      unlisten.forEach(u => u());
      resolve(false);
    }
  });
}

// ── Scan helpers ──────────────────────────────────────────────────────────────

async function scanOne(path: string): Promise<void> {
  const unlisten: Array<() => void> = [];

  await new Promise<void>(async (resolve) => {
    unlisten.push(await listen<{ files_scanned: number; albums_found: number }>(
      'scan:progress',
      (e) => scanStatus.set({ filesScanned: e.payload.files_scanned, albumsFound: e.payload.albums_found })
    ));
    unlisten.push(await listen<Album>('scan:album', (e) => {
      albums.update((a) => [...a, e.payload]);
    }));
    unlisten.push(await listen<void>('scan:done', () => {
      unlisten.forEach(u => u());
      resolve();
    }));

    try {
      const size = await invoke<string>('scan_music_folder', { path });
      librarySize.set(size);
      localStorage.setItem(SIZE_KEY, size);
    } catch (e) {
      console.error('Scan failed:', e);
      unlisten.forEach(u => u());
      resolve();
    }
  });
}

// ── Public API ────────────────────────────────────────────────────────────────

export async function scanFolder(path: string) {
  const paths = get(folderPaths);
  if (!paths.includes(path)) {
    const next = [...paths, path];
    folderPaths.set(next);
    saveFolders(next);
  }

  isScanning.set(true);
  scanStatus.set({ filesScanned: 0, albumsFound: 0 });

  await scanOne(path);

  isScanning.set(false);
  await saveCache();
}

export async function refreshLibrary() {
  const paths = get(folderPaths);
  if (!paths.length) return;

  isScanning.set(true);
  albums.set([]);
  scanStatus.set({ filesScanned: 0, albumsFound: 0 });

  for (const path of paths) {
    await scanOne(path);
  }

  isScanning.set(false);
  await saveCache();
}

export function clearLibrary() {
  stop();
  albums.set([]);
  librarySize.set('');
  localStorage.removeItem(SIZE_KEY);
  folderPaths.set([]);
  saveFolders([]);
  scanStatus.set({ filesScanned: 0, albumsFound: 0 });
  invoke('save_library_cache', { data: '[]' }).catch(() => {});
}
