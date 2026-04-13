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
export const scanStatus    = writable({ filesScanned: 0, albumsFound: 0, totalFiles: 0 });

export const albumCount = derived(albums, ($a) => $a.length);

// Persistent listener: cover art found online after initial scan
listen<{ id: string; cover_art: string }>('cover:update', (e) => {
  albums.update((a) =>
    a.map((album) => album.id === e.payload.id ? { ...album, cover_art: e.payload.cover_art } : album)
  );
});

// Save cache once all internet covers have been fetched
listen<void>('cover:fetch:done', () => { saveCache(); });

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
  let albumsFound = 0;

  await new Promise<void>(async (resolve) => {
    unlisten.push(await listen<{ total: number }>('scan:start', (e) => {
      scanStatus.update(s => ({ ...s, totalFiles: e.payload.total, filesScanned: 0, albumsFound: 0 }));
      albumsFound = 0;
    }));
    unlisten.push(await listen<{ files_scanned: number }>(
      'scan:progress',
      (e) => scanStatus.update(s => ({ ...s, filesScanned: e.payload.files_scanned }))
    ));
    unlisten.push(await listen<Album>('scan:album', (e) => {
      albumsFound++;
      scanStatus.update(s => ({ ...s, albumsFound }));
      albums.update((a) => [...a, e.payload]);
    }));
    unlisten.push(await listen<void>('scan:done', () => {
      unlisten.forEach(u => u());
      resolve();
    }));

    try {
      await invoke('scan_music_folder', { path });
    } catch (e) {
      console.error('Scan failed:', e);
      unlisten.forEach(u => u());
      resolve();
    }
  });
}

// Calculates total library size across all known folders and persists it.
async function updateTotalLibrarySize() {
  const paths = get(folderPaths);
  if (!paths.length) return;
  try {
    const sizeStrings = await Promise.all(
      paths.map(p => invoke<string>('get_library_size', { path: p }))
    );
    let totalBytes = 0;
    for (const s of sizeStrings) {
      const m = s.match(/([\d.]+)\s*(GB|MB)/);
      if (m) {
        const val = parseFloat(m[1]);
        totalBytes += m[2] === 'GB'
          ? Math.round(val * 1_073_741_824)
          : Math.round(val * 1_048_576);
      }
    }
    const GB = 1_073_741_824;
    const MB = 1_048_576;
    const size = totalBytes >= GB
      ? `${(totalBytes / GB).toFixed(3)} GB`
      : `${Math.round(totalBytes / MB)} MB`;
    librarySize.set(size);
    localStorage.setItem(SIZE_KEY, size);
  } catch (e) {
    console.error('Failed to calculate total library size:', e);
  }
}

// ── Public API ────────────────────────────────────────────────────────────────

// Ensures only one scan runs at a time. All scanFolder / refreshLibrary calls
// are serialised through this chain, preventing Tauri event listener conflicts.
let scanLock: Promise<void> = Promise.resolve();

export async function scanFolder(path: string) {
  scanLock = scanLock.then(async () => {
    const paths = get(folderPaths);
    // Skip if already in the library — use "Refresh library" to rescan.
    if (paths.includes(path)) return;

    const next = [...paths, path];
    folderPaths.set(next);
    saveFolders(next);

    isScanning.set(true);
    scanStatus.set({ filesScanned: 0, albumsFound: 0, totalFiles: 0 });

    await scanOne(path);

    isScanning.set(false);
    await updateTotalLibrarySize();
    await saveCache();
  }).catch((e) => { console.error('scanFolder error:', e); });
  return scanLock;
}

export async function refreshLibrary() {
  scanLock = scanLock.then(async () => {
    const paths = get(folderPaths);
    if (!paths.length) return;

    isScanning.set(true);
    albums.set([]);
    scanStatus.set({ filesScanned: 0, albumsFound: 0, totalFiles: 0 });

    for (const path of paths) {
      await scanOne(path);
    }

    isScanning.set(false);
    await updateTotalLibrarySize();
    await saveCache();
  }).catch((e) => { console.error('refreshLibrary error:', e); });
  return scanLock;
}

export function clearLibrary() {
  stop();
  albums.set([]);
  librarySize.set('');
  localStorage.removeItem(SIZE_KEY);
  folderPaths.set([]);
  saveFolders([]);
  scanStatus.set({ filesScanned: 0, albumsFound: 0, totalFiles: 0 });
  invoke('save_library_cache', { data: '[]' }).catch(() => {});
}
