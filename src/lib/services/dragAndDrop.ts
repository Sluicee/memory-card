import { getCurrentWebview } from "@tauri-apps/api/webview";
import { scanFolder } from "$lib/stores/library";

const AUDIO_EXTS = new Set([
  "mp3",
  "flac",
  "ogg",
  "m4a",
  "aac",
  "wav",
  "opus",
]);

/**
 * Sets up drag-and-drop file scanning on the webview window.
 * Returns an unlisten function or promise.
 */
export function setupDragAndDrop(onDragOverChange: (isOver: boolean) => void) {
  const webview = getCurrentWebview();
  return webview.onDragDropEvent((event) => {
    const type = event.payload.type;
    if (type === "enter" || type === "over") {
      onDragOverChange(true);
    } else if (type === "leave") {
      onDragOverChange(false);
    } else if (type === "drop") {
      onDragOverChange(false);
      const rawPaths = (event.payload as { paths: string[] }).paths;
      // If individual audio files are dropped, use the parent directory so the
      // whole album folder is scanned. Deduplicate in case multiple files from
      // the same folder are dropped at once.
      const resolved = [
        ...new Set(
          rawPaths.map((p) => {
            const ext = p.split(".").pop()?.toLowerCase() ?? "";
            if (AUDIO_EXTS.has(ext)) {
              const lastSep = Math.max(
                p.lastIndexOf("/"),
                p.lastIndexOf("\\"),
              );
              return lastSep >= 0 ? p.slice(0, lastSep) : p;
            }
            return p;
          }),
        ),
      ];
      (async () => {
        for (const path of resolved) {
          await scanFolder(path);
        }
      })();
    }
  });
}
