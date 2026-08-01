import { browser } from '$app/environment';
import { writable, type Readable } from 'svelte/store';

export type GamepadAction =
  | 'cross' | 'circle' | 'square' | 'triangle'
  | 'l1' | 'r1' | 'l2' | 'r2'
  | 'l3' | 'r3'
  | 'select' | 'start'
  | 'up' | 'down' | 'left' | 'right';

// Non-directional button mappings (0..11)
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
};

// Actions supporting hold-to-repeat
const DPAD_ACTIONS = new Set<GamepadAction>(['up', 'down', 'left', 'right']);
const HOLD_DELAY_MS  = 380;  // ms before first repeat fires
const HOLD_REPEAT_MS = 160;  // ms between subsequent repeats
const AXIS_THRESHOLD = 0.55; // deadzone for analog stick / axes

type Listener = (action: GamepadAction) => void;
const listeners = new Set<Listener>();

const _connected = writable(false);
export const gamepadConnected: Readable<boolean> = { subscribe: _connected.subscribe };

let rafId = 0;
let prevPressed: Record<string, boolean> = {};
const holdStart  = new Map<GamepadAction, number>();
const lastRepeat = new Map<GamepadAction, number>();

function fire(action: GamepadAction) {
  for (const fn of listeners) fn(action);
}

function processActionState(action: GamepadAction, pressed: boolean, now: DOMHighResTimeStamp) {
  const was = prevPressed[action] ?? false;

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

  prevPressed[action] = pressed;
}

function pollFrame(now: DOMHighResTimeStamp) {
  if (!browser || !document.hasFocus()) {
    prevPressed = {};
    holdStart.clear();
    lastRepeat.clear();
    rafId = requestAnimationFrame(pollFrame);
    return;
  }

  const pads = navigator.getGamepads ? navigator.getGamepads() : [];
  const pad  = Array.from(pads).find((p) => p !== null);

  if (pad) {
    _connected.set(true);

    // 1. Process non-directional buttons (0..11)
    for (let i = 0; i <= 11; i++) {
      const action = BUTTON_MAP[i];
      if (!action) continue;
      const btnObj = pad.buttons[i];
      const pressed = typeof btnObj === 'object' ? btnObj.pressed : btnObj === 1.0;
      processActionState(action, Boolean(pressed), now);
    }

    // 2. Compute combined direction states (D-pad buttons + Analog Sticks + Linux Axes)
    const btnUp    = Boolean(pad.buttons[12]?.pressed);
    const btnDown  = Boolean(pad.buttons[13]?.pressed);
    const btnLeft  = Boolean(pad.buttons[14]?.pressed);
    const btnRight = Boolean(pad.buttons[15]?.pressed);

    const axes = pad.axes || [];
    const stickX = axes[0] ?? 0;
    const stickY = axes[1] ?? 0;
    const dpadX  = axes[4] ?? axes[6] ?? 0;
    const dpadY  = axes[5] ?? axes[7] ?? 0;

    const isUp    = btnUp   || stickY < -AXIS_THRESHOLD || dpadY < -AXIS_THRESHOLD;
    const isDown  = btnDown || stickY > AXIS_THRESHOLD  || dpadY > AXIS_THRESHOLD;
    const isLeft  = btnLeft || stickX < -AXIS_THRESHOLD || dpadX < -AXIS_THRESHOLD;
    const isRight = btnRight|| stickX > AXIS_THRESHOLD  || dpadX > AXIS_THRESHOLD;

    processActionState('up', isUp, now);
    processActionState('down', isDown, now);
    processActionState('left', isLeft, now);
    processActionState('right', isRight, now);

  } else {
    _connected.set(false);
  }

  rafId = requestAnimationFrame(pollFrame);
}

export function startPolling() {
  if (rafId) return;
  prevPressed = {};
  rafId = requestAnimationFrame(pollFrame);
}

export function stopPolling() {
  cancelAnimationFrame(rafId);
  rafId = 0;
  prevPressed = {};
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
    const pads = navigator.getGamepads ? navigator.getGamepads() : [];
    const anyLeft = Array.from(pads).some((p) => p !== null);
    if (!anyLeft) {
      _connected.set(false);
    }
  });

  startPolling();
}
