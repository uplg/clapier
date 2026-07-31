# clapier 🐰

The Nabaztag:tag's HTTP burrow - Rust edition 2024, [Axum](https://github.com/tokio-rs/axum).

In 2026 a Nabaztag:tag speaks WPA2/WPA3 thanks to two firmware fixes
([uplg/nabgcc](https://github.com/uplg/nabgcc), branch `wpa23`). All it
still needs is a server handing it its bytecode and resources: that used
to be a `python -m http.server` running in a terminal corner - unstable,
silent, dead at the first crash. The clapier replaces it: a single
binary, logs that tell the rabbit's life, a status page, and a `launchd`
service that survives reboots.

## What it serves

At boot the rabbit contacts the "platform" configured in its bootstrap
bytecode and fetches its application bytecode (`vl/bc.jsp`), its Forth
scripts (`crontab.forth`, `hooks.forth`, ...), its MP3 surprises and its
choreographies (`vl/config/`). The clapier serves that tree in the
rabbit's exact dialect - a 2006 TCP stack inside a VM:

- **`Connection: close` on every response** - the rabbit reads until the
  connection closes;
- **complete bodies with `Content-Length`**, never chunked;
- **query string ignored** for file resolution (`/vl/bc.jsp?sn=...`
  serves `vl/bc.jsp`);
- path traversal refused, encoded or not.

## Architecture

```
crates/
  clapier          the binary: CLI, router, request logging
  clapier-vl       the file service in the rabbit's dialect
  clapier-fleet    the fleet register (one entry per rabbit, learned from the wire)
  clapier-journal  the request journal (bounded ring, thread-safe)
  clapier-pages    the pages for humans (status, listings)
```

## Quick start

```console
$ cargo build --release
$ ./target/release/clapier \
    --bind 0.0.0.0:80 \
    --root /path/to/content \
    --rabbit 192.168.1.155
```

`--rabbit` tags the rabbit's requests with a 🐰 in the logs and on the
status page. (On modern macOS, listening on port 80 needs no special
privileges.)

## Status page

- `http://<server>/_clapier` - uptime, the rabbit's last visit, the
  fleet table, recent requests (refreshes every 5 s);
- `http://<server>/_clapier/health` - `ok`.

The rest of the URL space belongs to the served content.

## Fleet table

One line per rabbit, built from what already travels on the wire -
nothing is asked of the rabbits:

- the `m` query param (the MAC the boot sends on `bc.jsp?...&m=...`)
  binds a rabbit to its IP and dates its last boot fetch;
- the garenne application broadcasts a pulse every 2 s on UDP 9999
  (`garenne 0.8.2 up=42s link=4`); the clapier listens and remembers
  the running version, uptime and link state.

`--pulse-port` moves the UDP listener, `--pulse-port 0` turns it off.

## launchd service (macOS)

The plist [`deploy/fr.uplg.clapier.plist`](deploy/fr.uplg.clapier.plist)
(adjust the paths) gives you a server that starts with the session and
restarts on its own:

```console
$ cp deploy/fr.uplg.clapier.plist ~/Library/LaunchAgents/
$ launchctl bootstrap gui/$UID ~/Library/LaunchAgents/fr.uplg.clapier.plist
$ tail -f ~/Library/Logs/clapier.log
```

To stop it: `launchctl bootout gui/$UID/fr.uplg.clapier`.

## Development

```console
$ cargo test      # including a smoke test that speaks HTTP/1.0 like the rabbit
$ cargo clippy --all-targets -- -D warnings
$ cargo fmt --check
```

## License

MIT. Inspired by the community project ServerlessNabaztag.
