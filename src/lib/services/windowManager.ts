import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
import { LogicalSize, LogicalPosition } from "@tauri-apps/api/dpi";
import type { ViewMode } from "$lib/stores/viewMode";

/**
 * Resizes and repositions the Tauri desktop window according to the current ViewMode.
 */
export async function updateWindowForViewMode(mode: ViewMode): Promise<void> {
  const win = getCurrentWindow();
  switch (mode) {
    case "normal":
    case "focus": {
      await win.setFullscreen(false);
      await win.setAlwaysOnTop(false);
      const targetW = 950;
      const targetH = 900;
      // On Windows, non-resizable windows sometimes ignore setSize when Enlarging.
      // Temporarily enable resizing to guarantee it applies.
      await win.setResizable(true);
      await win.setSize(new LogicalSize(targetW, targetH));
      await win.setResizable(false);
      
      // Clamp position so window stays fully on-screen
      try {
        const monitor = await currentMonitor();
        if (monitor) {
          const sf = monitor.scaleFactor;
          // monitor.position and size are in physical pixels
          const monX = monitor.position.x / sf;
          const monY = monitor.position.y / sf;
          const monW = monitor.size.width / sf;
          const monH = monitor.size.height / sf;
          const pos = await win.outerPosition();
          const curX = pos.x / sf;
          const curY = pos.y / sf;
          const clampedX = Math.max(
            monX,
            Math.min(curX, monX + monW - targetW),
          );
          const clampedY = Math.max(
            monY,
            Math.min(curY, monY + monH - targetH),
          );
          if (clampedX !== curX || clampedY !== curY) {
            await win.setPosition(new LogicalPosition(clampedX, clampedY));
          }
        }
      } catch {
        /* ignore position errors */
      }
      break;
    }
    case "fullscreen":
      await win.setAlwaysOnTop(false);
      await win.setResizable(true);
      await win.setFullscreen(true);
      break;
    case "mini":
      await win.setFullscreen(false);
      await win.setAlwaysOnTop(false);
      await win.setResizable(true);
      await win.setSize(new LogicalSize(400, 160));
      await win.setResizable(false);
      break;
  }
}
