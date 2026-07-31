# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- This changelog.
- `cargo-deny` guards licenses, advisories and dependency sources, locally
  (`just deny`) and in CI.
- A `justfile` gathering the everyday recipes: build, checks, the garenne
  suite, deploys and release cutting.
- `CHANGELOG.md` rides in the release archives.

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

[Unreleased]: https://github.com/uplg/clapier/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/uplg/clapier/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/uplg/clapier/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/uplg/clapier/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/uplg/clapier/releases/tag/v0.1.0
