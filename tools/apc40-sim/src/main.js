// Entry point. Task 7 replaces the console logging with Web MIDI and adds the bar + monitor.
import { Device } from './device.js';
import { Clock } from './clock.js';
import { describe } from './describe.js';
import { buildPanel, paint } from './panel.js';

const device = new Device();
const clock = new Clock();
const svg = document.getElementById('panel');
const log = (msgs) => { for (const m of msgs) console.log(describe(m)); };

buildPanel(svg, {
  press: (id) => log(device.press(id)),
  release: (id) => log(device.release(id)),
  setValue: (id, v) => log(device.setValue(id, v)),
  turn: (id, d) => log(device.turn(id, d)),
});

const frame = () => { paint(svg, device, clock.beats(performance.now())); requestAnimationFrame(frame); };
requestAnimationFrame(frame);
