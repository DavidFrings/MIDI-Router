// DOM wiring only; testable logic lives in model.js.
import { GRID, padLook, createFaderLock, throttleLatest, loadPage, savePage, createPendingReleases } from './model.js';
import { createApi } from './api.js';

const PIN_KEY = 'webRemote.pin';
const RETRY_MS = 2000;
const $ = (id) => document.getElementById(id);

const store = (() => { try { return window.localStorage; } catch { return null; } })();
let pin = (() => { try { return store.getItem(PIN_KEY); } catch { return null; } })();
let page = loadPage(store);
let online = false;
let ws = null;
let retryTimer = null;
let syncGen = 0;

const api = createApi(() => pin);
const lock = createFaderLock();
const padEls = new Map(); // note -> element
const faderEls = [];
const faderValues = Array(9).fill(null);
const held = new Map(); // note -> { page, pointerId }
const pendingReleases = createPendingReleases();

// ---------- build DOM ----------
for (let p = 1; p <= 9; p++) {
  const b = document.createElement('button');
  b.type = 'button';
  b.textContent = p;
  b.className = 'page';
  b.addEventListener('click', () => setPage(p));
  $('pages').append(b);
}

for (const row of GRID) {
  for (const note of row) {
    const el = document.createElement('div');
    el.className = note >= 82 ? 'pad scene' : 'pad';
    el.addEventListener('pointerdown', (e) => padDown(e, note, el));
    for (const t of ['pointerup', 'pointercancel', 'lostpointercapture']) {
      el.addEventListener(t, (e) => padUp(e, note, el));
    }
    padEls.set(note, el);
    $('grid').append(el);
  }
}

for (let i = 0; i < 9; i++) {
  const el = document.createElement('div');
  el.className = 'fader';
  el.innerHTML = '<div class="track"><div class="fill"></div><div class="thumb"></div></div><div class="flabel"></div>';
  el.querySelector('.flabel').textContent = i < 8 ? String(i + 1) : 'M';
  faderEls.push(el);
  wireFader(el, i);
  $('faders').append(el);
}

// ---------- rendering ----------
function drawPad(note, pad) {
  const el = padEls.get(note);
  if (!el) return;
  const look = padLook(pad);
  el.style.background = look.color;
  el.classList.toggle('blink', look.blink);
}

function drawFader(i) {
  const v = faderValues[i];
  faderEls[i].classList.toggle('unknown', v === null);
  faderEls[i].style.setProperty('--v', (v ?? 0) / 127);
}

function render(state) {
  for (const pad of state.pads) drawPad(pad.note, pad);
  state.faders.forEach((v, i) => {
    if (lock.accepts(i)) { faderValues[i] = v; drawFader(i); }
  });
}

function drawPages() {
  [...$('pages').children].forEach((b, i) => b.classList.toggle('active', i + 1 === page));
}

function setOnline(on, text = 'Offline') {
  online = on;
  document.body.classList.toggle('offline', !on);
  $('banner').hidden = on;
  if (!on) $('banner').textContent = text;
}

// ---------- connection ----------
function closeWs() {
  if (!ws) return;
  ws.onclose = ws.onmessage = ws.onopen = null;
  ws.close();
  ws = null;
}

function retryLater() {
  clearTimeout(retryTimer);
  retryTimer = setTimeout(connect, RETRY_MS);
}

let connectGen = 0;
async function connect() {
  clearTimeout(retryTimer);
  closeWs();
  if (!pin) { setOnline(false); return showPin(''); }
  const gen = ++connectGen;
  // One authenticated probe BEFORE the WS: a wrong PIN counts once for the lockout (a rejected
  // handshake is opaque and would count again) and the probe tells 401/429/503/offline apart.
  try { await api.state(page); } catch (e) { if (gen === connectGen) handleError(e); return; }
  if (gen !== connectGen) return;
  ws = api.openWs();
  ws.onopen = resync;
  ws.onmessage = (e) => onMessage(e.data);
  ws.onclose = () => {
    ws = null;
    setOnline(false);
    retryLater(); // connect() re-probes
  };
}

async function resync() {
  const gen = ++syncGen;
  try {
    // Failed releases first: a press must never stay stuck on the router.
    await pendingReleases.flush(sendRelease);
    const state = await api.state(page);
    if (gen !== syncGen || state.page !== page) return; // page changed meanwhile
    render(state);
    if (ws && ws.readyState === WebSocket.OPEN) setOnline(true);
  } catch (e) {
    if (gen === syncGen) handleError(e);
  }
}

function handleError(e) {
  if (e.status === 401 || e.status === 429) {
    clearTimeout(retryTimer);
    closeWs();
    setOnline(false);
    if (e.status === 401) {
      pin = null;
      try { store.removeItem(PIN_KEY); } catch { /* ignore */ }
    }
    showPin(e.status === 401 ? 'Wrong PIN' : 'Locked, try again in 60 s');
    return;
  }
  if (e.status === 400) return; // bad input from us; nothing to retry
  closeWs();
  setOnline(false, e.status === 503 ? 'Router not connected' : 'Offline');
  retryLater();
}

function onMessage(data) {
  let m;
  try { m = JSON.parse(data); } catch { return; }
  if (m.type === 'resync') return resync();
  if (m.page !== page) return;
  if (m.type === 'pad') drawPad(m.note, m);
  else if (m.type === 'fader' && lock.accepts(m.index)) {
    faderValues[m.index] = m.value;
    drawFader(m.index);
  }
}

function setPage(p) {
  if (p === page) return;
  page = p;
  savePage(store, p);
  drawPages();
  for (const el of padEls.values()) { el.style.background = ''; el.classList.remove('blink'); }
  if (ws && ws.readyState === WebSocket.OPEN) resync();
}

// ---------- pads ----------
function padDown(e, note, el) {
  e.preventDefault();
  if (!online || held.has(note)) return;
  try { el.setPointerCapture(e.pointerId); } catch { /* pointer already gone */ }
  held.set(note, { page, pointerId: e.pointerId });
  el.classList.add('held');
  api.press(page, note, true).catch(handleError);
}

// Release is never gated on `online`: a press that went out must always get its release.
function padUp(e, note, el) {
  const h = held.get(note);
  if (!h || (e && h.pointerId !== e.pointerId)) return;
  held.delete(note);
  el.classList.remove('held');
  sendRelease(h.page, note).catch((e) => {
    pendingReleases.add(h.page, note);
    handleError(e);
  });
}

// 400 = router rejects the input, so retrying can never succeed; treat as done.
function sendRelease(p, note) {
  return api.press(p, note, false).catch((e) => { if (e.status !== 400) throw e; });
}

function releaseAll() {
  for (const note of [...held.keys()]) padUp(null, note, padEls.get(note));
}

// ---------- faders ----------
const sendFader = throttleLatest((i, { p, v }) => api.fader(p, i, v).catch(handleError));

function wireFader(el, i) {
  let drag = null; // { pointerId, y, v, page }
  el.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    if (!online || drag) return;
    try { el.setPointerCapture(e.pointerId); } catch { /* ignore */ }
    lock.touch(i);
    el.classList.add('held');
    // Relative drag: touching the fader never makes the value jump.
    drag = { pointerId: e.pointerId, y: e.clientY, v: faderValues[i] ?? 0, page };
  });
  el.addEventListener('pointermove', (e) => {
    if (!drag || e.pointerId !== drag.pointerId) return;
    const h = el.querySelector('.track').clientHeight || 1;
    const v = Math.max(0, Math.min(127, Math.round(drag.v + ((drag.y - e.clientY) / h) * 127)));
    if (drag.page === page) { // after a page switch the view shows another page: don't touch it
      if (v === faderValues[i]) return;
      faderValues[i] = v;
      drawFader(i);
    }
    sendFader(i, { p: drag.page, v });
  });
  const end = (e) => {
    if (!drag || e.pointerId !== drag.pointerId) return;
    drag = null;
    lock.release(i);
    el.classList.remove('held');
  };
  for (const t of ['pointerup', 'pointercancel', 'lostpointercapture']) el.addEventListener(t, end);
}

// ---------- PIN ----------
function showPin(msg) {
  $('pin-msg').textContent = msg;
  $('pin-overlay').hidden = false;
  $('pin-input').value = '';
  $('pin-input').focus();
}

$('pin-form').addEventListener('submit', (e) => {
  e.preventDefault();
  const v = $('pin-input').value.trim();
  if (!v) return;
  pin = v;
  try { store.setItem(PIN_KEY, v); } catch { /* not remembered */ }
  $('pin-overlay').hidden = true;
  connect();
});

// ---------- lifecycle ----------
// A sleeping tablet leaves a half-open socket: reconnect when we come back.
document.addEventListener('visibilitychange', () => {
  if (document.hidden) releaseAll();
  else if ($('pin-overlay').hidden) connect();
});
window.addEventListener('pagehide', releaseAll);
window.addEventListener('online', () => { if ($('pin-overlay').hidden) connect(); });
document.addEventListener('contextmenu', (e) => e.preventDefault());
document.addEventListener('gesturestart', (e) => e.preventDefault()); // iOS pinch zoom
document.addEventListener('dblclick', (e) => e.preventDefault());

drawPages();
faderValues.forEach((_, i) => drawFader(i));
setOnline(false, 'Connecting…');
connect();
