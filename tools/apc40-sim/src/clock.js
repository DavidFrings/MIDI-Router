// Beat clock: internal BPM, or MIDI clock (24 ticks/beat) while ticks arrive.
const TIMEOUT = 500;

export class Clock {
  #bpm; #anchorBeats = 0; #anchorMs = 0; #midiBeats = 0; #lastTick = -Infinity; #mode = 'internal'; #seen = 0;
  constructor(bpm = 120) { this.#bpm = bpm; }

  get bpm() { return this.#bpm; }
  set bpm(v) { // re-anchor at the last time seen so beats never jump
    this.#anchorBeats = this.#internal(this.#seen); this.#anchorMs = this.#seen; this.#bpm = v;
  }

  #internal(now) { return this.#anchorBeats + ((now - this.#anchorMs) * this.#bpm) / 60000; }
  #sync(now) {
    this.#seen = Math.max(this.#seen, now);
    if (this.#mode === 'midi' && now - this.#lastTick >= TIMEOUT) {
      // continue from the last MIDI beat value, never backwards
      this.#mode = 'internal'; this.#anchorBeats = this.#midiBeats; this.#anchorMs = this.#lastTick;
    }
  }
  tick(now) {
    this.#sync(now);
    if (this.#mode === 'internal') { this.#midiBeats = this.#internal(now); this.#mode = 'midi'; }
    this.#midiBeats += 1 / 24;
    this.#lastTick = now;
  }
  source(now) { this.#sync(now); return this.#mode; }
  beats(now) { this.#sync(now); return this.#mode === 'midi' ? this.#midiBeats : this.#internal(now); }
}
