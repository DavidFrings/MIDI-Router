// Testable logic for the tablet remote. No DOM here.
import { RGB } from './colors.js';

/** 5 rows x 9 notes: clip pads (top row 32-39) plus scene launch 82-86 in the last column. */
export const GRID = [0, 1, 2, 3, 4].map((r) => [
  ...Array.from({ length: 8 }, (_, c) => (4 - r) * 8 + c),
  82 + r,
]);

const dim = (hex, f) =>
  '#' + [1, 3, 5].map((i) => Math.round(parseInt(hex.slice(i, i + 2), 16) * f)
    .toString(16).padStart(2, '0')).join('');

/** Pad colour: unknown -> grey, off -> 35 % of palette colour, on -> full colour, blinking. */
export function padLook(pad) {
  if (pad.color == null || !RGB[pad.color]) return { color: '#555', blink: false };
  return pad.on ? { color: RGB[pad.color], blink: true } : { color: dim(RGB[pad.color], 0.35), blink: false };
}

/** Tracks faders under a finger, so server echoes do not make them jump. */
export function createFaderLock() {
  const held = new Set();
  return {
    touch: (i) => held.add(i),
    release: (i) => held.delete(i),
    accepts: (i) => !held.has(i),
  };
}

/** At most one send(key, value) per key per `ms`; the trailing call sends the latest value. */
export function throttleLatest(send, ms = 30) {
  const slots = new Map(); // key -> { pending: boolean, value }
  const open = (key) => setTimeout(() => {
    const s = slots.get(key);
    if (s.pending) {
      s.pending = false;
      send(key, s.value);
      open(key);
    } else {
      slots.delete(key);
    }
  }, ms);
  return (key, value) => {
    const s = slots.get(key);
    if (s) {
      s.pending = true;
      s.value = value;
      return;
    }
    slots.set(key, { pending: false, value });
    send(key, value);
    open(key);
  };
}

const PAGE_KEY = 'webRemote.page';

/** Last page 1-9 from storage; anything invalid or a throwing storage -> 1. */
export function loadPage(storage) {
  try {
    const raw = storage.getItem(PAGE_KEY);
    const n = Number(raw);
    return /^\d+$/.test(raw ?? '') && n >= 1 && n <= 9 ? n : 1;
  } catch {
    return 1;
  }
}

export function savePage(storage, page) {
  try {
    storage.setItem(PAGE_KEY, String(page));
  } catch {
    // private mode / blocked storage: page just is not remembered
  }
}
