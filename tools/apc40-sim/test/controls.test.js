import test from 'node:test';
import assert from 'node:assert/strict';
import { CONTROLS, byId, findLed, findByCc, findRingType } from '../src/controls.js';

test('pad notes', () => { assert.equal(byId('pad-r4c0').note, 0x00); assert.equal(byId('pad-r0c7').note, 0x27); });
test('track controls', () => { const a = byId('arm-3'); assert.equal(a.note, 0x30); assert.equal(a.track, 2); assert.equal(a.local, 'toggle'); assert.equal(a.color, 'red'); });
test('radio group', () => { assert.equal(byId('select-1').local, 'radio'); assert.equal(byId('master').local, 'radio'); assert.equal(byId('master').note, 0x50); });
test('no stop button', () => { assert.equal(CONTROLS.some(c => c.note === 0x5c), false); });
test('unique ids and note/track pairs', () => {
  assert.equal(new Set(CONTROLS.map(c => c.id)).size, CONTROLS.length);
  const pairs = CONTROLS.filter(c => c.type === 'button').map(c => `${c.note}/${c.track ?? 0}`);
  assert.equal(new Set(pairs).size, pairs.length);
});
test('findLed', () => {
  assert.equal(findLed(0x30, 2).id, 'arm-3'); assert.equal(findLed(0x30, 12), null);
  assert.equal(findLed(0x5b, 5).id, 'play'); assert.equal(findLed(0x51, 0), null);
  assert.equal(findLed(0x44, 0), null); assert.equal(findLed(0x3c, 3).id, 'bank-left');
});
test('knob lookups', () => {
  assert.equal(findByCc(0x10, 4).id, 'deviceknob-1'); assert.equal(findByCc(0x07, 3).id, 'fader-4');
  assert.equal(findByCc(0x07, 9), null); assert.equal(findRingType(0x3f).id, 'trackknob-8');
});
test('mode 0 local and device banking', () => {
  assert.equal(byId('metronome').local, 'none'); assert.equal(byId('metronome').led, 'single');
  assert.equal(byId('device-left').local, 'toggle'); assert.equal(byId('device-left').banked, true);
  assert.equal(byId('device-left').track, undefined); assert.equal(byId('deviceknob-3').banked, true);
  assert.equal(byId('solo-1').color, 'blue'); assert.equal(byId('play').color, 'green');
});
test('counts', () => {
  assert.equal(CONTROLS.filter(c => c.id.startsWith('pad-')).length, 40);
  assert.equal(CONTROLS.filter(c => c.id.startsWith('scene-')).length, 5);
  assert.equal(CONTROLS.filter(c => c.type === 'fader').length, 10);
  assert.equal(CONTROLS.filter(c => c.type === 'knob').length, 16);
  assert.equal(CONTROLS.filter(c => c.type === 'encoder').length, 2);
  assert.equal(CONTROLS.filter(c => c.type === 'pedal').length, 1);
});
