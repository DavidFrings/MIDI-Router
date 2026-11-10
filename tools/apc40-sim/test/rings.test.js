import test from 'node:test';
import assert from 'node:assert/strict';
import { ringLeds } from '../src/rings.js';

const lit = (t, v) => ringLeds(t, v).flatMap((on, i) => (on ? [i + 1] : []));

test('single', () => { assert.deepEqual(lit(1,0),[1]); assert.deepEqual(lit(1,4),[1,2]); assert.deepEqual(lit(1,64),[8]); assert.deepEqual(lit(1,127),[15]); assert.deepEqual(lit(99,9),[2]); });
test('volume', () => { assert.deepEqual(lit(2,0),[]); assert.deepEqual(lit(2,1),[1]); assert.deepEqual(lit(2,126).length,14); assert.deepEqual(lit(2,127).length,15); });
test('pan', () => { assert.deepEqual(lit(3,0),[1,2,3,4,5,6,7,8]); assert.deepEqual(lit(3,63),[8]); assert.deepEqual(lit(3,64),[8]); assert.deepEqual(lit(3,65),[8,9]); assert.deepEqual(lit(3,127),[8,9,10,11,12,13,14,15]); });
test('off', () => { assert.deepEqual(lit(0,64),[]); });
test('every value of every type yields 15 entries', () => { for (const t of [0,1,2,3]) for (let v=0; v<128; v++) assert.equal(ringLeds(t,v).length, 15); });
