# MIDI-Router

MIDI-Router is a Rust app that sits between a MIDI controller and lighting/DJ software such as
Daslight. It adds banks (pages) to the controller, remaps notes and CCs per track, turns buttons into
toggles or momentary buttons, and drives the controller LEDs from the software's feedback.

## Features

- **Banks:** Track Select switches between 9 banks; every bank keeps its own button states and LED colours.
- **Remapping:** `note_map` and `control_map` give each track its own note/CC, so the software can tell
  the tracks apart.
- **Toggle and momentary buttons:** toggles stay on until pressed again; momentary buttons send
  NoteOn on press and NoteOff on release.
- **LED feedback:** an active button blinks in its colour, an inactive one shows its colour solid.
- **Auto-update:** checks GitHub Releases at startup and runs the new installer. It never blocks
  startup when offline.
- **Low latency:** written in Rust.
- REST API: planned.

## Installation

Download `midi-router-installer.exe` from the latest
[GitHub release](https://github.com/DavidFrings/MIDI-Router/releases) and run it.

The installer asks which `config.toml` to use: keep the current one (preselected on updates), the
Akai APC40 mkII template (preselected on a fresh install) or import your own file. The router is
ready to start afterwards.

## Requirements

- Windows
- A MIDI controller (template config for the Akai APC40 mkII in `configs/akai-apc40-mk2/`)
- MIDI software, e.g. Daslight
- [loopMIDI](https://www.tobias-erichsen.de/software/loopmidi.html) for the virtual ports to the software

## Configuration

The router reads `config.toml` from its working directory. Start from `example.config.toml` or the
APC40 template.

- `controller_name`: the router uses the first MIDI port whose name contains this text
  (case-insensitive), for both input and output.
- `software_name`: the router talks to the software through two loopMIDI ports,
  `to_<software_name>` and `from_<software_name>`. In the software, use `to_…` as input and
  `from_…` as output.
- `dev = true` (top level, optional): skips the update check.
- `[maps]`: `toggle_notes`, `momentary_notes`, `note_map` and `control_map`. The comments in
  `example.config.toml` explain each one.

Feedback convention (Daslight): the software sends NoteOn vel 1 when a button turns on and NoteOn
vel `<colour>` when it turns off. The colour is an APC40 RGB palette index.

## Web remote

A tablet or phone on the same network can control the router in a browser.

1. Set a PIN in `config.toml` under `[api]`: digits only, at least 4, in quotes, e.g. `pin = "4711"`.
   An empty `pin` turns the web remote off (MIDI routing still works).
2. Start the router. The TUI shows `Web remote: <url>`; press `w` to open it on the PC.
3. On the tablet open `http://<PC-IP>/` and enter the PIN.

Notes:

- The web remote uses plain HTTP: the PIN is sent unencrypted on your LAN. Use it only on a network you trust.
- Admin installs add a Windows Firewall rule for the router. After a non-admin install, Windows asks on
  the first start whether to allow the router on the network; choose allow. Otherwise allow the
  port manually in Windows Firewall.
- Default `port` is 80. If another program (e.g. IIS/HTTP.sys) uses it, change `port` in `config.toml`
  (then open `http://<PC-IP>:<port>/`). The TUI API log shows the bind error.

## Testing without hardware

`tools/apc40-sim/` is a browser-based APC40 mkII simulator that talks to the router over real MIDI.
See [its README](tools/apc40-sim/README.md). Known router issues are listed in
[docs/router-known-issues.md](docs/router-known-issues.md).

## Building from source

Requires Rust 1.87 or newer.

```powershell
git clone https://github.com/DavidFrings/MIDI-Router.git
cd MIDI-Router
copy configs\akai-apc40-mk2\akai-apc40-mk2.config.toml config.toml
# edit config.toml; add "dev = true" at the top to skip the update check
cargo run --release
```

Run the tests with `cargo test`.

To run the built `target\release\midi-router.exe` directly, put `config.toml` next to it and start it
from that folder.

## Releasing

Raise `version` in `Cargo.toml` and merge to `main`. The `Release` workflow
(`.github/workflows/release.yml`) builds on Windows, compiles the Inno Setup installer and publishes
release `v<version>` with `midi-router-installer.exe`, `midi-router.exe` and `example.config.toml`.
If that release already exists, the run does nothing.

To build the installer locally, install Inno Setup 6 and run `installer\gen-installer.cmd`. It builds
the router and writes `installer\midi-router-installer.exe`.

## Roadmap

- [x] Installer
- [x] Configuration file
- [x] Ratatui UI
- [x] Additional controller support
- [ ] REST API for controlling Daslight
- [ ] More controller template configs

## License

Mozilla Public License 2.0 (MPL-2.0), see [LICENSE](LICENSE).

## Author

**David Frings** · dev@davidfrings.dev

Issues and feature requests are welcome.
