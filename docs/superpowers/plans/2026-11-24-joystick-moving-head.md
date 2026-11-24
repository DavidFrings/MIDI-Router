# Joystick Moving-Head Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drive moving-head pan/tilt (+ fine) Daslight faders from a Thrustmaster TCA Sidestick, with banks, next to the APC40.

**Architecture:** Pure `JoystickState` (rate control, banks, fine, feedback) behind a `JoystickSource` trait; a joystick thread polls the source, sends 7-bit CCs on its own channel through the existing `to_software` port and broadcasts `RemoteEvent::Joystick`. A separate probe crate gathers real hardware data on Windows before the real backend is chosen.

**Tech Stack:** Rust 2024 (MSRV 1.89), wmidi, serde/toml, actix-web, ratatui; probe: gilrs 0.11 + hidapi 2; web remote: vanilla JS, `node --test`.

**Spec:** `docs/superpowers/specs/2026-11-24-joystick-moving-head-design.md`

## Global Constraints

- Router stays Windows-first; CI is `windows-latest`. `cargo test --locked` must pass.
- No gamepad/HID crate in the router crate until Task 7. gilrs/hidapi live only in `tools/joystick-probe/`.
- Output: 7-bit `ControlChange` on `[joystick].channel` (1..16) to `to_software`. No 14-bit/NRPN.
- Defaults: `speed = 2.0` s (0..127 at full deflection), `deadzone = 0.1`, `curve = 2.0`, start value `64`.
- Max 12 banks. Button numbers are the manual's 1-based numbers: 1 trigger, 2 fine toggle, 5..16 bank 1..12.
- Fader order everywhere: `[pan, tilt, pan_fine, tilt_fine]`.
- Commits end with the session attribution lines (Co-Authored-By + Claude-Session).
- TUI copy is English (matches existing TUI); probe copy is German.

## Review Focus

1. Daslight echoes our own CCs: an echo must not reset the float accumulator, or slow moves stall/jitter. Expect: feedback for a fader is ignored within 200 ms after we sent that fader (same rule as `ECHO_GUARD` in `fader_state.rs`). Test in Task 3.
2. Stick unplugged while trigger held: no runaway movement. Expect: `Connected(false)` clears trigger and axes. Test in Task 3.
3. Router not connected (no loopMIDI / Daslight): stick moves must not change stored values, or the head jumps on reconnect. Expect: `step` skips `tick` when `hub.handles()` is `None`. Test in Task 4.
4. A thread stall gives a huge `dt`: expect `tick` clamps `dt` to 0.1 s. Test in Task 3.
5. `channel` 1..9 collides with APC bank channels in Daslight: expect a `warn!` at startup, not an error. Test in Task 2 (validate returns Ok, warning text pinned via `channel_warning()`).

---

### Task 1: Probe tool + Windows build workflow

**Files:**
- Create: `tools/joystick-probe/Cargo.toml`, `tools/joystick-probe/Cargo.lock` (via `cargo generate-lockfile`), `tools/joystick-probe/src/main.rs`, `tools/joystick-probe/README.md`
- Create: `.github/workflows/joystick-probe.yml`
- Modify: `.gitignore` if `tools/joystick-probe/target` is not ignored

**Interfaces:** none (standalone binary `joystick-probe`).

- [ ] **Step 1: Crate.** Package `joystick-probe`, edition 2024, deps `gilrs = "0.11"`, `hidapi = "2"`. Own `[workspace]` table (empty) so it never joins a parent workspace.
- [ ] **Step 2: `main.rs`.** Log file `joystick-probe-log.txt` in the exe's directory (`std::env::current_exe()?.parent()`), every line prefixed with ms since start and the current step id; every line also printed to the console.
  - Startup: log OS (`std::env::consts::OS`), gilrs gamepad list (name, `os_name`, vendor/product id, `uuid`, `power_info`), hidapi device list (VID, PID, product string, usage page/usage, interface). Choose the hidapi device: first with VID `0x044F` (Thrustmaster), else first with usage page 1 and usage 4 (joystick); log the report descriptor in hex (`HidDevice::get_report_descriptor`).
  - gilrs built with `GilrsBuilder::new().with_default_filters(false)`; log every event with `code.into_u32()`, kind, raw value.
  - hidapi read loop in its own thread (`read_timeout` 50 ms); log each report in hex only when it differs from the previous one.
  - Live line: when a button press is seen, print `Button <raw code> gedrückt` so the tester sees feedback.
  - Steps (exact German copy from the spec), each step ends on Enter (stdin read in a thread, sent over a channel) except step 6, which is a 15 s countdown printed every second:
    1. Geräte suchen (automatic, then "Weiter mit Enter")
    2. "Drücke nacheinander Trigger, Button 2, 3, 4 – dann Enter"
    3. "Drücke die Basis-Buttons 5–16 der Reihe nach – dann Enter"
    4. "Hat: hoch, rechts, runter, links – dann Enter"
    5. "Stick: ganz links, ganz rechts, ganz vor, ganz zurück, dann loslassen – dann Enter"
    6. "Hintergrund-Test: Klick JETZT in ein anderes Fenster (z. B. Editor). Dann Trigger drücken und Stick bewegen. 15 Sekunden…"
    7. "Fertig. Schick die Datei joystick-probe-log.txt zurück." then wait for Enter so the console window stays open.
  - Any gilrs/hidapi init error is logged and the probe continues with the other library.
- [ ] **Step 3: README.md** (German, 10 lines): what it is, start the exe, follow the steps, send back the log.
- [ ] **Step 4: Workflow** `joystick-probe.yml`: `on: push: paths: ['tools/joystick-probe/**', '.github/workflows/joystick-probe.yml']` and `workflow_dispatch`; `permissions: contents: read`; job on `windows-latest`: `cargo build --release --locked --manifest-path tools/joystick-probe/Cargo.toml`, upload artifact `joystick-probe` with `tools/joystick-probe/target/release/joystick-probe.exe` and the README, `if-no-files-found: error`.
- [ ] **Step 5: Verify.** `cargo check --manifest-path tools/joystick-probe/Cargo.toml` (Linux needs libudev headers for hidapi; if missing, note it and rely on CI). Root `cargo test --locked` still 69+ passing.
- [ ] **Step 6: Commit** `feat(joystick-probe): diagnostic tool for TCA sidestick` and stop for the controller: the branch gets pushed (user approved early push) and the CI result checked.

### Task 2: `[joystick]` config

**Files:**
- Create: `src/joystick/mod.rs`, `src/joystick/config.rs`
- Modify: `src/main.rs` (`mod joystick;`), `src/utils/config.rs` (field + validate call), `example.config.toml`, `configs/akai-apc40-mk2/akai-apc40-mk2.config.toml` (commented-out example section)

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
  pub struct JoystickConfig { pub channel: u8, #[serde(default)] pub device: String,
      #[serde(default = "..")] pub speed: f32, #[serde(default = "..")] pub deadzone: f32,
      #[serde(default = "..")] pub curve: f32, pub banks: Vec<BankConfig> }
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
  pub struct BankConfig { pub pan: u8, pub tilt: u8, #[serde(default)] pub pan_fine: Option<u8>, #[serde(default)] pub tilt_fine: Option<u8> }
  impl JoystickConfig {
      pub fn validate(&self) -> anyhow::Result<()>;
      pub fn channel_warning(&self) -> Option<String>;
  }
  // Config: #[serde(default)] pub(crate) joystick: Option<JoystickConfig>
  ```

- [ ] **Step 1: Failing tests** in `config.rs`:
  - `defaults_apply`: `channel = 16` + one bank `pan = 20, tilt = 21` parses to `device == ""`, `speed == 2.0`, `deadzone == 0.1`, `curve == 2.0`, `pan_fine == None`.
  - `validate_rejects`: each of `channel = 0`, `channel = 17`, `pan = 128`, `pan_fine = 200`, `banks = []`, 13 banks, `deadzone = 1.0`, `speed = 0.0`, `curve = 0.0` → `Err` whose message names the field.
  - `channel_warning_for_apc_channels`: channel 5 → `Some(..)` containing `"channel 5"`; channel 16 → `None`.
  - In `utils/config.rs` tests: existing example configs still parse; a config without `[joystick]` gives `joystick == None`.
- [ ] **Step 2: Run** `cargo test joystick::config` → FAIL (module missing).
- [ ] **Step 3: Implement.** Valid: channel 1..=16, all CCs 0..=127, 1..=12 banks, `speed > 0`, `0 <= deadzone < 1`, `curve > 0`. `Config::new` calls `validate()` and logs `channel_warning()` with `warn!`.
- [ ] **Step 4: Run** `cargo test` → all pass.
- [ ] **Step 5: Commit** `feat(joystick): config section`.

### Task 3: `JoystickState` core logic

**Files:**
- Create: `src/joystick/state.rs`

**Interfaces:**
- Consumes: `JoystickConfig`, `BankConfig` (Task 2).
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq)] pub enum Axis { X, Y }
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub enum JoyInput { Button { index: u8, pressed: bool }, // 1-based, manual numbering
                      Hat { x: i8, y: i8 },                // -1/0/1, x: -1 = left
                      Axis { axis: Axis, value: f32 },     // -1..1; X +1 = right, Y +1 = forward
                      Connected(bool) }
  #[derive(Debug, Clone, PartialEq, Serialize)]
  pub struct JoystickSnapshot { pub connected: bool, pub bank: u8 /*1-based*/, pub banks: u8,
                                pub fine: bool, pub values: [Option<u8>; 4] }
  pub struct JoystickState { .. }
  impl JoystickState {
      pub fn new(config: JoystickConfig) -> Self;
      pub fn channel(&self) -> u8;                       // 1..16
      pub fn input(&mut self, input: JoyInput);
      pub fn tick(&mut self, dt: f32, now: Instant) -> Vec<(u8, u8)>; // (cc, value) to send
      pub fn feedback(&mut self, cc: u8, value: u8, now: Instant) -> bool; // true = stored value changed
      pub fn snapshot(&self) -> JoystickSnapshot;
  }
  ```
  `values` is `[pan, tilt, pan_fine, tilt_fine]` of the current bank; `None` where the CC is not configured.

- [ ] **Step 1: Failing tests** (helper `state()` = channel 16, bank 1 `{pan 20, tilt 21, pan_fine 22, tilt_fine 23}`, bank 2 `{pan 30, tilt 31}`, bank 3 `{pan 40, tilt 41}`, defaults):
  - `starts_at_64_on_bank_1`: snapshot `{connected: false, bank: 1, banks: 3, fine: false, values: [Some(64),Some(64),Some(64),Some(64)]}`, bank 2 values `[Some(64),Some(64),None,None]`.
  - `no_move_without_trigger`: X = 1.0, tick 1.0 s → empty, values unchanged.
  - `full_deflection_crosses_range_in_speed`: trigger held, X = 1.0, ten ticks of 0.1 s → pan reaches 127; output contains `(20, 127)`; tilt untouched. X = -1.0 → reaches 0.
  - `y_forward_raises_tilt`: Y = 1.0 → tilt goes up, CC 21.
  - `deadzone_and_curve`: X = 0.1 → no change after 1 s; X = 0.55 → after 1.0 s pan moved `round(63.5 * 0.25)` = 16 ± 1 (speed factor `((0.55-0.1)/0.9)^2 = 0.25`, 63.5 units/s at full deflection = 127/2).
  - `slow_move_accumulates`: X giving factor 0.01, 100 ticks of 0.01 s → pan changed by 1 (no CC lost to rounding).
  - `only_changed_values_are_sent`: two ticks with X = 0 and trigger → both empty.
  - `release_trigger_stops`: after release, ticks send nothing.
  - `fine_toggle_moves_fine_only`: Button 2 press+release → `fine == true`; X = 1.0 tick → CC 22 changes, CC 20 not. Second press → coarse.
  - `fine_without_fine_cc_does_nothing`: bank 2, fine on, X = 1.0 → empty.
  - `fine_is_per_bank`: fine on in bank 1, switch to bank 2 → `fine == false`, back to bank 1 → `true`.
  - `hat_steps_and_wraps`: Hat x = 1 → bank 2; x = 0; x = 1 → 3; x = 0; x = 1 → 1. Hat x = -1 from bank 1 → 3. Holding x = 1 (two `Hat{x:1}` inputs without 0 between) moves one bank only. Hat y ignored.
  - `base_buttons_select_banks`: Button 6 → bank 2; Button 9 (bank 5, not configured) → bank stays; Buttons 3, 4, 17 → nothing.
  - `banks_remember_values`: move pan in bank 1, switch to 2 and back → value kept.
  - `feedback_sets_value`: `feedback(21, 100, now)` → `true`, tilt 100; next move starts from 100. Unknown CC → `false`.
  - `feedback_echo_ignored`: after a tick sent CC 20 at `t`, `feedback(20, v, t + 100ms)` → `false` and accumulator unchanged; at `t + 300ms` → accepted.  *(Review Focus 1)*
  - `disconnect_stops_movement`: trigger + X = 1.0, `Connected(false)`, `Connected(true)`, tick → empty.  *(Review Focus 2)*
  - `dt_is_clamped`: trigger + X = 1.0, `tick(10.0, ..)` → pan = 70 (64 + 0.1 s × 63.5 = 70.35, rounds to 70).  *(Review Focus 4)*
- [ ] **Step 2: Run** `cargo test joystick::state` → FAIL.
- [ ] **Step 3: Implement.** Per bank: `values: [f32; 4]`, `fine: bool`, `last_sent: [Option<Instant>; 4]`. Speed per second at deflection `d`: `0` if `|d| <= deadzone`, else `sign(d) * ((|d|-deadzone)/(1-deadzone))^curve * 127/speed`. Clamp 0..127. Send when `round(new) != round(old)`. Echo guard 200 ms per fader (`const ECHO_GUARD: Duration`).
- [ ] **Step 4: Run** `cargo test joystick::state` → PASS.
- [ ] **Step 5: Commit** `feat(joystick): rate-control state with banks, fine mode, feedback`.

### Task 4: Wiring — thread, source trait, event, Daslight feedback

**Files:**
- Create: `src/joystick/device.rs`, `src/joystick/thread.rs`
- Modify: `src/router/remote.rs` (event variant, hub slot), `src/router/midi_handler.rs` (feedback intercept), `src/utils/threads.rs` (`router_iteration` hands the state to the handler), `src/main.rs` (create state, start thread)

**Interfaces:**
- Consumes: Task 3 API.
- Produces:
  ```rust
  // device.rs
  pub trait JoystickSource { fn poll(&mut self) -> Vec<JoyInput>; }
  pub fn open(device: &str) -> Box<dyn JoystickSource + Send>; // Task 7 fills it; until then yields nothing
  // thread.rs
  pub fn step(state: &Mutex<JoystickState>, source: &mut dyn JoystickSource, hub: &RemoteHub, dt: f32, now: Instant);
  pub fn joystick_thread(exit: Arc<AtomicBool>, hub: Arc<RemoteHub>, state: Arc<Mutex<JoystickState>>, source: Box<dyn JoystickSource + Send>);
  // remote.rs
  RemoteEvent::Joystick(JoystickSnapshot)            // JSON: {"type":"joystick", ...snapshot fields}
  RemoteHub { pub joystick: OnceLock<Arc<Mutex<JoystickState>>>, .. }
  // midi_handler.rs
  pub fn set_joystick(&mut self, state: Arc<Mutex<JoystickState>>);
  ```

- [ ] **Step 1: Failing tests.**
  - `thread.rs`, fake source returning a scripted `Vec<JoyInput>` per poll, hub with attached test handles (pattern from `remote.rs` tests):
    - `step_sends_cc_on_joystick_channel`: trigger + X = 1.0, `step(.., 0.1, ..)` → `to_software.sent` contains `[0xBF, 20, 70]` (CC, channel 16, 64 + 6.35 → 70).
    - `step_broadcasts_snapshot_on_change`: bank switch → `RemoteEvent::Joystick` with `bank: 2` received; a step with no change → no event.
    - `step_without_router_does_not_move`: hub detached, trigger + X = 1.0, step → snapshot values still 64. *(Review Focus 3)* Bank/fine inputs still apply.
  - `midi_handler.rs`: `joystick_feedback_is_taken_and_not_routed`: handler with `set_joystick`, software `ControlChange(Ch16, 21, 100)` → joystick tilt 100, an event `Joystick` with tilt 100 sent, no APC fader event, nothing sent to controller.
  - `remote.rs`: `joystick_event_json`: `serde_json::to_value(RemoteEvent::Joystick(snap))` == `{"type":"joystick","connected":true,"bank":1,"banks":3,"fine":false,"values":[64,64,null,null]}`.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.**
  - `step`: poll → `input` each; if `hub.handles()` is `Some`, `tick` → for each `(cc, v)` send `ControlChange(channel, cc, v)` via `to_software` (drop the joystick lock before locking `to_software`); broadcast when the snapshot differs from the last broadcast (keep last snapshot in a `static`-free way: pass it in, or store it on the state — builder's choice, test above pins behaviour).
  - `joystick_thread`: loop until `exit`, ~10 ms sleep, `dt` from `Instant` deltas.
  - `MidiHandler::process_software_message`: on `ControlChange` whose channel equals the joystick channel, call `feedback`, broadcast on `true`, and return before the APC fader path. Lock order handler → joystick only.
  - `router_iteration`: after `MidiRouter::new`, if `hub.joystick.get()` is `Some`, `router.handles().handler.lock().set_joystick(..)`.
  - `main.rs`: if `config.joystick` is `Some`, create the state, `hub.joystick.set(..)`, `joystick_thread(exit, hub, state, device::open(&cfg.device))`.
- [ ] **Step 4: Run** `cargo test` → all pass.
- [ ] **Step 5: Commit** `feat(joystick): thread, MIDI output and Daslight feedback`.

### Task 5: API endpoint + TUI status line

**Files:**
- Modify: `src/api/routes.rs` (new route), `src/utils/tui.rs` (status line), `src/utils/threads.rs` + `src/main.rs` (pass the state to the TUI)

**Interfaces:**
- Consumes: `RemoteHub::joystick`, `JoystickSnapshot`.
- Produces: `GET /api/joystick` (PIN like `/api/state`) → `200` snapshot JSON, `404` when not configured; works while the router is detached. `pub fn joystick_line(s: &JoystickSnapshot) -> String` in `tui.rs`.

- [ ] **Step 1: Failing tests.**
  - routes: `joystick_without_pin_is_401`, `joystick_not_configured_is_404`, `joystick_returns_snapshot` (hub with state set, detached router → 200, `bank == 1`).
  - tui: `joystick_line` for `{connected: true, bank: 2, banks: 3, fine: true, values: [Some(64), Some(70), Some(12), None]}` == `"Joystick: connected · bank 2/3 · FINE · pan 64 tilt 70 · fine 12 -"`; disconnected + coarse → starts with `"Joystick: not connected · bank 1/3 · coarse"`.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement.** TUI gets `Option<Arc<Mutex<JoystickState>>>`; draws the line under the existing header only when `Some`.
- [ ] **Step 4: Run** `cargo test` → pass.
- [ ] **Step 5: Commit** `feat(joystick): API endpoint and TUI status line`.

### Task 6: Web remote joystick view

**Files:**
- Modify: `tools/web-remote/index.html`, `tools/web-remote/src/main.js`, `tools/web-remote/src/model.js`, `tools/web-remote/src/api.js`, `tools/web-remote/style.css`, `tools/web-remote/test/model.test.js`, `tools/web-remote/README.md`

**Interfaces:**
- Consumes: `GET /api/joystick`, WS `{"type":"joystick", ...}`.
- Produces: `export function dotPos(x, y)` → `{ left: x / 127 * 100, top: (1 - y / 127) * 100 }` in percent, `null` if `x` or `y` is `null`. `api.joystick()`.

- [ ] **Step 1: Failing test** `dotPos`: `(0,0)` → `{left: 0, top: 100}`; `(127,127)` → `{left: 100, top: 0}`; `(64, null)` → `null`.
- [ ] **Step 2: Run** `cd tools/web-remote && npm test` → FAIL.
- [ ] **Step 3: Implement.** A "Joystick" toggle button in `#bar` (hidden until `/api/joystick` returns 200) switches between grid/faders and a `#joystick` section: two squares labelled "Coarse" and "Fine", each with a dot; the active one gets class `active`; a square with `null` pair gets class `unknown` (greyed); "Bank n/N" text. Load via `api.joystick()` on connect and on `resync`; update on WS `joystick`. Read-only: no pointer handlers.
- [ ] **Step 4: Run** `npm test` → pass. Manual check: `cargo run` with the APC sim (see `tools/apc40-sim/README.md`) and a `[joystick]` section; the view shows bank 1/N, both dots centred, "not connected".
- [ ] **Step 5: Commit** `feat(web-remote): read-only joystick view`.

### Task 7: Real joystick backend (blocked on the probe log)

Decided after the friend's `joystick-probe-log.txt` arrives: gilrs (if background input works), else hidapi with the logged report layout, else SDL3. The controller amends this task with the exact mapping (raw codes for buttons 1..16, hat, X/Y range and sign) from the log before dispatching it. Deliverable: `device::open` returns a real source with hotplug (reconnect poll every 1 s for hidapi), emitting `Connected(true/false)`; mapping unit-tested from captured log samples.

### Task 8: Version bump + docs

**Files:**
- Modify: `Cargo.toml` (`version = "0.4.0"`), `Cargo.lock` (via `cargo build`), `README.md` (joystick section: config example, button table), `installer/installer.iss` only if it hardcodes the version.

- [ ] **Step 1:** Bump, `cargo test --locked` passes, `grep -rn "0.3.0"` shows no stale version.
- [ ] **Step 2: Commit** `chore: bump version to 0.4.0`.
