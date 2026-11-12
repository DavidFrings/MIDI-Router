import test, { mock } from 'node:test';
import assert from 'node:assert/strict';
import { GRID, padLook, createFaderLock, throttleLatest, loadPage, savePage } from '../src/model.js';
import { RGB } from '../src/colors.js';

test('GRID top row is 32-39 plus scene 82', () => {
  assert.equal(GRID.length, 5);
  assert.deepEqual(GRID[0], [32, 33, 34, 35, 36, 37, 38, 39, 82]);
});

test('GRID bottom row is 0-7 plus scene 86', () => {
  assert.deepEqual(GRID[4], [0, 1, 2, 3, 4, 5, 6, 7, 86]);
});

test('unknown colour is grey and steady', () => {
  assert.deepEqual(padLook({ on: true, color: null }), { color: '#555', blink: false });
  assert.deepEqual(padLook({ on: false, color: null }), { color: '#555', blink: false });
});

test('off is dimmed', () => {
  assert.deepEqual(padLook({ on: false, color: 5 }), { color: '#590000', blink: false }); // ff0000 * 0.35
  assert.deepEqual(padLook({ on: false, color: 3 }), { color: '#595959', blink: false });
});

test('on blinks in full colour', () => {
  assert.deepEqual(padLook({ on: true, color: 21 }), { color: RGB[21], blink: true });
});

test('fader lock ignores updates while touched and accepts after release', () => {
  const lock = createFaderLock();
  assert.equal(lock.accepts(2), true);
  lock.touch(2);
  assert.equal(lock.accepts(2), false);
  assert.equal(lock.accepts(3), true);
  lock.release(2);
  assert.equal(lock.accepts(2), true);
});

test('throttleLatest sends first value, then only latest within 30 ms', () => {
  mock.timers.enable({ apis: ['setTimeout', 'Date'] });
  try {
    const sent = [];
    const t = throttleLatest((k, v) => sent.push([k, v]));
    t(0, 10);
    t(0, 11);
    t(0, 12);
    t(1, 50);
    assert.deepEqual(sent, [[0, 10], [1, 50]]);
    mock.timers.tick(30);
    assert.deepEqual(sent, [[0, 10], [1, 50], [0, 12]]);
    mock.timers.tick(100);
    assert.equal(sent.length, 3);
    t(0, 13);
    assert.deepEqual(sent.at(-1), [0, 13]);
  } finally {
    mock.timers.reset();
  }
});

test('loadPage falls back to 1', () => {
  const store = (v) => ({ getItem: () => v, setItem() {} });
  assert.equal(loadPage(store(null)), 1);
  assert.equal(loadPage(store('0')), 1);
  assert.equal(loadPage(store('10')), 1);
  assert.equal(loadPage(store('abc')), 1);
  assert.equal(loadPage({ getItem() { throw new Error('denied'); } }), 1);
  assert.equal(loadPage(undefined), 1);
  assert.equal(loadPage(store('7')), 7);
});

test('savePage round-trips and survives throwing storage', () => {
  const m = new Map();
  const s = { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => m.set(k, v) };
  savePage(s, 4);
  assert.equal(loadPage(s), 4);
  savePage({ setItem() { throw new Error('quota'); } }, 4);
});
