#!/usr/bin/env bash
# Make the rabbit speak: Kyutai Pocket TTS (French, the Estelle voice)
# rendered as a Violet-shaped MP3 (mono, 32 kHz, constant bitrate, like
# the original respiration and clock files the VS1003 has chewed since
# 2006), dropped into the clapier overlay and streamed by garenne.
#
# Usage:
#   rabbit-say.sh [options] "text to speak"
#
# Options:
#   --rabbit MAC    target rabbit (default 0019db9c2815); accepts
#                   00:19:db:9c:28:15 or 0019db9c2815
#   --voice SPEC    pocket engine: hf:// URL, .safetensors embedding or
#                   .wav to clone (default: Kyutai Estelle, french_24l);
#                   say engine: a macOS voice name (default Jacques)
#   --engine E      pocket (default) or say (the macOS fallback)
#   --name FILE     mp3 basename under vl/say/ (default latest)
#   --no-play       generate and install only, do not trigger playback
#
# The pocket engine wants the pocket-tts-cli binary built from the
# vendored tree (cd vendor/pocket-tts && cargo build --release
# -p pocket-tts-cli --no-default-features --features metal); override
# its location with POCKET_TTS_BIN. Model weights download from
# HuggingFace on first use. The file lands at
# overlay/rabbits/<mac>/vl/say/<FILE>.mp3, served by clapier at
# /vl/say/<FILE>.mp3 for that rabbit.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OVERLAY="$ROOT/garenne/overlay"
POCKET_BIN="${POCKET_TTS_BIN:-$ROOT/vendor/pocket-tts/target/release/pocket-tts-cli}"
ESTELLE="hf://kyutai/pocket-tts-without-voice-cloning/languages/french_24l/embeddings/estelle.safetensors"
RABBIT="0019db9c2815"
ENGINE="pocket"
VOICE=""
NAME="latest"
PLAY=1
TEXT=""

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
    --voice) VOICE="$2"; shift 2 ;;
    --engine) ENGINE="$2"; shift 2 ;;
    --name) NAME="$2"; shift 2 ;;
    --no-play) PLAY=0; shift ;;
    *) TEXT="$1"; shift ;;
  esac
done

if [[ -z "$TEXT" ]]; then
  echo "nothing to say" >&2
  exit 1
fi

DEST_DIR="$OVERLAY/rabbits/$RABBIT/vl/say"
DEST="$DEST_DIR/$NAME.mp3"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

case "$ENGINE" in
  pocket)
    if [[ ! -x "$POCKET_BIN" ]]; then
      echo "pocket-tts-cli not found at $POCKET_BIN (build it, or set POCKET_TTS_BIN, or --engine say)" >&2
      exit 1
    fi
    # Generation settings matched to lana's Estelle tuning.
    "$POCKET_BIN" generate --quiet --use-metal --variant french_24l \
      --voice "${VOICE:-$ESTELLE}" --eos-threshold=-3.0 \
      -t "$TEXT" -o "$TMP/say.wav"
    ;;
  say)
    say -v "${VOICE:-Jacques}" -o "$TMP/say.aiff" "$TEXT"
    mv "$TMP/say.aiff" "$TMP/say.wav"
    ;;
  *)
    echo "unknown engine: $ENGINE (pocket or say)" >&2
    exit 1
    ;;
esac

# Mono 32 kHz CBR 48 kbps: the era's shape, and at ~6 KB/s it flows
# far under garenne's paced receive window.
lame --quiet -m m --resample 32 -b 48 "$TMP/say.wav" "$TMP/say.mp3"

mkdir -p "$DEST_DIR"
# Atomic within the same filesystem: the rabbit never streams half a file.
cp "$TMP/say.mp3" "$DEST_DIR/.$NAME.mp3.tmp"
mv "$DEST_DIR/.$NAME.mp3.tmp" "$DEST"
echo "installed $(stat -f %z "$DEST") bytes at /vl/say/$NAME.mp3 (engine $ENGINE)"

if [[ $PLAY -eq 1 ]]; then
  python3 "$ROOT/scripts/garenne-ctl.py" play "/vl/say/$NAME.mp3"
fi
