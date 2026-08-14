#!/usr/bin/env bash
set -euo pipefail

# Deploy clapier to the Raspberry Pi 1 (Alpine/OpenRC).
#
#   PI_HOST=root@192.168.1.103 ./scripts/deploy-pi.sh [--seed-overlay]
#
# Pushes the cross-built binaries, the garenne bytecode, and (optionally) the
# overlay tree, then installs and restarts the OpenRC service. The overlay is
# runtime state on the Pi (adoption writes into it), so by default only
# common/ is synced; --seed-overlay pushes the full tree (first deploy, or
# to restore a rabbit's burrow).

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "${SCRIPT_DIR}")"
PI_HOST="${PI_HOST:-root@192.168.1.103}"
APP_DIR="${APP_DIR:-/opt/clapier}"
RABBIT_IP="${RABBIT_IP:-192.168.1.155}"
TARGET="${TARGET:-arm-unknown-linux-musleabihf}"
BIN_DIR="${REPO_ROOT}/target/${TARGET}/release"

SEED_OVERLAY=0
[ "${1:-}" = "--seed-overlay" ] && SEED_OVERLAY=1

for artifact in clapier garenne-ctl; do
  if [ ! -f "${BIN_DIR}/${artifact}" ]; then
    printf 'Missing %s. Run ./scripts/build-rpi1.sh first.\n' "${BIN_DIR}/${artifact}" >&2
    exit 1
  fi
done

if [ ! -f "${REPO_ROOT}/garenne/build/garenne.bin" ]; then
  printf 'Missing garenne/build/garenne.bin. Run garenne/build.sh first.\n' >&2
  exit 1
fi

printf '==> Preparing %s on %s\n' "${APP_DIR}" "${PI_HOST}"
ssh "${PI_HOST}" sh -s -- "${APP_DIR}" <<'EOF'
set -eu
APP_DIR="$1"
apk add --no-cache rsync libcap >/dev/null
if ! getent group clapier >/dev/null 2>&1; then addgroup -S clapier; fi
if ! id -u clapier >/dev/null 2>&1; then
  adduser -S -D -H -h "${APP_DIR}" -G clapier -s /sbin/nologin clapier
fi
mkdir -p "${APP_DIR}/overlay"
EOF

printf '==> Pushing binaries and bytecode\n'
rsync -avz "${BIN_DIR}/clapier" "${BIN_DIR}/garenne-ctl" "${PI_HOST}:${APP_DIR}/"
rsync -avz "${REPO_ROOT}/garenne/build/garenne.bin" "${PI_HOST}:${APP_DIR}/garenne.bin"

if [ "${SEED_OVERLAY}" = "1" ]; then
  printf '==> Seeding full overlay tree\n'
  rsync -avz "${REPO_ROOT}/garenne/overlay/" "${PI_HOST}:${APP_DIR}/overlay/"
else
  printf '==> Syncing overlay common/ (rabbit burrows untouched)\n'
  rsync -avz "${REPO_ROOT}/garenne/overlay/common/" "${PI_HOST}:${APP_DIR}/overlay/common/"
fi

printf '==> Installing OpenRC service and restarting\n'
TMP_UNIT="$(mktemp)"
sed -e "s#@@RABBIT_IP@@#${RABBIT_IP}#g" "${REPO_ROOT}/deploy/openrc/clapier" > "${TMP_UNIT}"
rsync -avz "${TMP_UNIT}" "${PI_HOST}:/etc/init.d/clapier"
rm -f "${TMP_UNIT}"
ssh "${PI_HOST}" sh -s -- "${APP_DIR}" <<'EOF'
set -eu
APP_DIR="$1"
chmod +x /etc/init.d/clapier
chown -R clapier:clapier "${APP_DIR}"
# Port 80 without root.
setcap 'cap_net_bind_service=+ep' "${APP_DIR}/clapier"
rc-update add clapier default >/dev/null 2>&1 || true
rc-service clapier restart || rc-service clapier start
sleep 2
rc-service clapier status
EOF

printf '==> Done. Logs: ssh %s tail -f /var/log/clapier.log\n' "${PI_HOST}"
