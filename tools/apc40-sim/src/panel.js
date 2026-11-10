// SVG front panel (layout after PDF p.10) and pointer/wheel input. No protocol logic here.
import { byId } from './controls.js';
import { ringLeds } from './rings.js';
import { ledColor } from './leds.js';

const NS = 'http://www.w3.org/2000/svg';
const reg = new WeakMap(); // svg -> { items, spins }
const RING_ON = '#ff7a00';
const RING_OFF = '#3a2410';
const OFF_STATE = { value: 1, secondary: 0, anim: 'none', period: 0, since: 0 };

function el(tag, attrs, parent) {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  parent?.appendChild(e);
  return e;
}
const set = (e, k, v) => { if (e.getAttribute(k) !== v) e.setAttribute(k, v); };
const dim = (hex) => '#' + [1, 3, 5].map((i) => Math.round(parseInt(hex.slice(i, i + 2), 16) * 0.22)
  .toString(16).padStart(2, '0')).join('');
const unlit = (c) => (c.led === 'rgb' ? '#2a2a2a' : dim(ledColor(c, OFF_STATE, 0)));

// ---- layout (viewBox 1280x720) ----
const COL = (i) => 62 + i * 62;
const PAD_Y = [122, 160, 198, 236, 274];
const RX = [760, 900, 1040, 1180]; // right block columns

function layout() {
  const L = [];
  const b = (id, x, y, w, h, label = '', o = {}) => L.push({ id, kind: 'button', x, y, w, h, label, ...o });
  const k = (id, x, y, r, label = '') => L.push({ id, kind: 'knob', x, y, r, label });
  const n8 = (f) => { for (let t = 0; t < 8; t++) f(t, COL(t)); };

  n8((t, x) => k(`trackknob-${t + 1}`, x, 58, 17));
  for (let r = 0; r < 5; r++) {
    n8((c, x) => b(`pad-r${r}c${c}`, x, PAD_Y[r], 50, 26, '', { rx: 5 }));
    b(`scene-${r + 1}`, COL(8), PAD_Y[r], 50, 26, '', { rx: 5 });
  }
  n8((t, x) => b(`clipstop-${t + 1}`, x, 318, 50, 18, 'Clip Stop'));
  b('stopall', COL(8), 318, 50, 18, 'Stop All');
  n8((t, x) => b(`select-${t + 1}`, x, 358, 50, 18, 'Select'));
  b('master', COL(8), 358, 50, 18, 'Master');
  n8((t, x) => { b(`activator-${t + 1}`, x - 14, 400, 24, 18, `${t + 1}`); b(`xfade-${t + 1}`, x + 14, 400, 24, 18, 'A|B'); });
  n8((t, x) => { b(`solo-${t + 1}`, x - 14, 440, 24, 18, 'Solo'); b(`arm-${t + 1}`, x + 14, 440, 24, 18, 'Arm'); });
  k('cue', COL(8), 418, 14, 'Cue Level');
  for (let i = 0; i < 9; i++)
    L.push({ id: i < 8 ? `fader-${i + 1}` : 'fader-master', kind: 'vfader', x: COL(i), a: 490, b: 668, label: i < 8 ? `${i + 1}` : 'Master' });

  // right block
  ['pan', 'sends', 'user'].forEach((id, i) => b(id, 640, PAD_Y[i], 36, 18, id));
  [['play', 'Play'], ['record', 'Record'], ['session', 'Session']].forEach(([id, label], i) =>
    b(id, 1000 + i * 90, 122, 44, 24, label, { ledAbove: true }));
  b('metronome', 1000, 172, 44, 18, 'Metronome');
  b('tap', 1090, 172, 44, 18, 'Tap Tempo');
  b('nudge-minus', 1000, 212, 44, 18, 'Nudge -');
  b('nudge-plus', 1090, 212, 44, 18, 'Nudge +');
  k('tempo', 1180, 192, 16, 'Tempo');
  for (let i = 0; i < 8; i++) k(`deviceknob-${i + 1}`, RX[i % 4], i < 4 ? 290 : 370, 20);
  ['device-left', 'device-right', 'bank-left', 'bank-right'].forEach((id, i) => b(id, RX[i], 440, 52, 18, byId(id).label));
  ['device-onoff', 'device-lock', 'clip-device-view', 'detail-view'].forEach((id, i) => b(id, RX[i], 480, 52, 18, byId(id).label));
  b('shift', RX[0], 560, 44, 20, 'Shift');
  b('up', 900, 528, 40, 18, 'Up');
  b('down', 900, 592, 40, 18, 'Down');
  b('left', 850, 560, 40, 18, 'Left');
  b('right', 950, 560, 40, 18, 'Right');
  b('bank', RX[2], 560, 44, 20, 'Bank');
  b('footswitch', RX[3], 560, 26, 14, 'Footswitch');
  L.push({ id: 'crossfader', kind: 'hfader', y: 655, a: 820, b: 1080, label: 'Crossfader' });
  return L;
}

// ---- rendering ----
function label(g, x, y, text) {
  if (text) el('text', { x, y, class: 'lbl', 'text-anchor': 'middle' }, g).textContent = text;
}

function renderButton(g, it, c) {
  const body = el('rect', { class: 'body', x: it.x - it.w / 2, y: it.y - it.h / 2, width: it.w, height: it.h,
    rx: it.rx ?? 3, fill: '#3a3a3e' }, g);
  label(g, it.x, it.y + it.h / 2 + 8, it.label);
  if (c.led === 'none') return;
  it.led = it.ledAbove ? el('circle', { cx: it.x, cy: it.y - it.h / 2 - 12, r: 5 }, g) : body;
  it.unlit = unlit(c);
}

function renderKnob(g, it, c) {
  if (c.type === 'knob') {
    const R = it.r + 8;
    it.dots = Array.from({ length: 15 }, (_, i) => {
      const a = ((-135 + (i * 270) / 14) * Math.PI) / 180;
      return el('circle', { cx: (it.x + R * Math.sin(a)).toFixed(1), cy: (it.y - R * Math.cos(a)).toFixed(1), r: 2.2, fill: RING_OFF }, g);
    });
  }
  el('circle', { class: 'body', cx: it.x, cy: it.y, r: it.r, fill: '#2d2d30', stroke: c.type === 'encoder' ? '#111' : '#555',
    'stroke-width': c.type === 'encoder' ? 4 : 1.5 }, g);
  it.pointer = el('line', { x1: it.x, y1: it.y - it.r + 3, x2: it.x, y2: it.y - 4, stroke: '#ddd', 'stroke-width': 2, 'stroke-linecap': 'round' }, g);
  label(g, it.x, it.y + it.r + 12, it.label);
}

function renderFader(g, it) {
  const v = it.kind === 'vfader';
  if (v) {
    el('line', { x1: it.x, y1: it.a, x2: it.x, y2: it.b, stroke: '#555', 'stroke-width': 3 }, g);
    el('rect', { x: it.x - 16, y: it.a - 10, width: 32, height: it.b - it.a + 20, fill: 'transparent' }, g);
    it.cap = el('rect', { class: 'body', x: it.x - 15, width: 30, height: 14, rx: 2, fill: '#c8c8cc' }, g);
    label(g, it.x, it.b + 22, it.label);
  } else {
    el('line', { x1: it.a, y1: it.y, x2: it.b, y2: it.y, stroke: '#555', 'stroke-width': 3 }, g);
    el('rect', { x: it.a - 10, y: it.y - 16, width: it.b - it.a + 20, height: 32, fill: 'transparent' }, g);
    it.cap = el('rect', { class: 'body', y: it.y - 15, width: 14, height: 30, rx: 2, fill: '#c8c8cc' }, g);
    label(g, (it.a + it.b) / 2, it.y + 30, it.label);
  }
}

// ---- input ----
function bindButton(g, id, h, latched) {
  g.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    if (latched.has(id)) { latched.delete(id); g.classList.remove('held'); h.release(id); return; }
    if (g.classList.contains('held')) return; // already held by another pointer
    g.classList.add('held');
    h.press(id);
    if (e.ctrlKey || id === 'shift') { latched.add(id); return; }
    g.setPointerCapture(e.pointerId);
    const up = (ev) => {
      if (ev.pointerId !== e.pointerId) return;
      g.removeEventListener('pointerup', up); g.removeEventListener('pointercancel', up);
      g.classList.remove('held');
      h.release(id);
    };
    g.addEventListener('pointerup', up); g.addEventListener('pointercancel', up);
  });
}

function bindDrag(g, onDown, onMove) {
  g.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    g.setPointerCapture(e.pointerId);
    onDown(e);
    const move = (ev) => { if (ev.pointerId === e.pointerId) onMove(ev); };
    const end = (ev) => {
      if (ev.pointerId !== e.pointerId) return;
      g.removeEventListener('pointermove', move); g.removeEventListener('pointerup', end); g.removeEventListener('pointercancel', end);
    };
    g.addEventListener('pointermove', move); g.addEventListener('pointerup', end); g.addEventListener('pointercancel', end);
  });
}

const latchedBySvg = new WeakMap();
/** Release all latched buttons visually, without sending messages (power cycle). */
export function unlatch(svg) {
  latchedBySvg.get(svg)?.clear();
  svg.querySelectorAll('.held').forEach((g) => g.classList.remove('held'));
}

export function buildPanel(svg, h) {
  const spins = new Map(); // endless encoders: running step count for the pointer angle
  const turn = (id, d) => { if (byId(id).type === 'encoder') spins.set(id, (spins.get(id) ?? 0) + d); h.turn(id, d); };
  const latched = new Set();
  latchedBySvg.set(svg, latched);
  const toSvg = (e) => new DOMPoint(e.clientX, e.clientY).matrixTransform(svg.getScreenCTM().inverse());

  svg.addEventListener('contextmenu', (e) => e.preventDefault());
  el('rect', { x: 4, y: 4, width: 1272, height: 712, rx: 16, fill: '#1b1b1d', stroke: '#333' }, svg);

  const items = layout();
  for (const it of items) {
    const c = byId(it.id);
    if (!c) throw new Error(`panel: unknown control ${it.id}`);
    it.c = c;
    const g = el('g', { 'data-id': it.id, class: `ctl ${it.kind}` }, svg);
    el('title', {}, g).textContent = c.label;
    if (it.kind === 'button') { renderButton(g, it, c); bindButton(g, it.id, h, latched); }
    else if (it.kind === 'knob') {
      renderKnob(g, it, c);
      let last = 0, acc = 0;
      bindDrag(g, (e) => { last = e.clientY; acc = 0; }, (e) => {
        acc += last - e.clientY; last = e.clientY; // up = +, 1 step per 2 px
        const steps = Math.trunc(acc / 2);
        if (steps) { acc -= steps * 2; turn(it.id, steps); }
      });
      let wacc = 0; // accumulated wheel: 100 px (pixels), 3 lines or 1 page per step
      g.addEventListener('wheel', (e) => {
        e.preventDefault();
        wacc += e.deltaY / [100, 3, 1][e.deltaMode];
        const steps = -Math.trunc(wacc);
        if (steps) { wacc += steps; turn(it.id, steps); }
      }, { passive: false });
    } else {
      renderFader(g, it);
      const valueAt = (e) => {
        const p = toSvg(e);
        return it.kind === 'vfader' ? ((it.b - p.y) / (it.b - it.a)) * 127 : ((p.x - it.a) / (it.b - it.a)) * 127;
      };
      const f = (e) => h.setValue(it.id, Math.min(127, Math.max(0, valueAt(e))));
      bindDrag(g, f, f);
    }
  }
  reg.set(svg, { items, spins });
}

export function paint(svg, device, beats) {
  const { items, spins } = reg.get(svg);
  for (const it of items) {
    if (it.led) set(it.led, 'fill', ledColor(it.c, device.led(it.id), beats) ?? it.unlit);
    if (it.dots) {
      const r = device.ring(it.id);
      ringLeds(r.type, r.value).forEach((on, i) => set(it.dots[i], 'fill', on ? RING_ON : RING_OFF));
    }
    if (it.pointer) {
      const a = it.c.type === 'encoder' ? ((spins.get(it.id) ?? 0) * 6) % 360 : -135 + (device.value(it.id) / 127) * 270;
      set(it.pointer, 'transform', `rotate(${a.toFixed(1)} ${it.x} ${it.y})`);
    }
    if (it.cap) {
      const t = device.value(it.id) / 127;
      if (it.kind === 'vfader') set(it.cap, 'y', (it.b - t * (it.b - it.a) - 7).toFixed(1));
      else set(it.cap, 'x', (it.a + t * (it.b - it.a) - 7).toFixed(1));
    }
  }
}
