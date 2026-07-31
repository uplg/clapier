#!/usr/bin/env bash
# The morning weather, spoken by the rabbit: wttr.in observation and
# forecast composed into a French sentence and handed to rabbit-say
# (Kyutai Pocket TTS, the Estelle voice).
#
# Usage:
#   rabbit-weather.sh [options]
#
# Options:
#   --loc PLACE     location for wttr.in (default: geolocated by IP)
#   --rabbit MAC    target rabbit, passed through to rabbit-say
#   --no-play       generate and install only, do not trigger playback
#
# The MP3 lands at /vl/say/meteo.mp3 so it never clobbers the ad-hoc
# /vl/say/latest.mp3. For a daily morning run, see
# deploy/fr.uplg.clapier-meteo.plist.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOC=""
SAY_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --loc) LOC="$2"; shift 2 ;;
    --rabbit) SAY_ARGS+=(--rabbit "$2"); shift 2 ;;
    --no-play) SAY_ARGS+=(--no-play); shift ;;
    *) echo "unknown argument: $1" >&2; exit 1 ;;
  esac
done

# One fetch carries everything: current condition (with its French
# wording), today's range, and the day's worst rain odds.
JSON="$(curl -sf -m 20 "https://wttr.in/${LOC// /+}?format=j1&lang=fr")" || {
  echo "wttr.in unreachable, the rabbit stays quiet" >&2
  exit 1
}

read -r TOWN TEMP MIN MAX RAIN < <(echo "$JSON" | jq -r '[
    .nearest_area[0].areaName[0].value,
    .current_condition[0].temp_C,
    .weather[0].mintempC,
    .weather[0].maxtempC,
    ([.weather[0].hourly[].chanceofrain | tonumber] | max)
  ] | @tsv' | tr '\t' ' ')
CONDITION="$(echo "$JSON" | jq -r '.current_condition[0].lang_fr[0].value' \
  | tr '[:upper:]' '[:lower:]')"

# Estelle reads digits fine; she just needs "moins" for the sign.
say_temp() { echo "${1/#-/moins }"; }

TEXT="Bonjour. À ${TOWN}, il fait $(say_temp "$TEMP") degrés, ${CONDITION}."
TEXT+=" Aujourd'hui, entre $(say_temp "$MIN") et $(say_temp "$MAX") degrés."
if [[ "$RAIN" -ge 50 ]]; then
  TEXT+=" Pensez au parapluie."
fi

echo "$TEXT"
exec "$ROOT/scripts/rabbit-say.sh" --name meteo "${SAY_ARGS[@]}" "$TEXT"
