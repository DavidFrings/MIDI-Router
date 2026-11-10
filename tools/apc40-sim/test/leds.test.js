import test from 'node:test';
import assert from 'node:assert/strict';
import { ledColor } from '../src/leds.js';
import { byId } from '../src/controls.js';

const st = v => ({ value:v, secondary:0, anim:'none', period:0, since:0 });
test('rgb primary/off', () => { const pad = byId('pad-r4c0'); assert.equal(ledColor(pad, st(5), 0), '#ff0000'); assert.equal(ledColor(pad, st(0), 0), null); });
test('rgb blink', () => { const pad = byId('pad-r4c0'); const s = { value:5, secondary:21, anim:'blink', period:0.5, since:0 }; assert.equal(ledColor(pad, s, 0.1), '#00ff00'); assert.equal(ledColor(pad, s, 0.3), '#ff0000'); });
test('rgb oneshot', () => { const pad = byId('pad-r4c0'); const s = { value:5, secondary:21, anim:'oneshot', period:1, since:2 }; assert.equal(ledColor(pad, s, 2.5), '#00ff00'); assert.equal(ledColor(pad, s, 3.5), '#ff0000'); });
test('clipstop blink and xfade', () => { assert.equal(ledColor(byId('clipstop-1'), st(2), 0.1), '#ff7a00'); assert.equal(ledColor(byId('clipstop-1'), st(2), 0.3), null); assert.equal(ledColor(byId('xfade-1'), st(1), 0), '#ffd400'); assert.equal(ledColor(byId('xfade-1'), st(2), 0), '#ff7a00'); assert.equal(ledColor(byId('play'), st(1), 0), '#00e05a'); });
test('extras', () => {
  const pad = byId('pad-r4c0');
  assert.equal(ledColor(pad, { value:0, secondary:21, anim:'blink', period:1, since:0 }, 0.7), null);
  assert.equal(ledColor(pad, { value:5, secondary:21, anim:'pulse', period:1, since:0 }, 0), '#00ff00');
  assert.equal(ledColor(pad, { value:5, secondary:21, anim:'pulse', period:1, since:0 }, 0.5), '#004000');
  assert.equal(ledColor(byId('clipstop-1'), st(1), 0.3), '#ff7a00');
  assert.equal(ledColor(byId('clipstop-1'), st(0), 0), null);
  assert.equal(ledColor(byId('play'), st(0), 0), null);
  assert.equal(ledColor(byId('xfade-1'), st(0), 0), null);
});
