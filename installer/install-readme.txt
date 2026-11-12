Thank you for installing MIDI Router by David Frings

## Web remote

A tablet or phone on the same network can control the router in a browser.

1. Set a PIN in config.toml under [api]: digits only, at least 4, in quotes, e.g. pin = "4711".
   An empty pin turns the web remote off (MIDI routing still works).
2. Start the router. The TUI shows Web remote: <url>; press w to open it on the PC.
3. On the tablet open http://<PC-IP>/ and enter the PIN.

Notes:

- The web remote uses plain HTTP: the PIN is sent unencrypted on your LAN. Use it only on a network you trust.
- Admin installs add a Windows Firewall rule for the router. After a non-admin install, Windows asks on
  the first start whether to allow the router on the network; choose allow. Otherwise allow the
  port manually in Windows Firewall.
- Default port is 80. If another program (e.g. IIS/HTTP.sys) uses it, change port in config.toml
  (then open http://<PC-IP>:<port>/). The TUI API log shows the bind error.
