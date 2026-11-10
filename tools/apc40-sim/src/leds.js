// LED state -> CSS colour at a given beat position (null = off).
import { RGB } from './colors.js';

const NAMED = { orange: '#ff7a00', green: '#00e05a', red: '#ff2020', blue: '#2a6bff', yellow: '#ffd400' };
const idx = (i) => (i > 0 ? RGB[i] ?? null : null);
// blink/pulse phase on the global beat grid, not relative to `since`
const phaseOf = (beats, period) => { const p = (beats % period) / period; return p < 0 ? p + 1 : p; };

function pulse(hex, phase) { // unverified: 25%-100% cosine curve
  const f = 0.625 + 0.375 * Math.cos(2 * Math.PI * phase);
  const ch = [1, 3, 5].map((i) => Math.round(parseInt(hex.slice(i, i + 2), 16) * f).toString(16).padStart(2, '0'));
  return '#' + ch.join('');
}

export function ledColor(control, state, beats) {
  const { value, secondary, anim, period } = state;
  switch (control.led) {
    case 'rgb': {
      if (period > 0 && anim === 'blink') // unverified
        return idx(phaseOf(beats, period) < 0.5 ? secondary : value);
      if (period > 0 && anim === 'oneshot') // unverified
        return idx(beats - state.since < period ? secondary : value);
      if (period > 0 && anim === 'pulse') { // unverified
        const c = idx(secondary);
        return c && pulse(c, phaseOf(beats, period));
      }
      return idx(value);
    }
    case 'single': return value > 0 ? NAMED[control.color] ?? null : null;
    case 'clipstop': // blink period 0.5 beat, lit in first half // unverified
      if (value === 0) return null;
      return value === 2 && phaseOf(beats, 0.5) >= 0.5 ? null : NAMED.orange;
    case 'xfade': return value === 1 ? NAMED.yellow : value >= 2 ? NAMED.orange : null;
    default: return null;
  }
}
