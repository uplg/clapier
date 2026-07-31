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
#   --voice SPEC    pocket engine: a Kyutai voice name, hf:// URL,
#                   .safetensors embedding or .wav to clone (default:
#                   the model's language voice, Estelle for french_24l);
#                   say engine: a macOS voice name (default Jacques)
#   --temp T        pocket engine sampling temperature (default: the
#                   model's recommended value from its config)
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
RABBIT="0019db9c2815"
ENGINE="pocket"
VOICE=""
TEMP=""
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
    --temp) TEMP="$2"; shift 2 ;;
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
    # Latency does not matter here, quality does: 8 flow-matching
    # decode steps instead of lana's realtime 1. No --voice means the
    # model's language default (Estelle for french_24l); no --temp means
    # the config's recommended temperature.
    EXTRA=()
    [[ -n "$VOICE" ]] && EXTRA+=(--voice "$VOICE")
    [[ -n "$TEMP" ]] && EXTRA+=(--temperature "$TEMP")
    "$POCKET_BIN" generate --quiet --use-metal --language french_24l \
      --eos-threshold=-3.0 --lsd-decode-steps 8 ${EXTRA[@]+"${EXTRA[@]}"} \
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

# Mono 32 kHz CBR: the era's shape, and at ~8 KB/s it flows far under
# garenne's paced receive window. The model speaks at -26 dB mean, way
# under the Violet sounds it shares a speaker with, so the loudness is
# normalized to speech level; ffmpeg also resamples 24 to 32 kHz much
# more cleanly than lame would.
ffmpeg -hide_banner -loglevel error -i "$TMP/say.wav" \
  -af "loudnorm=I=-16:TP=-1.5:LRA=11" -ar 32000 -ac 1 \
  -c:a libmp3lame -b:a 64k "$TMP/say.mp3"

mkdir -p "$DEST_DIR"
# Atomic within the same filesystem: the rabbit never streams half a file.
cp "$TMP/say.mp3" "$DEST_DIR/.$NAME.mp3.tmp"
mv "$DEST_DIR/.$NAME.mp3.tmp" "$DEST"
echo "installed $(stat -f %z "$DEST") bytes at /vl/say/$NAME.mp3 (engine $ENGINE)"

if [[ $PLAY -eq 1 ]]; then
  python3 "$ROOT/scripts/garenne-ctl.py" play "/vl/say/$NAME.mp3"
fi
