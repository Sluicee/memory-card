import { browser } from '$app/environment';
import { writable, type Readable } from 'svelte/store';

export type GamepadAction =
  | 'cross' | 'circle' | 'square' | 'triangle'
  | 'l1' | 'r1' | 'l2' | 'r2'
  | 'l3' | 'r3'
  | 'select' | 'start'
  | 'up' | 'down' | 'left' | 'right';

// Standard Gamepad API button indices
// Works identically for PS (DualShock/DualSense) and Xbox controllers
const BUTTON_MAP: Partial<Record<number, GamepadAction>> = {
  0:  'cross',     // PS: Cross    / Xbox: A
  1:  'circle',    // PS: Circle   / Xbox: B
  2:  'square',    // PS: Square   / Xbox: X
  3:  'triangle',  // PS: Triangle / Xbox: Y
  4:  'l1',        // PS: L1       / Xbox: LB
  5:  'r1',        // PS: R1       / Xbox: RB
  6:  'l2',        // PS: L2       / Xbox: LT
  7:  'r2',        // PS: R2       / Xbox: RT
  8:  'select',    // PS: Share    / Xbox: View
  9:  'start',     // PS: Options  / Xbox: Menu
  10: 'l3',        // PS: L3       / Xbox: LS (left stick click)
  11: 'r3',        // PS: R3       / Xbox: RS (right stick click)
  12: 'up',        // D-pad Up
  13: 'down',      // D-pad Down
  14: 'left',      // D-pad Left
  15: 'right',     // D-pad Right
};

// D-pad buttons that support hold-to-repeat
const DPAD_ACTIONS = new Set<GamepadAction>(['up', 'down', 'left', 'right']);
const HOLD_DELAY_MS  = 350;  // ms before first repeat fires
const HOLD_REPEAT_MS = 130;  // ms between subsequent repeats

type Listener = (action: GamepadAction) => void;
const listeners = new Set<Listener>();

const _connected = writable(false);
export const gamepadConnected: Readable<boolean> = { subscribe: _connected.subscribe };

let rafId = 0;
let prevPressed: boolean[] = [];
const holdStart  = new Map<GamepadAction, number>();
const lastRepeat = new Map<GamepadAction, number>();

function fire(action: GamepadAction) {
  for (const fn of listeners) fn(action);
}

function pollFrame(now: DOMHighResTimeStamp) {
  if (!browser || !document.hasFocus()) {
    prevPressed = [];
    holdStart.clear();
    lastRepeat.clear();
    rafId = requestAnimationFrame(pollFrame);
    return;
  }

  const pads = navigator.getGamepads();
  const pad  = pads.find((p) => p !== null);

  if (pad) {
    for (let i = 0; i < pad.buttons.length; i++) {
      const action = BUTTON_MAP[i];
      if (!action) continue;

      const pressed = pad.buttons[i].pressed;
      const was     = prevPressed[i] ?? false;

      if (pressed && !was) {
        // Rising edge — fire immediately
        fire(action);
        if (DPAD_ACTIONS.has(action)) {
          holdStart.set(action, now);
          lastRepeat.delete(action);
        }
      } else if (!pressed && was) {
        // Falling edge — clear hold state
        holdStart.delete(action);
        lastRepeat.delete(action);
      } else if (pressed && was && DPAD_ACTIONS.has(action)) {
        // Held — repeat after delay
        const start = holdStart.get(action) ?? now;
        const last  = lastRepeat.get(action);
        if (last === undefined && now - start >= HOLD_DELAY_MS) {
          fire(action);
          lastRepeat.set(action, now);
        } else if (last !== undefined && now - last >= HOLD_REPEAT_MS) {
          fire(action);
          lastRepeat.set(action, now);
        }
      }

      prevPressed[i] = pressed;
    }
  }

  rafId = requestAnimationFrame(pollFrame);
}

function startPolling() {
  if (rafId) return;
  prevPressed = [];
  rafId = requestAnimationFrame(pollFrame);
}

function stopPolling() {
  cancelAnimationFrame(rafId);
  rafId = 0;
  prevPressed = [];
  holdStart.clear();
  lastRepeat.clear();
}

export function addGamepadListener(fn: Listener): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function initGamepad(): void {
  if (!browser) return;

  window.addEventListener('gamepadconnected', () => {
    _connected.set(true);
    startPolling();
  });

  window.addEventListener('gamepaddisconnected', () => {
    const anyLeft = Array.from(navigator.getGamepads()).some((p) => p !== null);
    if (!anyLeft) {
      _connected.set(false);
      stopPolling();
    }
  });

  // Handle already-connected gamepad (e.g. page reload while plugged in)
  const already = Array.from(navigator.getGamepads()).find((p) => p !== null);
  if (already) {
    _connected.set(true);
    startPolling();
  }
}
