#!/usr/bin/env bash
# Build garenne (the rabbit's embedded application) with the Metal
# toolchain vendored in this repository (vendor/metal: Sylvain Huet's
# mtl compiler and simulator plus the preprocessing scripts).
#
# Usage:
#   ./build.sh        device build -> build/garenne.bin (servable as bc.jsp)
#   ./build.sh sim    simulator build + run (ANSI LED view; Ctrl-C to quit)
#   ./build.sh test   golden-frame test suite in the simulator
#
# Requires the mtl-dev Docker image (debian bookworm amd64 + multilib).
# The toolchain binaries are built inside that image on first use.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
METAL=/work/vendor/metal
MODE="${1:-bin}"

mkdir -p "$ROOT/garenne/build"

in_toolchain() {
  docker run --rm --platform linux/amd64 -v "$ROOT:/work" \
    -w /work/garenne/build mtl-dev bash -c "$1"
}

# The vendored tree carries sources only; compile the compiler and the
# simulator inside the image the first time (or after a clean).
ensure_toolchain() {
  if [[ ! -x "$ROOT/vendor/metal/compiler/mtl_comp/mtl_comp" ]] \
    || [[ ! -x "$ROOT/vendor/metal/compiler/mtl_simu/mtl_simu" ]]; then
    in_toolchain "make -C $METAL/compiler"
  fi
}

preprocess() {  # entry file, output file, extra defs
  in_toolchain "perl $METAL/scripts/preproc.pl ${3:-} $1 \
    | python3 $METAL/scripts/preproc_remove_extra_protos.py > $2"
}

ensure_toolchain

case "$MODE" in
  bin)
    preprocess /work/garenne/main.mtl /work/garenne/build/garenne.mtl
    in_toolchain "$METAL/compiler/mtl_comp/mtl_comp -s /work/garenne/build/garenne.mtl \
        /work/garenne/build/garenne.bin \
      && ls -la /work/garenne/build/garenne.bin"
    ;;
  sim)
    preprocess /work/garenne/main.mtl /work/garenne/build/garenne.mtl "-D SIMU"
    docker run --rm --platform linux/amd64 -v "$ROOT:/work" \
      -w /work/garenne/build mtl-dev \
      $METAL/compiler/mtl_simu/mtl_simu --mac 0123456789ab --logs init,vm \
      --source /work/garenne/build/garenne.mtl
    ;;
  test)
    preprocess /work/garenne/tests/main.mtl /work/garenne/build/garenne-tests.mtl "-D SIMU"
    # Secho prints on the vm log channel; the LED ANSI rendering is noise
    # we strip before judging the run.
    out="$(in_toolchain "timeout 8 $METAL/compiler/mtl_simu/mtl_simu --mac 0123456789ab \
      --logs init,vm --source /work/garenne/build/garenne-tests.mtl; true" \
      | perl -pe 's/\e\[[0-9;]*[A-Za-z]|\e\[[su]|\r//g')"
    echo "$out" | grep -E 'FAIL|got|want|TESTS' || true
    if echo "$out" | grep -q 'fail=0' && ! echo "$out" | grep -q 'FAIL'; then
      echo "test suite green"
    else
      echo "test suite RED" >&2
      exit 1
    fi
    ;;
  *)
    echo "unknown mode: $MODE" >&2
    exit 1
    ;;
esac
