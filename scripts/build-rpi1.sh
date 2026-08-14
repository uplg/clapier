#!/usr/bin/env bash
set -euo pipefail

# Cross-compile the clapier server (+ garenne-ctl) for the Raspberry Pi 1
# (ARMv6, musl, static). Same toolchain recipe as maison's backend:
# cargo-zigbuild + zig as the linker, tuned for the arm1176jzf-s core.

TARGET="${TARGET:-arm-unknown-linux-musleabihf}"
TOOLCHAIN="${RUSTUP_TOOLCHAIN:-stable}"
DEFAULT_RUSTFLAGS="-C target-cpu=arm1176jzf-s"

for tool in rustup cargo-zigbuild zig; do
  if ! command -v "${tool}" >/dev/null 2>&1; then
    printf 'Missing %s. Install it first.\n' "${tool}" >&2
    exit 1
  fi
done

if ! rustup target list --installed | grep -qx "${TARGET}"; then
  printf 'Missing Rust target: %s\nInstall it with: rustup target add %s\n' "${TARGET}" "${TARGET}" >&2
  exit 1
fi

export RUSTFLAGS="${DEFAULT_RUSTFLAGS}${RUSTFLAGS:+ ${RUSTFLAGS}}"
export RUSTC="$(rustup which --toolchain "${TOOLCHAIN}" rustc)"

if [ "$(uname -s)" = "Darwin" ]; then
  ulimit -n "${BUILD_ULIMIT_NOFILE:-4096}" 2>/dev/null || true
fi

printf 'Cross-compiling clapier + garenne-ctl for %s\n' "${TARGET}"

rustup run "${TOOLCHAIN}" cargo zigbuild \
  --release \
  --target "${TARGET}" \
  -p clapier \
  --bins \
  "$@"

printf 'Artifacts: target/%s/release/{clapier,garenne-ctl}\n' "${TARGET}"
