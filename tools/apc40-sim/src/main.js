// Entry point: wires device + clock + panel to Web MIDI, the top bar and the monitor.
import { Device } from './device.js';
import { Clock } from './clock.js';
import { describe } from './describe.js';
import { buildPanel, paint } from './panel.js';
import { openMidi } from './midi.js';

const $ = (id) => document.getElementById(id);
const device = new Device();
const clock = new Clock();
const svg = $('panel');
const hex = (b) => b.map((x) => x.toString(16).toUpperCase().padStart(2, '0')).join(' ');
let midi = null;

// ---- monitor ----
const logEl = $('log');
const MAX_LINES = 500;
function line(text, cls) {
  const atBottom = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight < 4;
  const d = document.createElement('div');
  d.textContent = text;
  if (cls) d.className = cls;
  logEl.appendChild(d);
  while (logEl.childElementCount > MAX_LINES) logEl.firstChild.remove();
  if (atBottom) logEl.scrollTop = logEl.scrollHeight;
}
$('clear').onclick = () => { logEl.textContent = ''; };
$('toggle-mon').onclick = () => $('monitor').classList.toggle('collapsed');

// ---- MIDI out / in ----
function out(msgs) {
  for (const m of msgs) {
    let sent = true;
    try { if (midi) midi.send(m); else sent = false; } catch (e) { sent = false; line(`send failed: ${e.message}`, 'warn'); }
    line(`→ ${hex(m)}  ${describe(m)}${sent ? '' : ' (not sent)'}`);
  }
}
let ticks = 0;
function incoming(bytes) {
  const now = performance.now();
  if (bytes[0] === 0xf8) { ticks++; clock.tick(now); return; }
  line(`← ${hex(bytes)}  ${describe(bytes)}`);
  const { replies, warnings } = device.receive(bytes, clock.beats(now));
  for (const w of warnings) line(`! ${w}`, 'warn');
  out(replies);
}

buildPanel(svg, {
  press: (id) => out(device.press(id)),
  release: (id) => out(device.release(id)),
  setValue: (id, v) => out(device.setValue(id, v)),
  turn: (id, d) => out(device.turn(id, d)),
});

// ---- top bar ----
const sel = { in: $('in'), out: $('out') };
const MODES = ['Mode 0 Generic', 'Mode 1 Ableton Live', 'Mode 2 Alternate Ableton Live'];
function fill(select, ports, keep) {
  select.textContent = '';
  select.add(new Option('— none —', ''));
  for (const p of ports) select.add(new Option(p.name, p.id));
  const pick = ports.find((p) => p.id === keep) ?? (keep === undefined ? ports.find((p) => /APC40/i.test(p.name)) : null);
  select.value = pick ? pick.id : '';
}
function apply() { midi?.use(sel.in.value, sel.out.value); }
function syncPorts(initial) {
  for (const [k, ports, label] of [['in', midi.inputs, 'input'], ['out', midi.outputs, 'output']]) {
    const prev = initial ? undefined : sel[k].value;
    fill(sel[k], ports, prev);
    if (prev && !sel[k].value) line(`MIDI ${label} port disappeared, selection reset to none`, 'warn');
  }
  apply();
}
sel.in.onchange = sel.out.onchange = apply;
$('power').onclick = () => { device.reset(); ticks = 0; line('Power cycle: device state reset'); };
$('bpm').onchange = (e) => {
  const v = Math.min(300, Math.max(20, Number(e.target.value) || 120));
  e.target.value = v;
  clock.setBpm(v, performance.now());
};

openMidi(incoming).then((m) => {
  midi = m; m.onChange = () => syncPorts(false); syncPorts(true);
}).catch((e) => {
  const b = $('banner'); b.textContent = e.message; b.hidden = false;
  fill(sel.in, [], null); fill(sel.out, [], null);
});

// ---- frame loop: panel + cheap status updates ----
const txt = (el, v) => { if (el.textContent !== v) el.textContent = v; };
let tickT = performance.now();
function frame() {
  const t = performance.now();
  paint(svg, device, clock.beats(t));
  txt($('mode'), MODES[device.mode] ?? `Mode ${device.mode}`);
  txt($('bank'), device.mode === 0 ? `Bank ${device.bank + 1}` : 'No banking');
  txt($('src'), clock.source(t) === 'midi' ? 'MIDI clock' : 'Internal');
  if (t - tickT >= 1000) { txt($('tps'), `clock: ${Math.round((ticks * 1000) / (t - tickT))} ticks/s`); ticks = 0; tickT = t; }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
