# clapier 🐰

[![ci](https://github.com/uplg/clapier/actions/workflows/ci.yml/badge.svg)](https://github.com/uplg/clapier/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/uplg/clapier)](https://github.com/uplg/clapier/releases)
[![changelog](https://img.shields.io/badge/changelog-keep%20a%20changelog-E05735)](CHANGELOG.md)
[![license](https://img.shields.io/github/license/uplg/clapier)](LICENSE)

The Nabaztag:tag's HTTP burrow: one binary serves a rabbit its
bytecode, its sounds and its choreographies, and gives you a status
page, a browser cockpit and a fleet view. Pairs with the
[uplg/nabgcc](https://github.com/uplg/nabgcc) `wpa23-gtk` firmware,
which teaches the 2006 rabbit WPA2/WPA3.

## Install

Grab the [latest release](https://github.com/uplg/clapier/releases):
archives for Linux x86_64 and aarch64 (static, NAS and Raspberry Pi
friendly), macOS Apple Silicon and Windows, each with the server, the
choreography encoder, the firmware flasher and the rabbit's brain
(`garenne.bin`), plus the firmware and checksums. No toolchain, no
Python, nothing else.

Still have a rabbit in a cupboard? The whole journey:

1. **Flash the firmware once.** Hold the head button while powering
   on: the rabbit opens a `NabaztagXX` access point. Join it and run
   `./flash-nabaztag Nab-wpa23-gtk-*.sim` (the flasher is in your
   platform archive, the `.sim` next to it). It paces the upload to
   the bootloader's rhythm and shows progress; the historical
   `http://192.168.0.1/u.htm` upload page still works, but it gives
   no feedback and browsers tend to outrun the rabbit and stall.
   Either way the bootloader itself is never touched: a bad flash is
   always recoverable from the same menu.
2. **Run the server.** Unpack the archive for your machine and run
   `./clapier --bind 0.0.0.0:80 --overlay overlay`. Any always-on box
   on your LAN will do. On Linux, port 80 wants root or a one-time
   `sudo setcap 'cap_net_bind_service=+ep' ./clapier`.
3. **Configure WiFi and point the rabbit at your server.** Put the
   rabbit back in config mode (head button + power), join
   `NabaztagXX` again and browse to `http://192.168.0.1`. The setup
   page lists the networks it hears: pick yours, select its
   encryption (WPA2 is the point of the new firmware), and type your
   key - the rabbit derives the WPA key material itself, any normal
   passphrase works. Then scroll to "Advanced configuration" and put
   your server's IP in the service address field. Save: the rabbit
   reboots, joins your network, and on its first bytecode fetch
   clapier adopts it - `garenne.bin` (in the archive, next to the
   server) is installed into `overlay/rabbits/<mac>/` on the spot and
   served. Ears, button, choreographies, streamed MP3, nothing to
   copy. The installed file is yours to inspect or replace;
   `--garenne` points elsewhere, and a `bc.jsp` already in the
   overlay always wins over adoption.
4. **Open the burrow.** `http://<server>/_clapier` for the fleet,
   `/_clapier/pilot` to drive from the browser.

The rabbit refetches its bytecode at every boot, so the worst a bad
`bc.jsp` costs is a power cycle, never a brick.

## From source

```console
$ cargo build --release
$ ./target/release/clapier \
    --bind 0.0.0.0:80 \
    --overlay garenne/overlay \
    --rabbit 192.168.1.155
```

`--rabbit` tags that IP with a 🐰 in the logs. `--root` optionally
mounts a legacy Violet tree behind the overlay for stock bytecodes.
The file service speaks the rabbit's 2006 dialect: `Connection: close`,
complete bodies with `Content-Length`, query strings ignored for file
resolution, path traversal refused.

A [justfile](justfile) carries these as recipes for those who have
[just](https://github.com/casey/just) (`just serve` for a quick look
on :8080); every raw command stays in this README for those who do
not.

## Pages

- `/_clapier` - uptime, last visit, fleet table, recent requests;
- `/_clapier/health` - `ok`;
- `/_clapier/pilot` - the cockpit: per-rabbit state (rssi, ears,
  audio, choreo), ping, salute, the .chor library, a box to compose a
  choreography in the Violet API dialect, and a sentence box once
  `--say-script` points at a speech pipeline. Every command passes a
  gate that bounds the arguments; the volume floor of 40 is written
  into it, in memory of a power supply.

The fleet table is built from what already travels on the wire: the
MAC the boot sends on `bc.jsp`, and the pulse garenne broadcasts every
2 s on UDP 9999 (`--pulse-port` moves it, `0` disables). `--poll-secs`
sets the /status polling cadence (default 30, 0 off).

## rabbit-say

Text to speech through the rabbit: Kyutai Pocket TTS (French, the
Estelle voice) rendered as the mono 32 kHz MP3 the VS1003 has chewed
since 2006, installed into the overlay and played on the spot:

```console
$ git submodule update --init vendor/pocket-tts
$ cd vendor/pocket-tts && cargo build --release -p pocket-tts-cli \
    --no-default-features --features metal && cd ../..
$ ./scripts/rabbit-say.sh "Bonjour."
```

(Or `just say "Bonjour."` once pocket-tts is built.)

## Hacking on garenne

The application the rabbit runs: cooperative scheduler, LLC/SNAP
framing, ARP, ICMP, UDP, TCP both ways, HTTP, streamed MP3, ears,
button, watchdog. Golden-frame tested against independent Python
vectors; the same suite gates every release.

```console
$ ./garenne/build.sh test    # the golden suite in the simulator
$ ./garenne/build.sh         # device build -> garenne/build/garenne.bin
$ ./scripts/deploy-garenne.sh --rabbit 00:19:db:9c:28:15
```

(Or `just garenne-test`, `just garenne`, `just deploy <mac>`.)

## launchd service (macOS)

[`deploy/fr.uplg.clapier.plist`](deploy/fr.uplg.clapier.plist) (adjust
the paths) gives a server that starts with the session and restarts on
its own:

```console
$ cp deploy/fr.uplg.clapier.plist ~/Library/LaunchAgents/
$ launchctl bootstrap gui/$UID ~/Library/LaunchAgents/fr.uplg.clapier.plist
```

Stop with `launchctl bootout gui/$UID/fr.uplg.clapier`.

## Development

```console
$ cargo test      # including a smoke test that speaks HTTP/1.0 like the rabbit
$ cargo clippy --all-targets -- -D warnings
$ cargo fmt --check
$ cargo deny check
```

(Or `just check` and `just deny`.)

Releases are cut by tagging: bump the workspace version, tag `vX.Y.Z`,
push, and CI builds every platform, golden-tests the bytecode, attaches
the latest firmware and publishes. `just release X.Y.Z` does the whole
dance after checking the tree is clean.

## Architecture

```
crates/
  clapier          the binary: CLI, router, request logging
  clapier-vl       the file service in the rabbit's dialect
  clapier-chor     the Violet choreography encoder (chor-encode)
  clapier-flash    the firmware flasher (flash-nabaztag)
  clapier-fleet    the fleet register, learned from the wire
  clapier-journal  the request journal (bounded ring, thread-safe)
  clapier-pages    the pages for humans (status, pilot, listings)
garenne/           the rabbit's embedded application (Metal bytecode,
                   our own IP/TCP/HTTP stack, served as vl/bc.jsp)
scripts/           deploy, remote control, log listener, rabbit-say
vendor/
  metal            the Metal toolchain (mtl compiler and simulator),
                   built inside the mtl-dev Docker image on demand
  pocket-tts       Kyutai Pocket TTS, native Rust port (submodule,
                   https://github.com/uplg/pocket-tts)
```

## License

MIT. Inspired by the community project ServerlessNabaztag.
