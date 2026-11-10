// Device state machine: user input side (see spec: Startup state, Buttons, Continuous controls, modes).
import { CONTROLS, byId } from './controls.js';

/** @typedef {{ value:number, secondary:number, anim:'none'|'oneshot'|'pulse'|'blink', period:number, since:number }} LedState */
const newLed = () => ({ value: 0, secondary: 0, anim: 'none', period: 0, since: 0 });
const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));

export class Device {
  constructor() { this.reset(); }

  reset() {
    this.mode = 0;
    this.bank = 0;
    this._leds = new Map();
    this._vals = new Map();
    this._rings = new Map();
    this._led(byId('select-1')).value = 1;
  }

  _get(id) { const c = byId(id); if (!c) throw new Error(`unknown control: ${id}`); return c; }
  // Banked controls keep separate state per bank (9 banks).
  // Mode 1/2 have no banking: banked controls use the bank-0 slot.
  _key(c) { return c.banked ? `${c.id}@${this.mode === 0 ? this.bank : 0}` : c.id; }
  _led(c) { const k = this._key(c); if (!this._leds.has(k)) this._leds.set(k, newLed()); return this._leds.get(k); }
  _ring(c) { const k = this._key(c); if (!this._rings.has(k)) this._rings.set(k, { type: 1, value: 0 }); return this._rings.get(k); }

  led(id) { return this._led(this._get(id)); }
  value(id) { return this._vals.get(this._key(this._get(id))) ?? 0; }
  ring(id) { return this._ring(this._get(id)); }

  _setLed(c, v) { Object.assign(this._led(c), newLed(), { value: v }); }
  _channel(c) { return c.track ?? (c.banked && this.mode === 0 ? this.bank : 0); }

  press(id) {
    const c = this._get(id);
    if (c.type === 'pedal') return [[0xb0, c.cc, 0x7f]];
    if (c.type !== 'button') return [];
    if (this.mode !== 0) return [[0x90 | this._channel(c), c.note, 0x7f]];
    switch (c.local) {
      case 'momentary': this._setLed(c, c.led === 'rgb' ? 21 : 1); break; // 21 = green, unverified
      case 'toggle': this._setLed(c, this._led(c).value ? 0 : 1); break;
      case 'radio':
        for (const o of CONTROLS) if (o.local === 'radio') this._setLed(o, o === c ? 1 : 0);
        // Track Select / Master select the bank and send no note in mode 0.
        this.bank = c.id === 'master' ? 8 : c.track;
        return Array.from({ length: 8 }, (_, i) => [0xb0 | this.bank, 0x10 + i, this.value(`deviceknob-${i + 1}`)]);
    }
    return [[0x90 | this._channel(c), c.note, 0x7f]];
  }

  release(id) {
    const c = this._get(id);
    if (c.type === 'pedal') return [[0xb0, c.cc, 0x00]];
    if (c.type !== 'button') return [];
    if (this.mode === 0) {
      if (c.local === 'radio') return [];
      if (c.local === 'momentary') this._setLed(c, 0);
    }
    return [[0x80 | this._channel(c), c.note, 0x7f]];
  }

  setValue(id, v) {
    const c = this._get(id);
    if (c.type !== 'fader' && c.type !== 'knob') return [];
    v = clamp(Math.round(v), 0, 127);
    if (c.type === 'knob' && this.mode !== 2) this._ring(c).value = v;
    const k = this._key(c);
    if ((this._vals.get(k) ?? 0) === v) return [];
    this._vals.set(k, v);
    return [[0xb0 | this._channel(c), c.cc, v]];
  }

  turn(id, delta) {
    const c = this._get(id);
    if (c.type === 'knob') return this.setValue(id, this.value(id) + delta);
    if (c.type !== 'encoder') return [];
    delta = clamp(Math.round(delta), -64, 63);
    if (delta === 0) return [];
    return [[0xb0, c.cc, delta < 0 ? 128 + delta : delta]];
  }
}
