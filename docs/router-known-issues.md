# MIDI-Router known issues

Found while building the APC40 mkII simulator (`tools/apc40-sim/`), re-checked against `main` after
PR #8 and #9. Code references name functions rather than line numbers, so they survive edits.
Controller notes are given in hex as in the APC40 mkII protocol (e.g. Arm = 0x30 = 48).

Use the simulator to reproduce: its monitor flags every message the hardware does not define as
`undefined on hardware`.

## Open

### 1. LED feedback uses the remapped software note, not the controller note

- **Where:** `MidiHandler::process_controller_message` remaps the controller note with
  `MappingConfig::remap_note`, then passes the remapped note to `LedController::refresh_single_led`
  (directly on NoteOff, via `toggle_note_handler` on NoteOn). `refresh_single_led` sends it to the
  controller unchanged.
- **Impact:** Arm track 1 (0x30, ch 1) is remapped to 40 in
  `configs/akai-apc40-mk2/akai-apc40-mk2.config.toml`, so its LED command goes to note 40 (0x28). That
  is neither a pad (pads are 0x00-0x27, protocol PDF p.13) nor Arm, so the Arm LED never lights.

### 2. `refresh_all_leds` writes every toggle note on channel 1 / 15

- **Where:** `LedController::refresh_all_leds` loops over all `toggle_notes` (0-63 in the APC40 config).
  `refresh_single_led` sends ch 1 for off and ch 1 + ch 15 for on.
- **Impact:** for 0x30-0x34 the channel selects the track, so every refresh writes track 1 only
  (ch 15 is undefined there). 0x35-0x39 have no LED. Runs on every bank switch (`handle_site_change`).

### 3. Remapped notes outside `toggle_notes` are dropped

- **Where:** the APC40 config remaps Solo (0x31), Activator (0x32), Clip Stop (0x34) and Crossfade A/B (0x42)
  to 68-79 and 105-120 for some tracks. `toggle_notes` only holds 0-63, so `is_toggle_note` fails and
  `process_controller_message` / `process_software_message` only log a warning.
- **Impact:** those buttons never reach the software and get no LED feedback. Check the Daslight mapping
  before changing it; the drop may or may not be intended.

### 4. Initial bank is hardcoded

- **Where:** `StateManager::new`: `current_bank: Channel::Ch1, // ToDo: get from controller`.
- **Impact:** if the controller starts on another bank, the router's bank is wrong until the first
  CC 16 arrives (any Track Select press or device knob 1).

### 5. Router cannot send SysEx

- **Where:** `OutputConnection::send` copies every message into a 3-byte buffer (`copy_to_slice`).
- **Impact:** SysEx does not fit and returns an error. Device SysEx replies forwarded to the software
  (the `_` arm of `process_controller_message`) fail, and the router can never send a mode
  introduction to the controller.

### 6. LED colour unknown until Daslight reports "off" once

- **Where:** `process_software_message` takes Daslight's NoteOn vel 1 as "on" and keeps the stored
  colour. The colour only arrives with the "off" message (vel = colour).
- **Impact:** a scene that is already on when the router starts blinks with colour 0 (invisible) until
  it is switched off once.

## Fixed

- **`control_map` was never applied** (faders all arrived as CC 7): `remap_control` read `note_map`.
  Fixed in PR #8.
- **On/off LEDs could look identical:** "on" sent only the blinking secondary colour, so with
  primary == secondary the blink was invisible, and every Daslight NoteOn counted as "on". Fixed in PR #8:
  vel 1 = on (blink 1/4 on ch 15, primary cleared first), any other velocity = off with that colour (solid).
