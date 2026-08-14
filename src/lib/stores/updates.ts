import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { openUrl } from '@tauri-apps/plugin-opener';

export interface UpdateInfo {
  version: string;
  url: string;
  is_available: boolean;
  /** false for pacman-managed (AUR) and unpackaged builds — those only get the release page. */
  can_auto_update: boolean;
}

export type UpdateStage =
  | 'idle'
  | 'preparing'   // asking the update server for the manifest
  | 'downloading'
  | 'installing'
  | 'restarting'
  | 'error';

export interface UpdateProgress {
  stage: UpdateStage;
  /** 0..1, or null while the total size is still unknown. */
  ratio: number | null;
}

export const updateInfo = writable<UpdateInfo | null>(null);
export const updateProgress = writable<UpdateProgress>({ stage: 'idle', ratio: null });

export async function checkForUpdates() {
  try {
    const info = await invoke<UpdateInfo>('check_for_updates');
    if (info.is_available) {
      updateInfo.set(info);
      console.log('Update available:', info.version);
    } else {
      updateInfo.set(null);
    }
  } catch (e) {
    console.error('Failed to check for updates:', e);
    updateInfo.set(null);
  }
}

/**
 * Clears a previous failure so the entry goes back to "Get Update (x.y.z)".
 * Call it when the options menu opens: the error label is only meaningful right
 * after the attempt that produced it.
 */
export function clearUpdateError() {
  updateProgress.update((p) => (p.stage === 'error' ? { stage: 'idle', ratio: null } : p));
}

function openReleasePage() {
  const info = get(updateInfo);
  if (info) openUrl(info.url);
}

/**
 * Download and install the update in place, then relaunch.
 *
 * Falls back to opening the GitHub release page whenever the in-place path
 * isn't available: packaging the plugin can't install over (AUR), or a release
 * that carries no `latest.json` — which is the case for every release made
 * before the updater shipped, since those builds were never signed.
 *
 * Resolves to 'restarting' once the install is done and the relaunch is under
 * way, 'fallback' when we handed off to the browser instead, 'error' when the
 * download or install itself failed.
 */
export async function installUpdate(): Promise<'restarting' | 'fallback' | 'error'> {
  const info = get(updateInfo);
  if (!info) return 'fallback';

  if (!info.can_auto_update) {
    openReleasePage();
    return 'fallback';
  }

  updateProgress.set({ stage: 'preparing', ratio: null });

  let update;
  try {
    update = await check();
  } catch (e) {
    console.error('Updater manifest check failed:', e);
    update = null;
  }

  if (!update) {
    updateProgress.set({ stage: 'idle', ratio: null });
    openReleasePage();
    return 'fallback';
  }

  try {
    let downloaded = 0;
    let total = 0;
    // Progress events arrive per chunk; repaint at most ~20×/sec so the
    // 1-second playback poll and the Three.js cover keep their frames.
    let lastPaint = 0;

    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case 'Started':
          total = event.data.contentLength ?? 0;
          updateProgress.set({ stage: 'downloading', ratio: total ? 0 : null });
          break;
        case 'Progress': {
          downloaded += event.data.chunkLength;
          const now = Date.now();
          if (total && now - lastPaint >= 50) {
            lastPaint = now;
            updateProgress.set({
              stage: 'downloading',
              ratio: Math.min(downloaded / total, 1),
            });
          }
          break;
        }
        case 'Finished':
          updateProgress.set({ stage: 'installing', ratio: 1 });
          break;
      }
    });

    updateProgress.set({ stage: 'restarting', ratio: 1 });
    await relaunch();
    return 'restarting';
  } catch (e) {
    console.error('Failed to install update:', e);
    updateProgress.set({ stage: 'error', ratio: null });
    // Frees the Update resource the check() above put in the webview's resource
    // table; without this a retry loop leaks one per attempt.
    await update.close().catch(() => {});
    return 'error';
  }
}
