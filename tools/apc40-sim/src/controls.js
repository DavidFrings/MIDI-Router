// APC40 mkII control inventory (see spec: Buttons, Continuous controls, Mode 0).
/** @typedef {{ id:string, label:string, type:'button'|'fader'|'knob'|'encoder'|'pedal',
 *   note?:number, cc?:number, track?:number, banked?:boolean,
 *   led:'rgb'|'single'|'clipstop'|'xfade'|'none', color?:'orange'|'green'|'red'|'blue',
 *   local:'momentary'|'toggle'|'radio'|'none', ringTypeCc?:number }} Control */

/** @type {Control[]} */
export const CONTROLS = [];
const n8 = (f) => Array.from({ length: 8 }, (_, i) => f(i));
const add = (c) => CONTROLS.push(c);
const btn = (id, label, note, led, local, extra = {}) =>
  add({ id, label, type: 'button', note, led, local, ...extra });

// Pad orientation: row 0 = top = 0x20-0x27, bottom = 0x00-0x07. // unverified
for (let row = 0; row < 5; row++)
  for (let col = 0; col < 8; col++)
    btn(`pad-r${row}c${col}`, `Pad ${row + 1}/${col + 1}`, (4 - row) * 8 + col, 'rgb', 'momentary');
for (let i = 0; i < 5; i++) btn(`scene-${i + 1}`, `Scene ${i + 1}`, 0x52 + i, 'rgb', 'momentary');

// per-track buttons: note, channel = track
const perTrack = (prefix, label, note, led, local, color) =>
  n8((t) => btn(`${prefix}-${t + 1}`, `${label} ${t + 1}`, note, led, local, { track: t, ...(color && { color }) }));
perTrack('clipstop', 'Clip Stop', 0x34, 'clipstop', 'momentary', 'orange');
btn('stopall', 'Stop All Clips', 0x51, 'none', 'none');
perTrack('select', 'Select', 0x33, 'single', 'radio', 'orange');
btn('master', 'Master', 0x50, 'single', 'radio', { color: 'orange' });
perTrack('activator', 'Activator', 0x32, 'single', 'toggle', 'orange');
perTrack('xfade', 'Crossfade A/B', 0x42, 'xfade', 'momentary');
perTrack('solo', 'Solo', 0x31, 'single', 'toggle', 'blue');
perTrack('arm', 'Rec Arm', 0x30, 'single', 'toggle', 'red');

for (let i = 1; i <= 8; i++) add({ id: `fader-${i}`, label: `Fader ${i}`, type: 'fader', cc: 0x07, track: i - 1, led: 'none', local: 'none' });
add({ id: 'fader-master', label: 'Master Fader', type: 'fader', cc: 0x0e, led: 'none', local: 'none' });
add({ id: 'crossfader', label: 'Crossfader', type: 'fader', cc: 0x0f, led: 'none', local: 'none' });

for (let i = 0; i < 8; i++) add({ id: `trackknob-${i + 1}`, label: `Track Knob ${i + 1}`, type: 'knob', cc: 0x30 + i, ringTypeCc: 0x38 + i, led: 'none', local: 'none' });
for (let i = 0; i < 8; i++) add({ id: `deviceknob-${i + 1}`, label: `Device Knob ${i + 1}`, type: 'knob', cc: 0x10 + i, ringTypeCc: 0x18 + i, banked: true, led: 'none', local: 'none' });
add({ id: 'tempo', label: 'Tempo', type: 'encoder', cc: 0x0d, led: 'none', local: 'none' });
add({ id: 'cue', label: 'Cue Level', type: 'encoder', cc: 0x2f, led: 'none', local: 'none' });
add({ id: 'footswitch', label: 'Footswitch', type: 'pedal', cc: 0x40, led: 'none', local: 'none' });

// device buttons: banked in mode 0, single LED, no track
const dev = (id, label, note, local) => btn(id, label, note, 'single', local, { banked: true, color: 'orange' });
dev('device-left', 'Device <', 0x3a, 'toggle');
dev('device-right', 'Device >', 0x3b, 'toggle');
dev('bank-left', 'Bank <', 0x3c, 'toggle');
dev('bank-right', 'Bank >', 0x3d, 'toggle');
dev('device-onoff', 'On/Off', 0x3e, 'momentary');
dev('device-lock', 'Lock', 0x3f, 'momentary');
dev('clip-device-view', 'Clip/Device View', 0x40, 'momentary');
dev('detail-view', 'Detail View', 0x41, 'momentary');

const one = (id, label, note, local, color = 'orange') => btn(id, label, note, 'single', local, { color });
one('pan', 'Pan', 0x57, 'toggle');
one('sends', 'Sends', 0x58, 'toggle');
one('user', 'User', 0x59, 'toggle');
one('metronome', 'Metronome', 0x5a, 'none'); // orange LED, but no local behaviour
one('play', 'Play', 0x5b, 'momentary', 'green');
one('record', 'Record', 0x5d, 'momentary', 'red');
one('session', 'Session', 0x66, 'momentary', 'red');
one('bank', 'Bank', 0x67, 'momentary');
for (const [id, label, note] of [['up', 'Up', 0x5e], ['down', 'Down', 0x5f], ['right', 'Right', 0x60], ['left', 'Left', 0x61],
  ['shift', 'Shift', 0x62], ['tap', 'Tap Tempo', 0x63], ['nudge-minus', 'Nudge -', 0x64], ['nudge-plus', 'Nudge +', 0x65]])
  btn(id, label, note, 'none', 'none');

const TRACK_NOTES = new Set([0x30, 0x31, 0x32, 0x33, 0x34, 0x42]);
export const byId = (id) => CONTROLS.find((c) => c.id === id);

export function findLed(note, ch) {
  if (TRACK_NOTES.has(note) && ch > 7) return null;
  return CONTROLS.find((c) => c.type === 'button' && c.led !== 'none' && c.note === note
    && (!TRACK_NOTES.has(note) || c.track === ch)) ?? null;
}
export function findByCc(cc, ch) {
  if (cc === 0x07) return ch > 7 ? null : CONTROLS.find((c) => c.cc === 0x07 && c.track === ch) ?? null;
  return CONTROLS.find((c) => c.cc === cc) ?? null;
}
export const findRingType = (cc) => CONTROLS.find((c) => c.ringTypeCc === cc) ?? null;
