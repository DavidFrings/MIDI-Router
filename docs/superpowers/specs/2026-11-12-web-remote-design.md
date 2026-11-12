# Web Remote: tablet control for the APC40 grid and faders

Date: 2026-11-12. Status: agreed with the user in a grilling session. "Builder's call" marks decisions
the user did not make.

## Intent

During a show the user wants to fire Daslight scenes from a tablet as well as from the Akai APC40 mkII.
The tablet opens a web page served by the router and shows a mirror of the Akai: the 9x5 button grid,
page (bank) buttons and the 9 faders. Pressing a pad on the tablet does exactly what pressing that pad
on the same page of the Akai does. Each tablet chooses its page on its own, independent of the Akai and
of other tablets. Faders move live on the tablet when the Akai, another tablet or Daslight changes them.

Nothing is installed on the tablet. It only opens `http://<PC-IP>/` in a browser.

## Goals

- A web page with 9 page buttons, the 9x5 grid (8x5 clip pads + scene launch column) and 9 faders
  (8 track + master), in Akai layout, landscape only.
- The router is the single source of truth. Tablets are mirrors and input devices.
- Any number of clients at once, all updated live over a WebSocket.
- Independent pages: every tablet has its own page; the Akai keeps its own page. A page change on one
  never changes another.
- Fader values follow Daslight fades (Daslight echoes fader CCs back to the router).
- Soft takeover for the Akai's physical faders.
- PIN protection with per-IP lockout.
- API tests in Rust and UI logic tests in JS.

## Non-goals

- Knobs, transport, track select, crossfader, any other APC40 control.
- Portrait layout.
- HTTPS. Plain HTTP on the LAN is accepted; the PIN travels in clear text.
- Fixing the open items in `docs/router-known-issues.md` (1-6).
- A tablet backup mode without the Akai. If the router is not connected (all four MIDI ports needed),
  the API returns `503`.
- Browser end-to-end tests (no Playwright).

## Anti-goals

- A tablet press that behaves differently from an Akai press (different toggle, mapping or LED result).
- Pad or fader state drifting between tablets, Akai and Daslight. (Pages differ on purpose; the state
  of a given page must look the same everywhere.)
- A tablet page change moving the Akai's page, or an Akai page change moving a tablet's page.
- A missing or bad PIN stopping MIDI routing. The router must keep routing; only the web part is off.
- A fixed PIN shipped in the public repo.

## Current state (facts)

- actix-web 4 server in `api_thread` (`src/utils/threads.rs:55-90`); only `GET /test`. Bind failure
  panics via `.expect`. Logger already excludes `/health`.
- `[api]` config: `enabled`, `bind_address`, `port` (`src/utils/config.rs:25-29`). Example config uses
  `127.0.0.1:8080`, APC40 template `0.0.0.0:8080`.
- `MidiHandler` lives in `Arc<Mutex<_>>` inside `MidiRouter` (`src/router/midi_connection.rs`). The
  router thread rebuilds `MidiRouter` on restart (TUI key `r`), so handler state is recreated.
- Page = bank = MIDI channel, 9 banks (Ch1-Ch9). The Akai changes page by sending CC 16 on a new
  channel (`handle_site_change`, `src/router/midi_handler.rs:57-82`). That CC is also forwarded to
  the software like any CC.
- Grid in the APC40 template: clip pads notes 0-39 (toggle), scene launch notes 82-86 (momentary).
- Daslight LED feedback: NoteOn velocity 1 = on, any other velocity = off with that velocity as colour.
  Colour is unknown until Daslight sends "off" once (known issue 6).
- Faders: Akai sends CC 7 on Ch1-8 and CC 14 (master) on Ch1. `control_map` remaps CC 7 per channel
  to CC `[0,1,2,3,4,5,9,10]`; CC 14 passes unchanged. Output goes to the software on the current bank
  channel. CCs from the software are dropped (`midi_handler.rs:200`).
- `tools/apc40-sim` is a Web MIDI front panel (Chrome/Edge only). It has the APC colour table
  (`src/colors.js`) and panel styling; it cannot run on a tablet because tablets lack Web MIDI.
- TUI keys: `q` quit, `r` restart (`src/utils/tui.rs:94`).
- Installer: Inno Setup, `PrivilegesRequiredOverridesAllowed=dialog` (user may install without admin).
- Stack: Rust 2024, tokio, actix-web, ratatui; tests via `#[test]`.

## Decided by the user

### Serving
- The router serves both the web page and the API from its actix server. One process, one port.
- Default `bind_address = "0.0.0.0"`, `port = 80`, changeable in `config.toml`. Bind failure is logged
  loudly in the TUI; no silent fallback port, no panic.
- Web files live in the repo at `tools/web-remote/` (plain HTML/CSS/JS ES modules, no build step, same
  style as `tools/apc40-sim`; reuse its look and colour table).
- File lookup order: `web-remote/` next to the exe, then `tools/web-remote/` in the working directory.
  No config key. If neither exists: API still runs, `/` shows an error page, TUI shows a hint.
- The installer copies `web-remote/` next to the exe.
- New default values only affect new configs (example config and APC40 template). Existing user
  configs are not touched.

### PIN
- New key `[api] pin` (string). Templates ship `pin = ""`.
- PIN must be digits only, at least 4 digits. Empty or invalid: the web server does not start, MIDI
  routing runs normally, TUI shows "Web remote off: set `pin` in config.toml" (or the reason).
- The user's own installed config gets the user's own PIN; the user adds that line themselves. The value
  never goes into the repo. A missing `pin` key defaults to empty (old configs still parse).
- PIN is required for all `POST` endpoints and `/ws`. Static files and `GET /health` are open.
- The tablet asks for the PIN once and keeps it in `localStorage`.
- 5 wrong attempts from one IP lock that IP for 60 s.

### API
- `GET /health`, `GET /api/state`, `POST /api/press`, `POST /api/fader`, WebSocket `/ws`.
  `GET /test` is removed. (The user first agreed to `POST /api/page` too; it was dropped by the
  assistant once pages became per-tablet, since the server no longer holds a tablet page.)

### Behaviour
- Tablet presses go through the same code path as Akai input: mapping, toggle/momentary logic,
  software output. The target bank is the tablet's page, not the Akai's current bank.
- Pages are independent. Each tablet has its own page (Q34). The Akai's page is changed only by the
  Akai (CC 16 as today) and never by a tablet. A tablet's page is changed only on that tablet.
- A tablet press on bank `b` updates the Akai LEDs only if the Akai currently shows bank `b`
  (same rule as Daslight feedback today, `refresh_led_if_visible`).
- On first open the tablet shows page 1; afterwards the last used page, kept in `localStorage`.
- Fader values are stored per bank x fader. On page change the tablet faders jump to the stored values
  of the new page.
- Daslight echoes the same fader CCs back; the router now accepts them, maps them back to the fader
  index and stores/broadcasts the value.
- Tablet fader: while touched it ignores incoming updates; after release it follows the router again.
- Akai soft takeover: the router keeps each physical fader's last position and the stored value per
  bank. If they differ (after a Daslight fade, a tablet move or a page change), Akai fader moves are
  ignored until the physical fader crosses the stored value or comes within +-2 of it. Per bank.

### UI
- Landscape, Akai layout: row of 9 page buttons (1-9, active one marked) on top, 9x5 grid below,
  9 faders below the grid.
- LEDs look like the Akai: off = colour dimmed, on = blinking in colour, unknown colour = grey.
- Scene launch is momentary: finger down = NoteOn, finger up = NoteOff.
- On WebSocket loss: red "Offline" banner, inputs locked, reconnect every 2 s, full state resync
  after reconnect.

### TUI
- TUI shows the LAN URL (e.g. `http://192.168.1.20/`) permanently.
- Key `w` opens `http://localhost:<port>/` in the default browser.

### Installer
- Adds an inbound Windows Firewall rule for the router exe, profiles private and public. Uninstall
  removes it.

### Tests
- Rust: `actix_web::test` for every endpoint, PIN missing/wrong (401), lockout, WebSocket updates.
  Unit tests for soft takeover, per-bank fader state and the Daslight CC return path.
- JS: `node --test` for UI logic (colours, fader touch lock).

## Builder's calls (made by the assistant, not the user)

- **Shared hub.** One shared object (e.g. `RemoteHub`), created once in `main` and passed to both the
  router thread and the API thread. It owns the broadcast channel to WebSocket clients and the command
  channel from the API into the MIDI handler. It survives router restarts; after a restart the new
  handler pushes a full state so clients resync.
- **Command path.** API handlers do not touch MIDI directly. They send a command (press, release,
  fader, page) to the router, which runs it through `MidiHandler` with the same output connections as
  Akai input. If the router is not connected (no MIDI ports), POSTs return `503`.
- **Input source flag.** Fader input carries its source (Akai or tablet). Soft takeover applies only
  to Akai input. Tablet input sets the stored value and is always sent.
- **Tablet input encoding.** A tablet press on grid cell (column c, row r) is injected as the NoteOn /
  NoteOff the Akai would send for that pad (clip notes 0-39, Ch1; scene launch 82-86), plus an
  explicit target bank. Track faders are injected as CC 7 on Ch(c+1), master as CC 14 on Ch1, so
  `control_map` applies unchanged. `MidiHandler` gets an entry point that takes the target bank
  instead of reading the Akai's current bank; the Akai path calls it with the current bank.
- **No page endpoint.** The tablet's page lives in the browser. Every POST carries `page`.
  `GET /api/state?page=p` returns the snapshot for that page.
- **Daslight CC return.** A CC from the software on channel `b` is matched against the mapped fader
  CCs of the track whose `control_map` entry produces it, plus CC 14 for master; the value is stored
  for bank `b` and broadcast. Unknown CCs stay ignored. Echoes of the router's own output just store
  the same value again.
- **Echo guard.** For 200 ms after the router sends a fader CC to the software, a CC from the
  software for the same bank and fader is ignored. Otherwise a late Daslight echo with an older value
  would overwrite the stored value mid-move and soft takeover would freeze the Akai fader.
- **Pad state scope.** `GET /api/state?page=p` returns `page`, 45 pads `{note, on, color}` (color
  `null` when unknown) and 9 fader values for that page.
- **Wire format.** JSON. The WebSocket only pushes deltas, each tagged with its page:
  `{"type":"pad","page":p,"note":n,"on":b,"color":c}`, `{"type":"fader","page":p,"index":i,"value":v}`,
  and `{"type":"resync"}` after a router restart. Clients drop deltas for other pages. On connect,
  reconnect, page change and `resync` the client fetches `GET /api/state?page=p`.
- **POST bodies.** `/api/press` `{"page":1..9,"note":n,"pressed":bool}` (pressed false only matters
  for momentary notes), `/api/fader` `{"page":1..9,"index":0..8,"value":0..127}`. Out-of-range values
  return `400`.
- **PIN transport.** All `/api/*` requests send header `X-Pin` (including `GET /api/state`, so state
  is not readable without the PIN). WebSocket uses `/ws?pin=...` because browsers cannot set headers
  on WebSockets. Wrong PIN on either counts toward the lockout. Locked IPs get `429`.
- **Fader send rate.** The tablet sends at most one fader POST per fader every 30 ms, always the
  latest value.
- **Blink rate.** The tablet blinks with a fixed CSS animation (about 2 Hz); it has no MIDI clock.
- **Dependencies.** `actix-ws` for the WebSocket and `actix-files` for static files (same ecosystem
  as actix-web). LAN IP found with a std UDP-socket trick; browser opened via `cmd /c start` on
  Windows and `xdg-open` elsewhere. No other new crates.
- **Firewall rule.** `netsh advfirewall firewall add rule ... profile=private,public` in the
  installer's `[Run]`, deleted in `[UninstallRun]`. Only runs for admin installs; non-admin installs
  skip it and the install readme says how to allow the port.

## Left to the builder

- Module layout under `src/api/` and how the hub is wired into `MidiRouter`/`MidiHandler`.
- Exact soft-takeover data structures.
- Exact HTML/CSS, reuse vs copy of `apc40-sim` files (no shared build step either way).
- Lockout store (in-memory map is enough).

## Assumption to verify at the end

- The user cannot capture Daslight's output right now and is fairly sure Daslight echoes exactly the
  CC it received (same CC number, same bank channel). Build the return path on that assumption, as the
  last task, behind the echo guard. Finish with a live test together with the user on Daslight. If
  Daslight sends something else, adjust the mapping and tell the user.
