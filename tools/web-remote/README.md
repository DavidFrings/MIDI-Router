# Web remote

Tablet control surface for the MIDI Router: the APC40 mkII 9x5 pad grid (clip pads
plus scene launch) and the 9 faders (1-8, M), on 9 pages. Plain HTML/CSS/JS, no build
step. The router serves this folder at `/` when `[api]` is enabled and a `pin` is set.

Open `http://<router-ip>:<port>/` on the tablet in landscape and enter the PIN once
(it is stored in the browser). Optionally "Add to Home Screen" for full screen.

- Pads send press on touch and release on lift, cancel or lost capture; held pads
  are released when the tab is hidden.
- Faders move relative to where you touch them, so they never jump.
- Red banner = offline (no WebSocket, or "Router not connected"); controls are
  disabled and the page retries every 2 s.

Tests (logic in `src/model.js`): `npm test` (Node 20+).
`src/colors.js` is a copy of `tools/apc40-sim/src/colors.js`.
