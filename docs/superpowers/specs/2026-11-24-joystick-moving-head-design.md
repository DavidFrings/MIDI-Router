# Joystick: moving-head pan/tilt from a Thrustmaster TCA Sidestick

Date: 2026-11-24. Status: agreed with the user in a grilling session. "Builder's call" marks decisions
the user did not make.

## Intent

Next to the APC40 the user wants to steer moving heads with a Thrustmaster TCA Sidestick Airbus Edition
(USB HID, Windows sees it as "T.A320 Pilot" or "T.A320 Copilot"). The stick drives pan and tilt as
MIDI faders in Daslight. A fine mode drives the pan-fine and tilt-fine channels instead, so there are
4 faders per moving head. Several moving heads (banks) can be switched with buttons on the stick.

## Hardware facts (from the manual)

- X/Y: 14 bit, 0..16383, spring return to centre.
- RZ (twist): 8 bit. Slider0 (throttle). Button 17 is virtual (first 20 % of throttle travel).
- Buttons 1 (trigger), 2-4 (head), 5-16 (base, 12 buttons). 8-way POV hat.
- No driver needed. Which head button is physically "2" and the Windows axis order are not verified yet
  (the probe log settles both).

## Behaviour

- **Move only while the trigger (button 1) is held.** Releasing the trigger stops all movement, so the
  stick springing back never moves a head.
- **Rate control.** Stick deflection sets the speed of the fader value, not the value itself.
  Below `deadzone` nothing moves. Above it the speed follows `((|d| - deadzone) / (1 - deadzone))^curve`.
  Full deflection moves a fader across 0..127 in `speed` seconds. The value is kept as a float per fader
  so very slow movement still accumulates; a CC is sent only when the rounded 7-bit value changes.
  Values clamp at 0 and 127.
- **Fine mode.** Button 2 toggles fine/coarse for the current bank. Every bank has its own fine flag.
  In fine mode the stick moves only `pan_fine` / `tilt_fine`, the coarse faders stay where they are.
  If the bank has no fine CC for an axis, that axis does nothing in fine mode.
- **Banks.** A bank is one moving head: its own `pan`, `tilt` and optional `pan_fine`, `tilt_fine` CCs.
  Every bank remembers its 4 values and its fine flag. Hat left/right selects the previous/next bank
  (wraps around, builder's call). Base buttons 5-16 select bank 1-12 directly. Buttons without a
  configured bank, hat up/down, buttons 3-4, twist and throttle do nothing.
- **Start value 64** for every fader after a router start.
- **Feedback.** A CC that Daslight sends on the joystick channel and one of a bank's CCs replaces the
  stored value (so a scene or mouse move in Daslight does not cause a jump on the next stick move).
  Feedback CCs on the joystick channel never reach the APC fader logic.
- **Independent of the APC.** Own MIDI channel, own banks; the APC bank never changes the joystick bank
  and the other way round. Output goes to the same `to_<software>` port as the APC traffic.
- **Optional + hotplug.** Without a `[joystick]` section the feature is off. With it, the router runs
  without the stick and picks it up when it is plugged in; unplugging is logged and shown, not an error.

## Config

```toml
[joystick]
channel = 16            # 1..16
device = "T.A320"       # optional, substring of the device name; empty/missing = first joystick
speed = 2.0             # optional, seconds for 0..127 at full deflection
deadzone = 0.1          # optional, 0..1
curve = 2.0             # optional, exponent

[[joystick.banks]]      # bank 1
pan = 20
tilt = 21
pan_fine = 22           # optional
tilt_fine = 23          # optional
```

At most 12 banks (base buttons). CCs are 0..127. Invalid values fail config loading with a clear error.

## Display

- **TUI:** one status line: connected yes/no, bank n/N, fine/coarse, the 4 values (`-` for a missing
  fine CC).
- **Web remote:** a read-only joystick view (own tab next to the pad grid). Two squares: coarse
  (pan x, tilt y) and fine (pan_fine x, tilt_fine y), each with a dot at the current value; the active
  mode is highlighted; bank n/N shown. A square for a missing fine pair is greyed out. Updates arrive as a
  new WebSocket event `joystick` and are part of the initial state a client receives.
  Hidden when `[joystick]` is not configured (builder's call).

## Architecture (builder's call)

- `src/joystick/config.rs`: `JoystickConfig`, validated, `Option` field in `Config`.
- `src/joystick/state.rs`: pure logic, no I/O: `JoystickState` takes `JoyInput` (button, hat, axis,
  connected) and `tick(dt)`, returns the CCs to send; `feedback(cc, value)`; `snapshot()`.
  Fully unit tested with a fake input sequence.
- `src/joystick/device.rs`: a small `JoystickSource` trait (poll → `Vec<JoyInput>`) with the real backend
  behind it. The backend (gilrs, hidapi or SDL) is chosen after the probe log. Until then only the trait
  and a fake exist.
- A joystick thread (started in `main` like the API thread, survives router restarts) polls the source
  at about 100 Hz, updates `Arc<Mutex<JoystickState>>`, sends CCs through `RemoteHub::handles()`'s
  `to_software` (skipped while the router is not connected) and broadcasts `RemoteEvent::Joystick`.
- Feedback: `MidiHandler::process_software_message` checks the joystick channel first and forwards to the
  shared state. Lock order: handler → joystick state; the joystick thread never holds the joystick
  lock while taking `to_software`, so no lock cycle.

## Probe tool

A separate crate `tools/joystick-probe/` (gilrs + hidapi stay out of the router) builds
`joystick-probe.exe`. It is sent to a friend who owns the stick. German console walk-through:

1. List devices (gilrs and hidapi): name, VID/PID, button/axis counts, HID report descriptor.
2. "Drücke nacheinander Trigger, Button 2, 3, 4".
3. "Drücke die Basis-Buttons 5-16 der Reihe nach".
4. "Hat: hoch, rechts, runter, links".
5. "Stick: ganz links, ganz rechts, ganz vor, ganz zurück, dann loslassen".
6. Background test: "Klick jetzt in ein anderes Fenster (z. B. Editor), dann Trigger drücken und Stick
   bewegen".
7. "Fertig. Schick die Datei joystick-probe-log.txt zurück."

gilrs (default filters off) and raw hidapi reports are logged in parallel during every step, with
timestamps and step markers, to `joystick-probe-log.txt` next to the exe.

Built by `.github/workflows/joystick-probe.yml` on `windows-latest` on push to paths
`tools/joystick-probe/**` (and `workflow_dispatch`), uploaded as artifact `joystick-probe`.

## Process

Worktree branch `feat/joystick`. Probe + workflow first and pushed early. Core logic, config, TUI and
web view in parallel with TDD against the fake source. Real backend after the log. Version bump to
0.4.0 before the PR. PR to `main`, merge only after the user's OK.

## Non-goals

- Inverting axes (Daslight does that).
- Twist, throttle, hat up/down, buttons 3-4.
- 14-bit / NRPN output; Daslight gets 7-bit CCs like everything else.
- Controlling the joystick from the web remote.
- Persisting joystick values across router restarts.
