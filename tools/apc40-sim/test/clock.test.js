import test from 'node:test';
import assert from 'node:assert/strict';
import { Clock } from '../src/clock.js';

test('internal clock', () => { const c = new Clock(120); assert.equal(c.source(0), 'internal'); assert.equal(c.beats(1000) - c.beats(500), 1); });
test('midi clock', () => { const c = new Clock(); for (let i=0;i<24;i++) c.tick(1000+i); assert.equal(c.source(1100), 'midi'); assert.equal(c.source(1700), 'internal'); });
test('bpm change does not jump', () => {
  const c = new Clock(120); const b = c.beats(1000); c.bpm = 60;
  assert.equal(c.bpm, 60); assert.equal(c.beats(1000), b); assert.equal(c.beats(2000) - b, 1);
});
test('internal -> midi continues forward', () => {
  const c = new Clock(120); const b = c.beats(1000); c.tick(1000);
  assert.ok(Math.abs(c.beats(1000) - (b + 1 / 24)) < 1e-9);
  c.tick(1020); assert.ok(Math.abs(c.beats(1020) - (b + 2 / 24)) < 1e-9);
});
test('midi -> internal never backwards', () => {
  const c = new Clock(120); for (let i = 0; i < 24; i++) c.tick(1000 + i);
  const m = c.beats(1023); assert.ok(c.beats(1700) >= m); assert.ok(c.beats(1800) > c.beats(1700));
});
