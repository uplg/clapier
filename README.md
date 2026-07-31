# clapier 🐰

The Nabaztag:tag's HTTP burrow - Rust edition 2024, [Axum](https://github.com/tokio-rs/axum).

In 2026 a Nabaztag:tag speaks WPA2/WPA3 and hears broadcast again
thanks to four firmware fixes ([uplg/nabgcc](https://github.com/uplg/nabgcc),
branch `wpa23-gtk`, proposed upstream): a PMK truncated by `strcpy`, a
wrong cipher suite in the association request, `.bss.*` sections never
zeroed at boot, and a group key that was never installed. All it still
needs is a server handing it its bytecode and resources: that used to
be a `python -m http.server` running in a terminal corner - unstable,
silent, dead at the first crash. The clapier replaces it: a single
binary, logs that tell the rabbit's life, a status page, and a `launchd`
service that survives reboots.

## What it serves

At boot the rabbit contacts the "platform" configured in its bootstrap
bytecode and fetches its application bytecode (`vl/bc.jsp`), then
whatever that application asks for: MP3s, spoken sentences (`vl/say/`),
binary choreographies in the original Violet format (`vl/config/chor/`,
`vl/chor/`). The clapier serves all of it in the rabbit's exact
dialect - a 2006 TCP stack inside a VM:

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
garenne/           the rabbit's embedded application (Metal bytecode,
                   our own IP/TCP/HTTP stack, served as vl/bc.jsp)
scripts/           deploy, remote control, log listener, rabbit-say
vendor/
  metal            the Metal toolchain (Sylvain Huet's mtl compiler and
                   simulator), built inside the mtl-dev Docker image
  pocket-tts       Kyutai Pocket TTS, native Rust port (candle), used
                   by rabbit-say for the rabbit's speaking voice
```

## garenne

The application the rabbit actually runs: cooperative scheduler, LLC/
SNAP framing, ARP, ICMP, UDP, TCP client and server, HTTP on both
sides, streamed MP3 to the VS1003, ears, button, watchdog. It is
rewritten from scratch (the VM natives are the only contract) and
tested by golden frames against independent Python vectors:

```console
$ ./garenne/build.sh test    # the golden suite in the simulator
$ ./garenne/build.sh         # device build -> garenne/build/garenne.bin
$ ./scripts/deploy-garenne.sh --rabbit 00:19:db:9c:28:15
```

A deploy lands in `garenne/overlay/rabbits/<mac>/vl/bc.jsp`; the rabbit
refetches its bytecode at every boot, so the worst a bad build costs is
a power cycle, never a brick.

## rabbit-say

Text to speech through the rabbit: Kyutai Pocket TTS (French, the
Estelle voice) rendered as the mono 32 kHz MP3 shape the VS1003 has
chewed since 2006, installed into the overlay and streamed on the spot:

```console
$ cd vendor/pocket-tts && cargo build --release -p pocket-tts-cli \
    --no-default-features --features metal && cd ../..
$ ./scripts/rabbit-say.sh "Bonjour."
```

## The release route

Every tagged release ships ready-to-run archives, one per platform,
no Rust toolchain required:

| file | for |
| --- | --- |
| `clapier-vX.Y.Z-linux-x86_64.tar.gz` | any x86_64 Linux box or NAS (static musl build) |
| `clapier-vX.Y.Z-linux-aarch64.tar.gz` | Raspberry Pi 4/5 and other 64-bit ARM boards (static musl build) |
| `clapier-vX.Y.Z-macos-aarch64.tar.gz` | Apple Silicon Macs (the launchd plist rides along) |
| `clapier-vX.Y.Z-windows-x86_64.zip` | Windows |
| `garenne-vX.Y.Z.bin` | the rabbit's application bytecode, built and golden-tested in CI |
| `Nab-wpa23-gtk-*.sim` | the WPA2/WPA3 firmware, hardware-proven, mirrored from the latest [uplg/nabgcc](https://github.com/uplg/nabgcc/releases) release |
| `flash-nabaztag.py` | the firmware flasher (Python, stdlib only), rides along with the firmware |
| `SHA256SUMS` | checksums for all of the above |

Each archive holds the `clapier` server, the `chor-encode` choreography
encoder, and this README.

### Adopting a Nabaztag:tag in 2026

Still have a rabbit in a cupboard? Here is the whole journey:

1. **Firmware.** A stock Nabaztag:tag only speaks WEP/WPA1. Flash the
   `wpa23-gtk` build once and it joins WPA2/WPA3 networks and hears
   broadcasts again. The `.sim` and its flasher come with every
   release: hold the head button while powering on, join the
   `NabaztagXX` access point the rabbit opens, then
   `python3 flash-nabaztag.py Nab-wpa23-gtk-*.sim`. The bootloader is
   never touched, so a bad flash is always recoverable from the same
   menu.
2. **Server.** Download the archive for your machine, unpack, and run
   `./clapier --bind 0.0.0.0:80 --overlay overlay`. Any always-on box
   on your LAN will do - a Pi is plenty. On Linux, port 80 wants
   either root or a one-time
   `sudo setcap 'cap_net_bind_service=+ep' ./clapier`.
3. **Point the rabbit at it.** In the rabbit's setup portal (hold the
   head button while powering on), configure your WiFi and give your
   server's IP as the platform address. Everything else the rabbit
   ever fetches now comes from your clapier.
4. **Give it a brain.** Drop the release's `garenne-vX.Y.Z.bin` into
   `overlay/rabbits/<mac>/vl/bc.jsp` (lowercase MAC, no colons) and
   power-cycle: the rabbit boots the garenne application - ears,
   button, choreographies, streamed MP3, a heartbeat on UDP 9999.
5. **Open the burrow.** `http://<server>/_clapier` shows the fleet at
   a glance; `/_clapier/pilot` drives it from the browser: ping,
   salute, choreographies composed on the page, and a sentence box
   once `--say-script` points at a speech pipeline (see rabbit-say).

The worst a bad bytecode costs is a power cycle - the rabbit refetches
`bc.jsp` at every boot, so it never bricks.

## Quick start (from source)

```console
$ cargo build --release
$ ./target/release/clapier \
    --bind 0.0.0.0:80 \
    --overlay garenne/overlay \
    --rabbit 192.168.1.155
```

The overlay is all a garenne rabbit needs (its bytecode, its voice, its
sounds). `--root` optionally mounts a legacy base tree behind it, for
serving the original Violet ecosystem to a stock bytecode; whatever the
overlay does not hold is looked up there, or 404s without it.

`--rabbit` tags the rabbit's requests with a 🐰 in the logs and on the
status page. (On modern macOS, listening on port 80 needs no special
privileges.)

## Status page

- `http://<server>/_clapier` - uptime, the rabbit's last visit, the
  fleet table, recent requests (refreshes every 5 s);
- `http://<server>/_clapier/health` - `ok`.

The rest of the URL space belongs to the served content.

## Pilot page

`http://<server>/_clapier/pilot` drives the rabbits from a browser: one
block per rabbit with its polled state (rssi, ears, audio, choreo) and
the controls - ping, salute, the .chor library, a text box to compose a
choreography in the Violet API dialect (encoded server side by
`clapier-chor`), and a sentence box for the voice when `--say-script`
points at a speech pipeline. Every command goes through a gate that
bounds the arguments; the volume floor of 40 is written into it, in
memory of a power supply.

`--poll-secs` sets the /status polling cadence (default 30, 0 off).

## Fleet table

One line per rabbit, built from what already travels on the wire -
nothing is asked of the rabbits:

- the `m` query param (the MAC the boot sends on `bc.jsp?...&m=...`)
  binds a rabbit to its IP and dates its last boot fetch;
- the garenne application broadcasts a pulse every 2 s on UDP 9999
  (`garenne 0.9.0 up=42s link=4`); the clapier listens and remembers
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
