import { writable, derived } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Album } from '../types';

export const folderPath = writable<string | null>(null);
export const albums = writable<Album[]>([]);
export const isScanning = writable(false);
export const librarySize = writable('');
export const selectedAlbum = writable<Album | null>(null);

export const scanStatus = writable({ filesScanned: 0, albumsFound: 0 });

export const albumCount = derived(albums, ($a) => $a.length);

export async function scanFolder(path: string) {
  isScanning.set(true);
  folderPath.set(path);
  albums.set([]);
  scanStatus.set({ filesScanned: 0, albumsFound: 0 });

  // Listen for streaming events before invoking
  const unlistenProgress = await listen<{ files_scanned: number; albums_found: number }>(
    'scan:progress',
    (e) => {
      scanStatus.set({
        filesScanned: e.payload.files_scanned,
        albumsFound: e.payload.albums_found,
      });
    }
  );

  const unlistenAlbum = await listen<Album>('scan:album', (e) => {
    albums.update((a) => [...a, e.payload]);
  });

  const unlistenDone = await listen<number>('scan:done', () => {
    isScanning.set(false);
    unlistenProgress();
    unlistenAlbum();
    unlistenDone();
  });

  try {
    const size = await invoke<string>('scan_music_folder', { path });
    librarySize.set(size);
  } catch (e) {
    console.error('Scan failed:', e);
    isScanning.set(false);
    unlistenProgress();
    unlistenAlbum();
    unlistenDone();
  }
}
