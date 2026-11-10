# APC40 mkII Simulator

Browser-based Akai APC40 mkII front panel that talks to the MIDI-Router over Web MIDI.
Plain ES modules, no build, no dependencies.

## Setup (Windows)

1. Install Windows MIDI Services: `winget install Microsoft.WindowsMIDIServicesSDK`.
2. In MIDI Settings, create a loopback pair named `APC40 mkII Sim (A)` / `APC40 mkII Sim (B)`.
   Plain loopMIDI does not work for the controller port: the router opens one name for both input
   and output, so it would read back its own LED messages (feedback loop; loopMIDI mutes the port).
3. Create the loopMIDI ports `to_Daslight` and `from_Daslight` (software side).
4. Start the router.
5. In `tools/apc40-sim` run `python -m http.server 8000`.
6. Open `http://localhost:8000` in Chrome or Edge and allow MIDI and SysEx access.
7. Select port `APC40 mkII Sim (B)` as both input and output. The simulator preselects (B) when present; Check that it is selected for both, because (A) is the router's endpoint.

If Web MIDI is unavailable or denied, a red banner shows the error. The panel and monitor still
work; outgoing messages are logged with `(not sent)`.

## Controls

- Click: press a button; releasing the mouse releases it.
- Ctrl+click: latch a button held down. A plain click on a latched button releases it.
- Shift latches on a plain click (click again to release).
- Knobs: drag up/down (1 step per 2 px) or use the mouse wheel (1 step per 100 px, 3 lines or one page).
- Faders and crossfader: drag.
- Power cycle: resets the device to its startup state (mode 0, bank 1).

The top bar shows the mode, the bank, the clock source (MIDI clock or Internal), the BPM
(used while no MIDI clock arrives) and the incoming clock rate. Incoming `F8` is not logged.
The monitor logs `→` (sent) and `←` (received) messages, with warnings highlighted; it keeps 500 lines.

## Tests

```
node --test "tools/apc40-sim/test/*.test.js"
```
