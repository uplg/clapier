#!/usr/bin/env bash
# Write a dance in the Violet API's own text dialect and watch the
# rabbit perform it: the CDL is encoded to binary .chor (chor-encode),
# installed in the overlay, and played through ctl chor.
#
# Usage:
#   rabbit-dance.sh [options] "fps,t,type,args,..."
#   rabbit-dance.sh [options] @score.cdl
#
# Options:
#   --rabbit MAC    target rabbit (default 0019db9c2815)
#   --name FILE     chor basename under vl/chor/ (default custom)
#   --no-play       encode and install only
#
# Dialect (commas, like api.nabaztag.com in 2006):
#   fps
#   t,led,<led 0-4>,<r>,<g>,<b>         led byte is the wire value:
#                                       0=base 2=middle 4=nose
#   t,motor,<0|1>,<angle 0-360>,<0>,<dir 0|1>
#   t,palette,<led>,<index 0-7>
#
# Power manners on a tired supply: move one ear at a time.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OVERLAY="$ROOT/garenne/overlay"
ENCODER="$ROOT/target/release/chor-encode"
RABBIT="0019db9c2815"
NAME="custom"
PLAY=1
CDL=""

normalize_mac() {
  local mac
  mac="$(echo "$1" | tr -d ':' | tr '[:upper:]' '[:lower:]')"
  if [[ ! "$mac" =~ ^[0-9a-f]{12}$ ]]; then
    echo "invalid MAC: $1" >&2
    exit 1
  fi
  echo "$mac"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --rabbit) RABBIT="$(normalize_mac "$2")"; shift 2 ;;
    --name) NAME="$2"; shift 2 ;;
    --no-play) PLAY=0; shift ;;
    @*) CDL="$(cat "${1#@}")"; shift ;;
    *) CDL="$1"; shift ;;
  esac
done

if [[ -z "$CDL" ]]; then
  echo "nothing to dance" >&2
  exit 1
fi

if [[ ! -x "$ENCODER" ]]; then
  echo "building chor-encode..."
  (cd "$ROOT" && cargo build --release -q -p clapier-chor)
fi

DEST_DIR="$OVERLAY/rabbits/$RABBIT/vl/chor"
mkdir -p "$DEST_DIR"
"$ENCODER" "$CDL" "$DEST_DIR/.$NAME.chor.tmp"
mv "$DEST_DIR/.$NAME.chor.tmp" "$DEST_DIR/$NAME.chor"
echo "installed $(stat -f %z "$DEST_DIR/$NAME.chor") bytes at /vl/chor/$NAME.chor"

if [[ $PLAY -eq 1 ]]; then
  "${GARENNE_CTL:-$ROOT/target/release/garenne-ctl}" chor "/vl/chor/$NAME.chor"
fi
