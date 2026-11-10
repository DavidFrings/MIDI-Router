// LED ring patterns. Rows: [maxValue, firstLed, lastLed] (1-based, inclusive).
export const RING = { OFF: 0, SINGLE: 1, VOLUME: 2, PAN: 3 };

const SINGLE_MAX = [3,8,12,17,21,25,30,34,38,43,47,52,56,60,65,69,73,78,82,87,91,95,100,104,108,113,117,122,127];
const SINGLE = SINGLE_MAX.map((m, i) => { const f = (i >> 1) + 1; return [m, f, f + (i & 1)]; });

const VOLUME = [[0,1,0],[9,1,1],[18,1,2],[27,1,3],[36,1,4],[45,1,5],[54,1,6],[63,1,7],[71,1,8],
  [80,1,9],[89,1,10],[98,1,11],[107,1,12],[116,1,13],[126,1,14],[127,1,15]];

const PAN = [[8,1,8],[17,2,8],[26,3,8],[35,4,8],[44,5,8],[53,6,8],[62,7,8],[64,8,8],[73,8,9],
  [82,8,10],[91,8,11],[100,8,12],[109,8,13],[118,8,14],[127,8,15]];

const TABLES = { [RING.VOLUME]: VOLUME, [RING.PAN]: PAN };

export function ringLeds(type, value) {
  const leds = new Array(15).fill(false);
  if (type === RING.OFF) return leds;
  const rows = TABLES[type] ?? SINGLE; // type 4-127 behaves as SINGLE
  const [, first, last] = rows.find(([max]) => value <= max);
  for (let i = first; i <= last; i++) leds[i - 1] = true;
  return leds;
}
