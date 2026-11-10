import test from 'node:test';
import assert from 'node:assert/strict';
import { describe } from '../src/describe.js';

test('describe', () => { assert.equal(describe([0x90,0x52,0x7f]), 'Note On ch1 #82 vel127'); assert.equal(describe([0x81,0x30,0x7f]), 'Note Off ch2 #48 vel127'); assert.equal(describe([0xb0,0x07,64]), 'CC ch1 #7 = 64'); assert.equal(describe([0xf8]), 'Clock'); assert.match(describe([0xf0,0x7e,0x7f,0x06,0x01,0xf7]), /^SysEx/); });
test('describe details', () => {
  assert.equal(describe([0xf0,0x0a,0xf7]), 'SysEx F0 0A F7 (3 bytes)');
  assert.equal(describe([0xfa]), 'Start'); assert.equal(describe([0xfb]), 'Continue');
  assert.equal(describe([0xfc]), 'Stop'); assert.equal(describe([0xfe]), 'Active Sensing');
  assert.equal(describe([0xe0, 1, 2]), 'Other E0 01 02');
  assert.equal(describe([]), 'Other');
});
