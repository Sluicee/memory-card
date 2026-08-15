import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
import { LogicalSize, LogicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import type { ViewMode } from "$lib/stores/viewMode";

// The compositor's own configure events are the part our awaits never see:
// they land after every call has already returned. Tracking when the last one
// arrived is what lets settleGeometry() below tell "the window is done moving"
// from "the window has not started yet".
let lastGeometryEventAt = 0;
let eventsHooked = false;
function hookWindowEvents(win: ReturnType<typeof getCurrentWindow>): void {
  if (eventsHooked) return;
  eventsHooked = true;
  const mark = () => {
    lastGeometryEventAt = Date.now();
  };
  void win.onResized(mark);
  void win.onMoved(mark);
}

const sleep = (msec: number) => new Promise((r) => setTimeout(r, msec));

/**
 * Waits for the window's geometry to stop changing.
 *
 * `minWaitMs` is not padding: a configure we are waiting for has not been
 * emitted yet at the moment we start waiting, so a pure quiet-check would
 * pass instantly and wait for nothing. Measured, the configure lands ~20ms
 * after the call that provoked it; the minimum covers that with margin, and
 * the quiet loop then covers a compositor that is slower than usual.
 */
async function settleGeometry(
  minWaitMs = 120,
  quietMs = 80,
  maxWaitMs = 600,
): Promise<void> {
  const start = Date.now();
  await sleep(minWaitMs);
  while (
    Date.now() - start < maxWaitMs &&
    Date.now() - lastGeometryEventAt < quietMs
  ) {
    await sleep(30);
  }
}

/**
 * Resizes and repositions the Tauri desktop window according to the current ViewMode.
 */
export async function updateWindowForViewMode(mode: ViewMode): Promise<void> {
  const win = getCurrentWindow();
  hookWindowEvents(win);
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
    case "fullscreen": {
      await win.setAlwaysOnTop(false);
      await win.setResizable(true);
      // setResizable makes GTK re-assert its *requested* inner geometry — the
      // configured 950x900 — and that re-assert races everything after it.
      //
      // Waiting here is what keeps the race from being lost. When the main
      // thread happens to be idle, all the calls in this branch collapse into
      // a single GTK frame (measured: 6ms for the whole branch, versus
      // 130-480ms when it is busy), the re-assert is processed *after* the
      // setSize below, and the window ends up fullscreen-flagged but 950x948
      // at the monitor origin — the "jumps to the top-left corner at the old
      // size" failure. Letting the re-assert land first makes our size the
      // last word instead.
      await settleGeometry();
      // Belt and braces for a re-assert that still arrives late: requesting
      // the monitor's own size makes such a configure harmless, since it then
      // carries the size fullscreen wants anyway. (An outer 2010x1170 in the
      // geometry log is this inner size plus the decorations, and is what a
      // successful fullscreen passes through on its own.)
      const monitor = await currentMonitor();
      if (monitor) {
        await win.setSize(
          new PhysicalSize(monitor.size.width, monitor.size.height),
        );
      }
      await win.setFullscreen(true);
      break;
    }
    case "mini":
      await win.setFullscreen(false);
      await win.setAlwaysOnTop(false);
      await win.setResizable(true);
      await win.setSize(new LogicalSize(400, 160));
      await win.setResizable(false);
      break;
  }
}
