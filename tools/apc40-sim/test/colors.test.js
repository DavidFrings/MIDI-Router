import test from 'node:test';
import assert from 'node:assert/strict';
import { RGB } from '../src/colors.js';

test('rgb table', () => {
  assert.equal(RGB.length, 128);
  assert.equal(RGB[0], '#000000'); assert.equal(RGB[5], '#ff0000');
  assert.equal(RGB[21], '#00ff00'); assert.equal(RGB[45], '#0000ff'); assert.equal(RGB[127], '#4b1502');
});
