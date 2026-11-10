// Device state machine: user input side (see spec: Startup state, Buttons, Continuous controls, modes).
import { CONTROLS, byId, findLed, findByCc, findRingType } from './controls.js';

/** @typedef {{ value:number, secondary:number, anim:'none'|'oneshot'|'pulse'|'blink', period:number, since:number }} LedState */
const newLed = () => ({ value: 0, secondary: 0, anim: 'none', period: 0, since: 0 });
const hex = (b) => b.map((x) => x.toString(16).padStart(2, '0')).join(' ');
const warn = (reason, b) => ({ replies: [], warnings: [`undefined on hardware: ${reason} [${hex(b)}]`] });
const PERIODS = [1 / 6, 0.25, 0.5, 1, 2]; // 1/24, 1/16, 1/8, 1/4, 1/2 note in beats
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

  /** Host → device. Never throws. Clock/real-time bytes are handled by the caller. */
  receive(bytes, nowBeats = 0) {
    const ok = { replies: [], warnings: [] };
    if (!bytes.length) return warn('empty message', bytes);
    const st = bytes[0];
    if ([0xf8, 0xfa, 0xfb, 0xfc, 0xfe].includes(st)) return ok;
    if (st < 0x80) return warn('data byte without status', bytes);
    if (st === 0xf0) return this._sysex(bytes);
    const kind = st & 0xf0, ch = st & 0x0f;
    if (kind !== 0x80 && kind !== 0x90 && kind !== 0xb0) return warn('unsupported message type', bytes);
    if (bytes.length < 3) return warn('truncated message', bytes);
    const [, d1, d2] = bytes;
    if (kind === 0xb0) return this._hostCc(ch, d1, d2, bytes);
    const c = findLed(d1, ch);
    if (!c) return warn('note without LED or channel out of range', bytes);
    this._hostLed(c, ch, kind === 0x80 ? 0 : d2, nowBeats);
    return ok;
  }

  _hostLed(c, ch, vv, nowBeats) {
    if (c.led !== 'rgb' || ch === 0 || vv === 0) return this._setLed(c, vv);
    const l = this._led(c), i = ch - 1;
    // ch 1-5 one-shot, 6-10 pulse, 11-15 blink
    Object.assign(l, { secondary: vv, anim: ['oneshot', 'pulse', 'blink'][Math.floor(i / 5)], period: PERIODS[i % 5], since: nowBeats });
  }

  _hostCc(ch, cc, vv, bytes) {
    const ring = findRingType(cc);
    const c = ring ?? findByCc(cc, ch);
    if (!c || c.type !== 'knob') return warn('CC not writable by host', bytes);
    if (c.banked && this.mode === 0 && ch > 8) return warn('bank channel out of range', bytes);
    const bank = this.bank;
    if (c.banked && this.mode === 0) this.bank = ch;
    try {
      if (ring) this._ring(c).type = vv >= 4 ? 1 : vv;
      else { this._vals.set(this._key(c), vv); this._ring(c).value = vv; }
    } finally { this.bank = bank; }
    return { replies: [], warnings: [] };
  }

  _sysex(b) {
    if (b.at(-1) !== 0xf7) return warn('truncated SysEx', b);
    if (b.length === 6 && b[1] === 0x7e && b[3] === 6 && b[4] === 1) {
      // unverified: firmware and serial bytes are placeholders
      return { warnings: [], replies: [[0xf0, 0x7e, b[2], 0x06, 0x02, 0x47, 0x29, 0x00, 0x19, 0x01, 0x00, 0x00, 0x00, 0x7f, 0x00, 0x00, 0x00, 0x00, ...Array(15).fill(0), 0x00, 0xf7]] };
    }
    if (b.length === 12 && b[1] === 0x47 && b[3] === 0x29 && b[4] === 0x60 && b[5] === 0 && b[6] === 4) {
      const mode = b[7] - 0x40;
      if (mode < 0 || mode > 2) return warn('unknown introduction mode', b);
      this.mode = mode;
      const faders = [...Array(8).keys()].map((i) => this.value(`fader-${i + 1}`)).concat(this.value('fader-master'));
      // unverified: length bytes copied literally from the PDF (lists 4 but sends 9)
      return { warnings: [], replies: [[0xf0, 0x47, 0x7f, 0x29, 0x61, 0x00, 0x04, ...faders, 0xf7]] };
    }
    return warn('unknown SysEx', b);
  }
}
