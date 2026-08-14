# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-08-15

### Added

- Clapier can now live on a Raspberry Pi 1: `scripts/build-rpi1.sh`
  cross-builds the server and garenne-ctl for ARMv6 musl (cargo-zigbuild,
  arm1176jzf-s), `deploy/openrc/clapier` + `scripts/deploy-pi.sh` install
  it as an Alpine/OpenRC service under /opt/clapier (dedicated user,
  setcap for port 80). The release workflow ships a `linux-armv6` archive.
- `led I RRGGBB` control verb (garenne 0.12.0): one LED to a static
  color, untouched by the breathing animation. Born for the daily Tempo
  color on the belly LED, driven by maison.

### Changed

- garenne's default clapier address is the Pi (192.168.1.103); the Mac
  launchd agent is retired. The speech pipeline (pocket-tts, ffmpeg)
  remains a Mac-side workflow.

### Fixed

- garenne-ctl no longer trips the `libc::time_t` deprecation on musl
  targets (width-agnostic cast).

## [0.1.5] - 2026-08-01

### Added

- The fleet self-heals: garenne 0.11.1 signs its two-second pulse with
  the rabbit's MAC, so a restarted clapier re-identifies every rabbit
  from the pulse alone, and the pilot never goes blind again. Older
  pulses still parse; the trailing field is optional.

- The log socket answers a roll call at bind time: on macOS a
  SO_REUSEPORT join can race the dying previous instance and leave the
  new socket deaf to broadcasts while looking perfectly bound. The
  listener now proves it hears (a broadcast probe, or any rabbit pulse,
  within 2.5 s) and rebinds until it does.

### Fixed

- `garenne-ctl` accepts broadcast targets again, as the Python tool
  always did (`SO_BROADCAST` was lost in the port).
- `/favicon.ico` no longer pollutes the request log and journal.
- The launchd template passes `--garenne` with an absolute path: agents
  start from `/`, where the relative default never finds the brain, and
  the adoption promise held everywhere but there.

## [0.1.4] - 2026-08-01

### Added

- The Violet shelf (`docs/violet/`): the original Metal reference (676
  pages), the grammar and garbage collector papers, the WiFi driver
  documentation, the VM natives table and the original boot and nominal
  bytecode sources, mirrored from Sylvain Huet's site with attribution
  so they outlive any single website. Marked `linguist-vendored`.
- `garenne-ctl`: the rabbit remote in Rust, in every platform archive.
  One-shot commands over the UDP control port and a `listen` mode for
  the timestamped log channel; replaces the Python scripts, which are
  gone.
- This changelog.
- `cargo-deny` guards licenses, advisories and dependency sources, locally
  (`just deny`) and in CI.
- A `justfile` gathering the everyday recipes: build, checks, the garenne
  suite, deploys and release cutting.
- `CHANGELOG.md` rides in the release archives.

### Changed

- The MTL ABI document sheds its 2026 campaign notes and becomes the
  timeless reference it was growing into.

## [0.1.3] - 2026-08-01

### Added

- The adoption: a rabbit fetching its bytecode from an empty burrow gets
  `garenne.bin` installed into `overlay/rabbits/<mac>/` on the spot, then
  served. `--garenne` points at another brain, a missing file turns
  adoption off, and a `bc.jsp` already in the overlay always wins.
- `garenne.bin` ships inside every platform archive, next to the server,
  so the unpacked directory works with nothing to copy.

## [0.1.2] - 2026-08-01

### Changed

- The bundled firmware moves to `wpa23-gtk` r2: the config portal becomes
  one modern, mobile-friendly, self-contained page (same fields, same
  routes, same on-device WPA key derivation), and its HTTP responses gain
  charset, an explicit close and a real Content-Length. The 2006 pages
  stay in the firmware source behind `ifdef PORTAL2006`.
- The README walks through the WiFi setup portal step by step.

## [0.1.1] - 2026-07-31

### Added

- `flash-nabaztag`: the firmware flasher rewritten in Rust and shipped in
  every platform archive. Raw HTTP/1.0 POST to the bootloader, real
  backpressure against its 800-byte receive window, no send timeout, the
  kernel taught the same patience, and the connection drop at the end
  read as the success it is.

### Changed

- The release mirrors only the `.sim` from the firmware repository: the
  flasher is a clapier tool, not a firmware asset.
- The README describes the historical `u.htm` upload page honestly: it
  works, blind and easily outrunning the rabbit.

### Fixed

- `SO_REUSEPORT` is unix-gated, so the Windows build exists at all.

## [0.1.0] - 2026-07-31

### Added

- First release. Ready-to-run archives for Linux x86_64 and aarch64
  (static musl), macOS Apple Silicon and Windows, each carrying `clapier`
  (the burrow server) and `chor-encode` (the Violet choreography
  encoder).
- `garenne.bin` built in CI from the committed mtl-dev Dockerfile, with
  the golden-frame suite as the release gate.
- The hardware-proven `wpa23-gtk` firmware mirrored into the release,
  with checksums for everything.
- The README's adoption guide, from cupboard to burrow.

[Unreleased]: https://github.com/uplg/clapier/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/uplg/clapier/compare/v0.1.5...v0.2.0
[0.1.5]: https://github.com/uplg/clapier/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/uplg/clapier/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/uplg/clapier/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/uplg/clapier/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/uplg/clapier/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/uplg/clapier/releases/tag/v0.1.0
