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

// On at least one Linux/WebKitGTK setup, pulling L2 bleeds its trigger
// value onto axes[1] — the same slot the Standard Gamepad mapping uses for
// the left stick's Y axis — even though pad.mapping reports "standard".
// The browser conflates the two before this code ever sees the data, so
// they can't be told apart by origin; requiring the threshold-crossing to
// hold for a beat filters the short antagonistic spike L2 produces without
// noticeably slowing down a deliberate stick push.
const STICK_STABLE_MS = 50;

type Listener = (action: GamepadAction) => void;
const listeners = new Set<Listener>();

const _connected = writable(false);
export const gamepadConnected: Readable<boolean> = { subscribe: _connected.subscribe };

// setInterval-driven, NOT requestAnimationFrame — this loop runs for the
// entire app lifetime regardless of view, and having it as a second
// concurrent rAF consumer alongside a view's own draw loop (e.g.
// ClipPlayerView's WebGL canvas) was found to throttle BOTH loops down to
// ~15fps on WebKitGTK/NVIDIA (reproduced directly: two independent
// simultaneous rAF loops on separate canvases both dropped to ~15fps,
// while either alone ran at full ~60fps). setInterval at roughly the same
// cadence preserves gamepad responsiveness without competing for rAF slots.
const POLL_INTERVAL_MS = 16;
let intervalId: ReturnType<typeof setInterval> | 0 = 0;
let prevPressed: Record<string, boolean> = {};
const holdStart  = new Map<GamepadAction, number>();
const lastRepeat = new Map<GamepadAction, number>();

function fire(action: GamepadAction) {
  for (const fn of listeners) fn(action);
}

// Chromium hands back GamepadButton objects ({ pressed, value }), but
// WebKitGTK on Linux has been observed handing back raw floats (0.0/1.0)
// for individual button slots instead — this normalizes both shapes.
function isButtonPressed(btnObj: GamepadButton | number | undefined): boolean {
  if (btnObj === undefined) return false;
  return typeof btnObj === 'object' ? btnObj.pressed : btnObj === 1.0;
}

// DEV-only: prints the raw pad snapshot whenever it changes, so a real
// mapping (e.g. the D-pad hat's actual axis indices) can be read off
// devtools instead of guessed at. Open devtools in `npx tauri dev`, press
// the D-pad / L2 / R2, and check the console.
let lastDebugSnapshot = '';
function debugDumpPadState(pad: Gamepad) {
  const buttons = pad.buttons.map((b, i) => {
    const val = typeof b === 'object' ? b.value : b;
    return Number(val) > 0.05 ? `${i}:${Number(val).toFixed(2)}` : null;
  }).filter(Boolean);
  const axes = pad.axes.map((a, i) => (Math.abs(a) > 0.05 ? `${i}:${a.toFixed(2)}` : null)).filter(Boolean);
  const snapshot = JSON.stringify({ buttons, axes });
  if (snapshot !== lastDebugSnapshot) {
    lastDebugSnapshot = snapshot;
    // eslint-disable-next-line no-console
    console.debug('[gamepad]', pad.id, 'mapping=' + JSON.stringify(pad.mapping), 'buttons=', buttons, 'axes=', axes);
  }
}

interface AxisDebounceState {
  rawDir: -1 | 0 | 1;
  rawSince: number;
  confirmedDir: -1 | 0 | 1;
}

function debounceAxis(state: AxisDebounceState, value: number, now: DOMHighResTimeStamp): -1 | 0 | 1 {
  const rawDir: -1 | 0 | 1 = value < -AXIS_THRESHOLD ? -1 : value > AXIS_THRESHOLD ? 1 : 0;

  if (rawDir !== state.rawDir) {
    state.rawDir = rawDir;
    state.rawSince = now;
  }

  if (rawDir === 0) {
    // Returning to center is trusted immediately — only asserting a
    // direction needs the settle time, not releasing one.
    state.confirmedDir = 0;
  } else if (rawDir !== state.confirmedDir && now - state.rawSince >= STICK_STABLE_MS) {
    state.confirmedDir = rawDir;
  }

  return state.confirmedDir;
}

const stickYDebounce: AxisDebounceState = { rawDir: 0, rawSince: 0, confirmedDir: 0 };
const stickXDebounce: AxisDebounceState = { rawDir: 0, rawSince: 0, confirmedDir: 0 };

function resetAxisDebounce() {
  stickYDebounce.rawDir = stickYDebounce.confirmedDir = 0;
  stickYDebounce.rawSince = 0;
  stickXDebounce.rawDir = stickXDebounce.confirmedDir = 0;
  stickXDebounce.rawSince = 0;
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

function pollFrame() {
  const now = performance.now();

  if (!browser || !document.hasFocus()) {
    prevPressed = {};
    holdStart.clear();
    lastRepeat.clear();
    resetAxisDebounce();
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
      processActionState(action, isButtonPressed(pad.buttons[i]), now);
    }

    // 2. Compute combined direction states (D-pad buttons + Analog Stick)
    //
    // A previous version also OR'd in axes[4..7] as a guessed "Linux hat"
    // fallback for the D-pad. That guess turned out to collide with L2/R2 —
    // on at least one Linux/WebKitGTK controller, the analog trigger axes
    // land at those same indices, so pulling L2 was read as "d-pad down".
    // Removed until we have verified axis indices for a real hat (see the
    // DEV-only diagnostic dump below) — the analog stick remains a working
    // substitute for directional input in the meantime.
    const btnUp    = isButtonPressed(pad.buttons[12]);
    const btnDown  = isButtonPressed(pad.buttons[13]);
    const btnLeft  = isButtonPressed(pad.buttons[14]);
    const btnRight = isButtonPressed(pad.buttons[15]);

    const axes = pad.axes || [];
    const stickX = axes[0] ?? 0;
    const stickY = axes[1] ?? 0;

    const stickYDir = debounceAxis(stickYDebounce, stickY, now);
    const stickXDir = debounceAxis(stickXDebounce, stickX, now);

    const isUp    = btnUp   || stickYDir < 0;
    const isDown  = btnDown || stickYDir > 0;
    const isLeft  = btnLeft || stickXDir < 0;
    const isRight = btnRight|| stickXDir > 0;

    processActionState('up', isUp, now);
    processActionState('down', isDown, now);
    processActionState('left', isLeft, now);
    processActionState('right', isRight, now);

    if (import.meta.env.DEV) debugDumpPadState(pad);

  } else {
    _connected.set(false);
  }
}

export function startPolling() {
  if (intervalId) return;
  prevPressed = {};
  intervalId = setInterval(pollFrame, POLL_INTERVAL_MS);
}

export function stopPolling() {
  if (intervalId) clearInterval(intervalId);
  intervalId = 0;
  prevPressed = {};
  holdStart.clear();
  lastRepeat.clear();
  resetAxisDebounce();
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
