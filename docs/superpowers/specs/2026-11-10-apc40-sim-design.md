# APC40 mkII Browser Simulator — Design

Date: 2026-11-10. Status: agreed with the user.

## Goal

A web app in `tools/apc40-sim/` that draws an Akai APC40 mkII (SVG/CSS replica of the real layout)
and behaves over real MIDI exactly like the connected hardware, so the MIDI-Router can be tested
without the device. **The router code is not changed.**

## Setup (Windows)

- Controller side: Windows MIDI Services loopback pair. Router uses `APC40 mkII Sim (A)` (name contains
  `APC40 mkII`, matching the existing config), the simulator uses `APC40 mkII Sim (B)`.
  A plain loopMIDI port cannot be used here: the router opens one name for both in and out, so it
  would read back its own LED messages (feedback loop; loopMIDI mutes the port).
- Software side: loopMIDI ports `to_Daslight` / `from_Daslight` as before.
- Browser: Chrome or Edge, Web MIDI with `sysex: true`. Served from localhost
  (`python -m http.server` inside `tools/apc40-sim/`). No build step, plain ES modules.

## Behaviour

Source of truth: `APC40Mk2_Communications_Protocol_v1.2.pdf` (repo root). "ch" = 0-based channel nibble.

### Startup state (also "Power cycle" button)
All LEDs off, all knob/fader values 0, every ring type Single (1), mode 0, selected track bank 0
(Track 1 selected, its LED lit). Nothing persisted.

### Buttons (device → host)
Press sends `9c nn 7F`, release sends `8c nn 7F` — for every button in every mode (also toggles).
Exception: in mode 0, Track Select 1-8 and Master send **nothing** (they select the bank).

| Control | Note | Channel |
|---|---|---|
| Clip pads, 5 rows x 8 | 0x00-0x27 | 0. Bottom row = 0x00-0x07, top row = 0x20-0x27 (as in Ableton's APC40_MkII script; PDF does not say the orientation — mark `// unverified`) |
| Record Arm / Solo / Activator / Track Select / Clip Stop | 0x30 / 0x31 / 0x32 / 0x33 / 0x34 | track 0-7 |
| Crossfade A/B | 0x42 | track 0-7 |
| Device ◀, ▶, Bank ◀, ▶, On/Off, Lock, Clip/Device View, Detail View | 0x3A-0x41 | mode 0: bank 0-8; mode 1/2: 0 |
| Master, Stop All Clips, Scene 1-5 | 0x50, 0x51, 0x52-0x56 | 0 |
| Pan, Sends, User, Metronome, Play, Record | 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5D | 0 |
| Up, Down, Right, Left, Shift, Tap Tempo, Nudge-, Nudge+, Session, Bank | 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67 | 0 |

No STOP (0x5C) button: the mkII has none.

### Continuous controls (device → host)
- Track faders 1-8: CC 0x07 on ch = track. Master fader CC 0x0E ch 0. Crossfader CC 0x0F ch 0. Absolute 0-127.
- Device knobs 1-8: CC 0x10-0x17; ch = bank in mode 0, ch 0 in mode 1/2. Value stored per bank.
- Track knobs 1-8: CC 0x30-0x37 ch 0. Not banked.
- Knobs are endless but report absolute values; device stores and clamps 0-127; it sends only when the value changes.
- Tempo CC 0x0D, Cue Level CC 0x2F, ch 0: relative. +1..+63 → 0x01-0x3F, −1..−64 → 0x7F..0x40.
- Footswitch CC 0x40 ch 0: press 0x7F, release 0x00.

### Mode 0 (Generic) local behaviour
- LED lit while held (momentary): clip pads, Clip Stop, Crossfade A/B, Device On/Off, Device Lock,
  Clip/Device View, Detail View, Bank, Scene Launch, Play, Record, Session. While held: RGB LEDs show
  colour index 21 (green, `// unverified`), other LEDs value 1; on release the LED goes off.
- Toggle (LED flips on each press): Activator, Solo, Record Arm, Device ◀/▶, Bank ◀/▶, Pan, Sends, User.
- No local LED: arrows, Shift, Nudge±, Metronome, Tap Tempo, Stop All Clips.
- Track Select 1-8 + Master: radio group, one LED lit. Pressing one sets the bank (0-7, Master = 8),
  then sends the 8 stored device-knob values of the new bank as CC 0x10-0x17 on ch = new bank.
  Pressing the already selected one also re-sends the dump.
- Device buttons 0x3A-0x41 and device knobs/rings keep separate state per bank; the panel shows the selected bank.
- Rings follow their knob locally.
- Host LED messages and local events write the same LED state; last write wins.

### Mode 1 (Ableton Live) / Mode 2 (Alternate Ableton Live)
All buttons momentary, no local LEDs, no banking (device controls on ch 0, Track Select sends 0x33 on
ch = track, Master sends 0x50). Mode 1: rings follow knobs locally and host can set them.
Mode 2: rings only change from host messages.

### Host → device
- LED on: `9c nn vv`. LED off: `8c nn xx` or `9c nn 00`.
- RGB notes (pads 0x00-0x27, scenes 0x52-0x56): ch 0 sets the primary colour `vv` (index into the
  table below) and clears any animation. ch 1-15 sets the secondary colour `vv` with animation:
  1-5 one-shot, 6-10 pulse, 11-15 blink, at rates 1/24, 1/16, 1/8, 1/4, 1/2 respectively.
  Rate r means a period of r whole notes in beats: 1/24 → 1/6 beat, 1/16 → 0.25, 1/8 → 0.5, 1/4 → 1, 1/2 → 2.
  Visuals (`// unverified`, PDF does not define them): blink = secondary for the first half of each
  period, primary for the second; pulse = secondary brightness 25%-100% sine over the period;
  one-shot = secondary for one period from when it was set, then primary.
- Clip Stop 0x34 ch = track: 0 off, 1 on, 2 blink 1/8, 3-127 on.
- Crossfade A/B 0x42 ch = track: 0 off, 1 yellow, 2-127 orange.
- Track notes 0x30-0x33 ch = track: 0 off, 1-127 on.
- Single LEDs, channel ignored: 0x3A-0x41, 0x50, 0x57-0x5B, 0x5D, 0x66, 0x67: 0 off, 1-127 on.
  0x3A-0x41 apply to the currently selected bank.
- Ring value: CC 0x10-0x17 (device, ch = bank in mode 0, else ignored) and CC 0x30-0x37 (track) set the
  knob value and redraw the ring. Ring type: CC 0x18-0x1F (device) and CC 0x38-0x3F (track):
  0 off, 1 Single, 2 Volume, 3 Pan, 4-127 Single.
- Undefined input (note without an LED such as 0x44-0x4F, 0x51, 0x5C, 0x5E-0x65, 0x68-0x7F;
  channel 8-15 on 0x30-0x34/0x42; unknown CC; unknown SysEx) is ignored and reported as a warning
  `"undefined on hardware: <reason>"`.
- MIDI clock: `F8` ticks (24 per beat) drive animation when one arrived within the last 500 ms;
  otherwise an internal BPM (UI input, default 120).

### SysEx
- Device Inquiry `F0 7E <ch|7F> 06 01 F7` → reply (35 bytes)
  `F0 7E <same ch> 06 02 47 29 00 19 01 00 00 00 7F 00 00 00 00` + 15 × `00` + `00 F7`.
  Firmware and serial bytes are placeholders (`// unverified`).
- Introduction `F0 47 <any> 29 60 00 04 <40|41|42> <v1> <v2> <v3> F7` → sets mode 0/1/2, then replies
  `F0 47 7F 29 61 00 04 <f1..f9> F7` with the 9 fader values (tracks 1-8, master). Length bytes copied
  literally from the PDF (`// unverified`, PDF lists 4 but sends 9).

### Ring patterns (15 LEDs, index 1-15 left to right)
- Single: thresholds where the lit LED(s) change, value → LEDs:
  0-3:1, 4-8:1+2, 9-12:2, 13-17:2+3, 18-21:3, 22-25:3+4, 26-30:4, 31-34:4+5, 35-38:5, 39-43:5+6,
  44-47:6, 48-52:6+7, 53-56:7, 57-60:7+8, 61-65:8, 66-69:8+9, 70-73:9, 74-78:9+10, 79-82:10,
  83-87:10+11, 88-91:11, 92-95:11+12, 96-100:12, 101-104:12+13, 105-108:13, 109-113:13+14,
  114-117:14, 118-122:14+15, 123-127:15.
- Volume: 0:none, 1-9:1, 10-18:1-2, 19-27:1-3, 28-36:1-4, 37-45:1-5, 46-54:1-6, 55-63:1-7, 64-71:1-8,
  72-80:1-9, 81-89:1-10, 90-98:1-11, 99-107:1-12, 108-116:1-13, 117-126:1-14, 127:1-15.
- Pan: 0-8:1-8, 9-17:2-8, 18-26:3-8, 27-35:4-8, 36-44:5-8, 45-53:6-8, 54-62:7-8, 63-64:8, 65-73:8-9,
  74-82:8-10, 83-91:8-11, 92-100:8-12, 101-109:8-13, 110-118:8-14, 119-127:8-15.
- Off: none.

### RGB colour table (velocity → hex)
```
0 000000  1 1E1E1E  2 7F7F7F  3 FFFFFF  4 FF4C4C  5 FF0000  6 590000  7 190000
8 FFBD6C  9 FF5400 10 591D00 11 271B00 12 FFFF4C 13 FFFF00 14 595900 15 191900
16 88FF4C 17 54FF00 18 1D5900 19 142B00 20 4CFF4C 21 00FF00 22 005900 23 001900
24 4CFF5E 25 00FF19 26 00590D 27 001902 28 4CFF88 29 00FF55 30 00591D 31 001F12
32 4CFFB7 33 00FF99 34 005935 35 001912 36 4CC3FF 37 00A9FF 38 004152 39 001019
40 4C88FF 41 0055FF 42 001D59 43 000819 44 4C4CFF 45 0000FF 46 000059 47 000019
48 874CFF 49 5400FF 50 190064 51 0F0030 52 FF4CFF 53 FF00FF 54 590059 55 190019
56 FF4C87 57 FF0054 58 59001D 59 220013 60 FF1500 61 993500 62 795100 63 436400
64 033900 65 005735 66 00547F 67 0000FF 68 00454F 69 2500CC 70 7F7F7F 71 202020
72 FF0000 73 BDFF2D 74 AFED06 75 64FF09 76 108B00 77 00FF87 78 00A9FF 79 002AFF
80 3F00FF 81 7A00FF 82 B21A7D 83 402100 84 FF4A00 85 88E106 86 72FF15 87 00FF00
88 3BFF26 89 59FF71 90 38FFCC 91 5B8AFF 92 3151C6 93 877FE9 94 D31DFF 95 FF005D
96 FF7F00 97 B9B000 98 90FF00 99 835D07 100 392B00 101 144C10 102 0D5038 103 15152A
104 16205A 105 693C1C 106 A8000A 107 DE513D 108 D86A1C 109 FFE126 110 9EE12F 111 67B50F
112 1E1E30 113 DCFF6B 114 80FFBD 115 9A99FF 116 8E66FF 117 404040 118 757575 119 E0FFFF
120 A00000 121 350000 122 1AD000 123 074200 124 B9B000 125 3F3100 126 B35F00 127 4B1502
```

## UI

- Layout as on PDF p.10 / the real device: top-left 8 track knobs with rings; 5×8 pad grid + scene
  column; Clip Stop row + Stop All; Track Select row + Master; Activator/Crossfade pairs; Solo/Arm
  pairs + Cue Level knob; 9 faders. Right side: Pan/Sends/User column, Play (green)/Record (red)/
  Session (red), Metronome, Tap, Nudge-/+, Tempo knob; 8 device knobs (2×4) with rings; Device/Bank
  arrows row; On/Off, Lock, Clip/Device View, Detail View row; Shift, arrow cross, Bank; crossfader;
  footswitch button. LED colours: single LEDs orange except Play green, Record/Session red,
  Solo blue, Arm red; Crossfade yellow/orange.
- Input: mouse/touch hold = pressed; Ctrl+click latches a button (second Ctrl+click releases);
  Shift button latches on plain click; faders drag; knobs vertical drag and mouse wheel (1 step per
  wheel notch, drag 1 step per 2 px); relative encoders send one step per notch/2 px.
  No keyboard shortcuts.
- Top bar: MIDI input/output dropdowns (default: first port whose name contains `APC40`), mode
  indicator, clock source + BPM input, "Power cycle".
- Collapsible MIDI monitor: direction, hex bytes, decoded text (e.g. `Note On ch1 #82 vel127`),
  warnings highlighted yellow. Keeps the last 500 lines.
- UI text in English.

## Tests
Protocol logic in plain ES modules with `node --test` (Node ≥ 18). UI checked manually in the browser.

## Out of scope
Router changes. Router bugs found during analysis are recorded in `docs/router-known-issues.md`.
