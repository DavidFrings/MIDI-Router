# MIDI-Router known issues (found during APC40 simulator work)

For future agents. All issues below are open. Status of each: not fixed, out of scope of the
APC40 simulator work. Line numbers refer to the source at the time of writing (branch `feat/apc40-sim`).
Controller notes are hex in the APC40 mkII protocol (e.g. Arm = 0x30 = 48).

## 1. LED feedback uses the remapped software note, not the controller note

- Evidence: `src/router/midi_handler.rs:94` and `:113` remap the controller note with `remap_note`;
  `:116-121` (NoteOff) and `:207-212` (`toggle_note_handler`, called at `:97-103`) pass the remapped
  note to `refresh_single_led`. `src/router/led_controller.rs:40-42` sends it to the controller
  unchanged. Remap table: `configs/akai-apc40-mk2/akai-apc40-mk2.config.toml:16-34`.
- Impact: Arm track 1 (note 0x30 = 48, ch 1) is remapped to 40 (`config.toml:17-18`), so its LED
  command goes to note 40, not 48. The Arm LED never lights; note 40 is not an Arm LED (per the
  APC40 mkII protocol PDF p.13, pads are 0x00-0x27; 40 = 0x28 is neither a pad nor Arm).

## 2. `refresh_all_leds` sends notes 0-63 on channel 1 / 13

- Evidence: `src/router/led_controller.rs:13-26` loops over all `toggle_notes`
  (`config.toml:12-14`: 0-63); `led_controller.rs:39-43` uses `Channel::Ch13` for ON and `Channel::Ch1` for OFF.
- Impact: for notes 0x30-0x39 the channel selects the track, so every refresh writes track 1 only.
  Channel 13 (index 12) is undefined on hardware for those notes. Bank switches
  (`midi_handler.rs:58-80`) trigger this refresh.

## 3. Remapped notes 68-79 and 105-120 are dropped (not in toggle_notes)

- Evidence: `config.toml:20-34` remaps notes 49 (Solo), 50 (Activator), 52 (Clip Stop), 66 (Crossfade A/B)
  to 68-79 and 105-120 for some banks. `toggle_notes` contains only 0-63 (`config.toml:12-14`), so
  `is_toggle_note` fails and the message is only logged as a warning: `midi_handler.rs:104-109`
  (controller NoteOn), `:122-127` (NoteOff), `:163-167` and `:181-185` (software side).
- Impact: those buttons never reach the software and get no LED feedback. This differs from the
  assumption that the router sends these notes to the device (it discards them). Whether that is
  intended is unclear; check against the Daslight mapping before changing it.

## 4. Initial bank hardcoded

- Evidence: `src/router/state_manager.rs:20`: `current_bank: Channel::Ch1, // ToDo: get from controller`.
- Impact: if the controller starts on another bank, the router's bank is wrong until the first
  CC 16 (`midi_handler.rs:63-79`) arrives.

## 5. Router cannot send SysEx

- Evidence: 3-byte buffers with `copy_to_slice` in `src/router/midi_handler.rs:233-235`
  (`send_midi_message`) and `src/router/led_controller.rs:60-62` (also dead code at
  `midi_connection.rs:447`). A SysEx message does not fit, so `copy_to_slice` returns an error.
  Controller SysEx reaches `forward_raw_message` via `midi_handler.rs:136-138` and fails; software
  SysEx is ignored (`midi_handler.rs:189`). `src/router/output_connection.rs` only holds the connection.
- Impact: device SysEx replies are forwarded to the software and fail with an error; mode
  introduction SysEx from the software never reaches the controller.
