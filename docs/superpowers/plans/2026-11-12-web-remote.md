# Web Remote Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A tablet web page, served by the router, that mirrors the APC40 9x5 grid and 9 faders, sends presses and fader moves through the router, and follows Akai/Daslight changes live.

**Architecture:** A shared `RemoteHub` (created in `main`) holds the current router handles (`Arc<Mutex<MidiHandler>>` + output connections) and a `tokio::sync::broadcast` channel of `RemoteEvent`s. API handlers lock the handler and call a new bank-explicit entry point; `MidiHandler` emits pad/fader events at one choke point. Soft takeover and the echo guard live in a pure `FaderState`. The UI is plain ES modules in `tools/web-remote/`, served by actix-files.

**Tech Stack:** Rust 2024 (actix-web 4, actix-ws, actix-files, tokio broadcast), plain HTML/CSS/JS, `node --test`.

**Spec:** `docs/superpowers/specs/2026-11-12-web-remote-design.md`

## Global Constraints

- Default `[api]`: `enabled = true`, `bind_address = "0.0.0.0"`, `port = 80`, `pin = ""`. Only templates change; never touch a user's `config.toml`. Never write the user's real PIN into the repo.
- PIN: digits only, at least 4. Empty/invalid: web server not started, MIDI routing unaffected, warn on target `api`: `Web remote off: set pin in config.toml (digits only, at least 4)`.
- PIN required on every `/api/*` route (header `X-Pin`) and `/ws` (`?pin=`). Open: static files, `GET /health`.
- 5 wrong PINs from one IP: that IP gets `429` for 60 s.
- Router not attached (MIDI not connected): every `/api/*` route returns `503`.
- Pages are per tablet. Nothing a tablet does changes `StateManager::current_bank`. Akai LEDs change only for the bank the Akai shows.
- Soft takeover: pick-up window ±2, or crossing. Echo guard 200 ms.
- Fader send throttle in the UI: at most one POST per fader per 30 ms, latest value wins. Reconnect every 2 s. Blink ≈2 Hz CSS.
- New runtime crates only `actix-ws` and `actix-files`; tokio gains feature `sync`. Dev-only: `actix-test`, `awc`.
- Lock order everywhere: handler, then to_controller, then to_software (matches `src/router/input_connection.rs`).
- Bind failure must log `error!(target: "api", …)` and return; no panic, no fallback port.
- Out of scope: known issues 1-6, knobs/transport/crossfader, portrait, HTTPS, tablet backup mode without the Akai.

## Review Focus

1. `pin = 4711` written unquoted in TOML must parse (as `"4711"`), not abort startup. Test in Task 4.
2. API thread and MIDI callbacks locking in different orders deadlock the router. Every API path locks handler → controller → software. Reviewer checks Task 5 code; test `press_does_not_deadlock_with_concurrent_software_feedback` in Task 5.
3. Router restart while tablets are connected: API returns `503` while detached, then clients receive `{"type":"resync"}`. Test in Task 5.
4. A scene-launch finger lifted outside the pad or cancelled by the OS must still send `pressed:false`; otherwise Daslight sees a stuck note. UI listens to `pointerup`, `pointercancel` and `lostpointercapture` (Task 7; reviewer checks).
5. Port 80 already in use (IIS/HTTP.sys): router keeps routing, TUI shows the bind error. Test `start_server_returns_err_when_port_taken` in Task 5.

---

### Task 1: FaderState (soft takeover + echo guard)

**Files:**
- Create: `src/router/fader_state.rs`
- Modify: `src/router/mod.rs` (add `pub mod fader_state;`)

**Interfaces:**
- Produces:
  - `pub const FADER_COUNT: usize = 9; pub const PICKUP_WINDOW: u8 = 2; pub const ECHO_GUARD: Duration = Duration::from_millis(200);`
  - `pub struct FaderState` (`Default`), banks indexed `0..9` = `Channel::index()`.
  - `pub fn controller_move(&mut self, bank: usize, index: usize, value: u8, now: Instant) -> bool` — true = accept and send.
  - `pub fn remote_set(&mut self, bank: usize, index: usize, value: u8, now: Instant)`
  - `pub fn software_set(&mut self, bank: usize, index: usize, value: u8, now: Instant) -> bool` — false = ignored by echo guard.
  - `pub fn value(&self, bank: usize, index: usize) -> Option<u8>`
  - `pub fn controller_fader_index(channel: Channel, cc: u8) -> Option<usize>` — CC 7 on Ch1-Ch8 → 0..7, CC 14 on any channel → 8, else `None`.

- [ ] **Step 1: Write failing tests** in `#[cfg(test)] mod tests`:
  - `first_controller_move_is_accepted` (no stored value → true, `value == Some(v)`)
  - `continuous_controller_moves_stay_accepted` (60, 61, 62 all true)
  - `controller_ignored_after_remote_change_until_pickup` (stored 100 via `remote_set`, physical at 20 → move to 50 false, `value == Some(100)`; move to 98 true)
  - `controller_picks_up_when_crossing` (stored 100, physical 90 → jump to 110 true)
  - `page_change_requires_pickup` (move bank 0 to 20; bank 1 stored 80 via `remote_set`; move on bank 1 to 25 false)
  - `software_echo_within_200ms_is_ignored` (`remote_set` at t, `software_set` at t+100ms false, value unchanged)
  - `software_after_200ms_is_applied` (t+250ms true, value updated)
  - `controller_fader_index_maps_apc_faders` (Ch3/CC7 → 2, Ch1/CC14 → 8, Ch1/CC16 → None, Ch9/CC7 → None)

- [ ] **Step 2: Run** `cargo test fader_state` — expect compile failure / FAIL.

- [ ] **Step 3: Implement.** State: `values: [[Option<u8>; 9]; 9]`, `physical: [Option<u8>; 9]` (one physical fader per index, shared by all banks), `last_sent: [[Option<Instant>; 9]; 9]`. `controller_move` rule:

```rust
let prev = self.physical[index];
self.physical[index] = Some(value);
let accept = match self.values[bank][index] {
    None => true,
    Some(stored) => prev == Some(stored)                          // already in sync
        || value.abs_diff(stored) <= PICKUP_WINDOW
        || prev.is_some_and(|p| (p < stored) != (value < stored)), // crossed
};
if accept { self.values[bank][index] = Some(value); self.last_sent[bank][index] = Some(now); }
accept
```

`remote_set` stores value and `last_sent = now`. `software_set` returns false if `last_sent` is within `ECHO_GUARD` of `now`, else stores.

- [ ] **Step 4: Run** `cargo test fader_state` — PASS.
- [ ] **Step 5: Commit** `feat(router): fader state with soft takeover and echo guard`

---

### Task 2: Remote types, hub, and bank-explicit pad input

**Files:**
- Create: `src/router/remote.rs`
- Modify: `src/router/mod.rs`, `src/router/midi_handler.rs`, `src/router/midi_connection.rs`, `Cargo.toml` (tokio feature `sync`)

**Interfaces:**
- Produces (in `remote.rs`):
  - `#[derive(Serialize, Clone, Debug, PartialEq)] #[serde(tag = "type", rename_all = "lowercase")] pub enum RemoteEvent { Pad { page: u8, note: u8, on: bool, color: Option<u8> }, Fader { page: u8, index: usize, value: u8 }, Resync }`
  - `pub enum RemoteInput { Press { note: u8, pressed: bool }, Fader { index: usize, value: u8 } }`
  - `#[derive(Serialize)] pub struct PadState { pub note: u8, pub on: bool, pub color: Option<u8> }`
  - `#[derive(Serialize)] pub struct PageState { pub page: u8, pub pads: Vec<PadState>, pub faders: [Option<u8>; 9] }`
  - `pub const GRID_NOTES: [u8; 45]` = 0..=39 then 82..=86. (`shortcut: APC40 mkII layout only, make it config if another controller needs the remote`.)
  - `#[derive(Clone)] pub struct RouterHandles { pub handler: Arc<Mutex<MidiHandler>>, pub to_controller: Arc<Mutex<OutputConnection>>, pub to_software: Arc<Mutex<OutputConnection>> }`
  - `pub struct RemoteHub { router: Mutex<Option<RouterHandles>>, pub events: broadcast::Sender<RemoteEvent> }` with `new() -> Arc<Self>` (capacity 256), `attach(&self, RouterHandles)` (stores, sends `Resync`), `detach(&self)`, `handles(&self) -> Option<RouterHandles>`.
  - Page ↔ bank: `page = bank.index() + 1`; `Channel::from_index(page - 1)`.
- Produces (in `MidiHandler`):
  - `pub fn set_event_sender(&mut self, tx: broadcast::Sender<RemoteEvent>)`
  - `pub fn handle_remote_input(&mut self, input: RemoteInput, bank: Channel, to_controller: &mut OutputConnection, to_software: &mut OutputConnection) -> Result<()>`
  - `pub fn snapshot(&mut self, bank: Channel) -> PageState` — pads in `GRID_NOTES` order, `color: None` when stored colour is 0, faders from `FaderState` (all `None` until Task 3 wires it).
- Produces (in `MidiRouter`): `MidiRouter::new(config, events: broadcast::Sender<RemoteEvent>)`, `pub fn handles(&self) -> RouterHandles`.

- [ ] **Step 1: Write failing tests** in `midi_handler.rs` tests:
  - `remote_press_on_other_bank_sends_on_that_channel_and_leaves_akai_leds` (Akai on Ch1, remote press note 5 bank Ch3 → software got `NoteOn(Ch3, 5, 127)`, controller `sent` empty, `current_bank` still Ch1)
  - `remote_press_on_visible_bank_matches_akai_press` (same note via controller msg on one handler and via remote on another → identical software and controller `sent`)
  - `pad_change_emits_event` (subscribe receiver; remote press → `RemoteEvent::Pad { page: 3, note: 5, on: true, color: None }`)
  - `software_feedback_emits_pad_event_with_colour` (software NoteOn Ch2 note 5 vel 21 → `Pad { page: 2, note: 5, on: false, color: Some(21) }`)
  - `snapshot_has_45_pads_and_9_faders`
  - in `remote.rs`: `attach_sends_resync`, `detach_clears_handles`

- [ ] **Step 2: Run** `cargo test` — FAIL.

- [ ] **Step 3: Implement.** Thread an explicit `bank: Channel` through `process_controller_message` (Akai path passes `current_bank`). Replace every pad LED refresh in the toggle, momentary and software paths with one `fn pad_changed(&mut self, ctrl, bank: &Channel, note: Note) -> Result<()>` that refreshes the Akai LED only when `bank` is the current bank (existing `refresh_led_if_visible` logic) and sends `RemoteEvent::Pad` (ignore `send` errors: no subscribers is normal). Keep the Akai `NoteOff`-on-toggle LED restore. `handle_remote_input` turns `Press` into `NoteOn(Ch1, note, 127)` / `NoteOff(Ch1, note, 0)` and runs the same path; it never calls `handle_site_change`. `Fader` returns `Ok(())` for now.

- [ ] **Step 4: Run** `cargo test` — PASS (existing tests too).
- [ ] **Step 5: Commit** `feat(router): remote hub and bank-explicit pad input`

---

### Task 3: Faders through the handler

**Files:**
- Modify: `src/router/midi_handler.rs`

**Interfaces:**
- Consumes: `FaderState`, `controller_fader_index` (Task 1); `RemoteInput::Fader`, `RemoteEvent::Fader` (Task 2).
- Produces: `snapshot` fills `faders` from `FaderState::value`.

- [ ] **Step 1: Write failing tests:**
  - `akai_fader_is_remapped_and_emits_event` (Ch2 CC7 val 64 on Akai bank Ch1 → software `ControlChange(Ch1, 1, 64)`, event `Fader { page: 1, index: 1, value: 64 }`)
  - `akai_fader_blocked_after_remote_move` (remote fader index 0 → 100 on bank Ch1; Akai Ch1 CC7 val 20 → nothing sent to software)
  - `remote_fader_sends_on_page_channel` (remote index 8 value 90 bank Ch4 → software `ControlChange(Ch4, 14, 90)`)
  - `remote_fader_rejects_index_9` (returns `Err`)
  - `non_fader_cc_passes_unchanged` (Ch1 CC16 still forwarded as today)

- [ ] **Step 2: Run** `cargo test midi_handler` — FAIL.

- [ ] **Step 3: Implement.** In the `ControlChange` branch: if `controller_fader_index` matches, Akai input goes through `controller_move` (drop when false), remote input through `remote_set`; on send emit `RemoteEvent::Fader`. `RemoteInput::Fader { index, value }` becomes `ControlChange(Ch(index+1), 7, value)` for 0..7 and `ControlChange(Ch1, 14, value)` for 8, so `control_map` applies unchanged. Use `Instant::now()` here; tests only need ordering, not timing.

- [ ] **Step 4: Run** `cargo test` — PASS.
- [ ] **Step 5: Commit** `feat(router): faders with soft takeover for Akai and remote input`

---

### Task 4: Config PIN and auth primitives

**Files:**
- Modify: `src/utils/config.rs`
- Create: `src/api/auth.rs`

**Interfaces:**
- Produces:
  - `ApiConfig.pin: String` with `#[serde(default, deserialize_with = "string_or_int")]`.
  - `pub fn valid_pin(pin: &str) -> bool` (digits only, len ≥ 4).
  - `pub struct Lockout` (`Default`) with `pub fn check(&self, ip: IpAddr, now: Instant) -> bool` (true = allowed), `pub fn fail(&self, ip: IpAddr, now: Instant)`, `pub fn success(&self, ip: IpAddr)`. Constants `MAX_FAILS: u32 = 5`, `LOCK_TIME: Duration = Duration::from_secs(60)`. Internal `Mutex<HashMap<IpAddr, (u32, Option<Instant>)>>`; counter resets when the lock expires.

- [ ] **Step 1: Write failing tests:**
  - `pin_missing_defaults_empty` (old `[api]` without `pin` parses)
  - `pin_unquoted_integer_parses` (`pin = 4711` → `"4711"`)
  - `valid_pin_rules` (`"1234"` ok, `"0123"` ok, `"123"` no, `"12a4"` no, `""` no)
  - `five_fails_lock_for_60s` (5×`fail` → `check` false at +1 s, true at +61 s)
  - `success_resets_counter` (4 fails, success, 4 fails → still allowed)
  - `lock_is_per_ip`

- [ ] **Step 2: Run** `cargo test auth config` — FAIL.
- [ ] **Step 3: Implement** as specified.
- [ ] **Step 4: Run** — PASS.
- [ ] **Step 5: Commit** `feat(api): pin config and per-IP lockout`

---

### Task 5: HTTP API, static files, server wiring

**Files:**
- Create: `src/api/routes.rs`, `src/api/server.rs`
- Delete: `src/api/test.rs`
- Modify: `src/api/mod.rs`, `src/utils/threads.rs`, `src/main.rs`, `Cargo.toml` (`actix-files`, dev `actix-test`, `awc`)

**Interfaces:**
- Consumes: `RemoteHub`, `RemoteInput`, `PageState` (Task 2), `Lockout`, `valid_pin` (Task 4).
- Produces:
  - `pub struct ApiState { pub hub: Arc<RemoteHub>, pub pin: String, pub lockout: Lockout }`
  - `pub fn authorize(req: &HttpRequest, state: &ApiState, given: Option<&str>) -> Result<(), HttpResponse>` — 429 if locked, 401 + `fail` if wrong/missing, `success` otherwise. Missing peer addr → `0.0.0.0`.
  - `pub fn configure(state: web::Data<ApiState>, web_root: Option<PathBuf>) -> impl FnOnce(&mut web::ServiceConfig)` registering `GET /health`, `GET /api/state?page=`, `POST /api/press` `{page, note, pressed}`, `POST /api/fader` `{page, index, value}`, and static files at `/` (`index.html`), or a `500` HTML error page "web-remote files not found" when `web_root` is `None`. (`/ws` added in Task 6.)
  - `pub fn find_web_root(exe_dir: Option<&Path>, cwd: &Path) -> Option<PathBuf>` — `exe_dir/web-remote`, then `cwd/tools/web-remote`, must contain `index.html`.
  - `pub async fn start_server(config: ApiConfig, hub: Arc<RemoteHub>) -> anyhow::Result<actix_web::dev::Server>` — bind errors returned, not panicked.
  - `api_thread(exit, config, hub)`; `router_thread(..., hub)` calls `hub.attach(router.handles())` after a successful connect and `hub.detach()` before rebuilding. `init_threads` skips the API with the warning from Global Constraints when `!enabled` or `!valid_pin`.
- Validation: `page` 1..=9, `note` in `GRID_NOTES`, `index` 0..=8, `value` 0..=127, else `400`. Handlers lock in the global lock order, call `handle_remote_input` / `snapshot`, map errors to `500`.

- [ ] **Step 1: Write failing tests** (`actix_web::test`, handler attached with unconnected `OutputConnection`s):
  - `health_is_open`
  - `state_without_pin_is_401`, `state_with_pin_returns_page` (page 3, 45 pads, 9 faders)
  - `press_sends_to_software_on_page_channel` (inspect `to_software.sent`)
  - `bad_input_is_400` (page 0, page 10, note 50, index 9, value 128)
  - `sixth_attempt_after_five_wrong_pins_is_429` (6th uses correct PIN)
  - `detached_router_is_503`
  - `static_index_served_when_root_exists` (tempdir with `index.html`), `missing_root_shows_error_page`
  - `find_web_root_prefers_exe_dir`
  - `start_server_returns_err_when_port_taken` (bind a `std::net::TcpListener` first)
  - `press_does_not_deadlock_with_concurrent_software_feedback` (thread A loops `handle_software_msg` with the same lock order as `input_connection.rs`; 200 POSTs complete within 5 s)

- [ ] **Step 2: Run** `cargo test api` — FAIL.
- [ ] **Step 3: Implement.** Remove `GET /test`. Keep `Logger::default().exclude("/health")`.
- [ ] **Step 4: Run** `cargo test` and `cargo build` — PASS.
- [ ] **Step 5: Commit** `feat(api): remote state, press and fader endpoints with pin`

---

### Task 6: WebSocket `/ws`

**Files:**
- Create: `src/api/ws.rs`
- Modify: `src/api/server.rs` (route), `Cargo.toml` (`actix-ws`)

**Interfaces:**
- Consumes: `ApiState`, `authorize` (Task 5), `RemoteHub::events` (Task 2).
- Produces: `GET /ws?pin=` upgrade. Forwards every `RemoteEvent` as JSON text. On `RecvError::Lagged` sends `{"type":"resync"}`. Answers ping, ends on close or when the client stream ends.

- [ ] **Step 1: Write failing tests** with `actix_test::start` + `awc` ws client:
  - `ws_wrong_pin_is_rejected` (401 on handshake)
  - `ws_receives_pad_event_after_press` (connect, POST press, next text frame parses to `{"type":"pad","page":1,…}`)
  - `ws_receives_resync_on_attach`

- [ ] **Step 2: Run** `cargo test ws` — FAIL.
- [ ] **Step 3: Implement** with `actix_ws::handle`, spawn one task per session, `tokio::select!` on broadcast receiver and message stream.
- [ ] **Step 4: Run** `cargo test` — PASS.
- [ ] **Step 5: Commit** `feat(api): websocket live updates`

---

### Task 7: Tablet UI

**Files:**
- Create: `tools/web-remote/index.html`, `style.css`, `package.json` (`{"type":"module","private":true,"scripts":{"test":"node --test"}}`), `README.md`, `src/colors.js` (copy of `tools/apc40-sim/src/colors.js`, header comment says so), `src/model.js`, `src/api.js`, `src/main.js`, `test/model.test.js`

**Interfaces:**
- Consumes: HTTP/WS contract from Tasks 5-6.
- Produces (`model.js`):
  - `export const GRID` — 5 rows × 9 notes, row `r` = `[(4-r)*8 + 0 … (4-r)*8 + 7, 82 + r]` (top row 32-39 + 82).
  - `export function padLook(pad)` → `{ color: string, blink: boolean }`: `color === null` → `'#555'`, no blink; off → `RGB[color]` dimmed to 35 % brightness; on → `RGB[color]`, `blink: true`.
  - `export function createFaderLock()` → `{ touch(i), release(i), accepts(i) }`.
  - `export function throttleLatest(send, ms = 30)` → `(key, value) => void`, at most one `send(key, value)` per key per `ms`, trailing call sends the latest value.
  - `export function loadPage(storage)` / `savePage(storage, page)` — invalid, missing or throwing storage → 1.

- [ ] **Step 1: Write failing tests** in `test/model.test.js`:
  - `GRID top row is 32-39 plus scene 82`, `GRID bottom row is 0-7 plus scene 86`
  - `unknown colour is grey and steady`, `off is dimmed`, `on blinks in full colour`
  - `fader lock ignores updates while touched and accepts after release`
  - `throttleLatest sends first value, then only latest within 30 ms` (use `mock.timers`)
  - `loadPage falls back to 1` (missing, `"0"`, `"10"`, `"abc"`, throwing storage)

- [ ] **Step 2: Run** `cd tools/web-remote && npm test` — FAIL.
- [ ] **Step 3: Implement.** Layout: landscape, top bar with 9 page buttons (active marked) and an offline banner; 9×5 pad grid; 9 vertical faders below (labels 1-8, M). Look taken from `tools/apc40-sim/style.css`. PIN overlay asks once, stores in `localStorage`, reopens on `401`; `429` shows "Locked, try again in 60 s". `api.js` sends `X-Pin`; WS reconnects every 2 s; on open, reconnect, page change and `resync` it fetches `/api/state?page=`. Deltas for other pages dropped. While offline: red "Offline" banner, inputs disabled. Pads send `pressed:true` on `pointerdown` and `pressed:false` on `pointerup`, `pointercancel` and `lostpointercapture` (use `setPointerCapture`). Fader updates from server ignored while `accepts(i)` is false. Blink: CSS animation, period 0.5 s.
- [ ] **Step 4: Run** `npm test` — PASS. Then `cargo run` with a config that has a PIN, open `http://localhost/` in a browser, check grid, page switch and offline banner (stop the router).
- [ ] **Step 5: Commit** `feat(web-remote): tablet UI`

---

### Task 8: TUI URL and `w` key

**Files:**
- Modify: `src/utils/helper.rs`, `src/utils/tui.rs`, `src/utils/threads.rs`, `src/main.rs`

**Interfaces:**
- Produces:
  - `pub fn web_url(host: &str, port: u16) -> String` — `http://host/` for port 80, else `http://host:port/`.
  - `pub fn lan_host(bind_address: &str) -> Option<String>` — for `0.0.0.0` the address of a UDP socket "connected" to `8.8.8.8:80` (no packet sent); otherwise `bind_address`.
  - `pub fn open_in_browser(url: &str)` — `cmd /c start "" <url>` on Windows, `xdg-open` elsewhere; log errors, never panic.
  - `App::new(..., web_url: Option<String>, local_url: Option<String>)`; the info area shows `Web remote: <url>` or `Web remote: off`; key `w` opens `local_url`; bottom bar adds `Open web remote <W>`.

- [ ] **Step 1: Write failing tests:** `web_url_omits_port_80`, `web_url_keeps_other_port`, `lan_host_keeps_explicit_bind`.
- [ ] **Step 2: Run** `cargo test helper` — FAIL.
- [ ] **Step 3: Implement.** `None` URLs when the web remote is off.
- [ ] **Step 4: Run** `cargo test` — PASS; `cargo run`, press `w`, browser opens.
- [ ] **Step 5: Commit** `feat(tui): show web remote URL, open with w`

---

### Task 9: Templates, installer, docs

**Files:**
- Modify: `example.config.toml`, `configs/akai-apc40-mk2/akai-apc40-mk2.config.toml`, `installer/installer.iss`, `installer/install-readme.txt`, `README.md`

- [ ] **Step 1:** Both templates: `bind_address = "0.0.0.0"`, `port = 80`, `pin = ""` with comment `# Web remote PIN: digits only, at least 4. Empty = web remote off.`
- [ ] **Step 2:** Installer `[Files]`: `Source: "{#ProjectDir}\tools\web-remote\*"; DestDir: "{app}\web-remote"; Excludes: "test,package.json,README.md"; Flags: ignoreversion recursesubdirs`. `[Run]`: `netsh advfirewall firewall add rule name="MIDI Router" dir=in action=allow program="{app}\{#MyAppExeName}" profile=private,public` with `Flags: runhidden; Check: IsAdminInstallMode`. `[UninstallRun]`: `netsh advfirewall firewall delete rule name="MIDI Router"` with `RunOnceId: "DelFirewallRule"; Flags: runhidden`.
- [ ] **Step 3:** `install-readme.txt` and `README.md`: web remote section (set `pin`, open `http://<PC-IP>/`, `w` key, firewall note for non-admin installs, PIN is sent unencrypted on the LAN).
- [ ] **Step 4:** Verify: `cargo test`; templates parse (`config_templates_parse` test in `config.rs` that `toml::from_str`s both files via `include_str!`). Installer compile is checked by CI on Windows.
- [ ] **Step 5: Commit** `feat(installer): ship web remote, firewall rule, port 80 defaults`

---

### Task 10: Daslight fader return path (last)

**Files:**
- Modify: `src/router/mapping_config.rs`, `src/router/midi_handler.rs`

**Interfaces:**
- Consumes: `FaderState::software_set` (Task 1), `RemoteEvent::Fader` (Task 2).
- Produces: `MappingConfig::software_fader_index(&self, cc: u8) -> Option<usize>` — first `i` in 0..8 where `remap_control(Ch(i+1), 7) == cc`, else `cc == 14` → 8.

- [ ] **Step 1: Write failing tests:**
  - `software_fader_index_reverses_control_map` (APC map: 9 → 6, 0 → 0, 14 → 8, 20 → None)
  - `daslight_fader_cc_updates_state_and_emits` (software `ControlChange(Ch2, 3, 77)` → event `Fader { page: 2, index: 3, value: 77 }`, snapshot bank Ch2 shows 77)
  - `daslight_echo_right_after_send_is_ignored`
  - `daslight_change_makes_akai_fader_need_pickup`

- [ ] **Step 2: Run** `cargo test` — FAIL.
- [ ] **Step 3: Implement** in the `ControlChange` arm of `process_software_message` (today `{}`). Nothing goes to the Akai.
- [ ] **Step 4: Run** `cargo test` — PASS.
- [ ] **Step 5: Commit** `feat(router): follow Daslight fader feedback`
- [ ] **Step 6: Checkpoint with the user.** Live test on Windows with Daslight: move a fader in Daslight, tablet follows; move the Akai fader, no jump, pickup works. If Daslight sends other CCs/channels, stop and report.
