#!/usr/bin/env bash
# One-shot setup for the ai-monopoly-controller systemd unit. Run from inside
# the checked-out repo, wherever that is on this host — no need to reclone
# into a fixed path. Safe to re-run.
set -euo pipefail

REPO_ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
WORKER="$REPO_ROOT/services/monopoly-worker"

# The unit hardcodes /opt/exposure-academy; point it at the real checkout
# with a symlink instead of moving/copying anything or templating the unit.
sudo id exposure-monopoly >/dev/null 2>&1 \
  || sudo useradd --system --home /var/lib/exposure-monopoly --create-home \
       --groups docker exposure-monopoly
sudo usermod -aG docker exposure-monopoly
sudo ln -sfn "$REPO_ROOT" /opt/exposure-academy

[ -d /opt/exposure-academy/.venv-monopoly ] \
  || sudo python3 -m venv /opt/exposure-academy/.venv-monopoly
sudo /opt/exposure-academy/.venv-monopoly/bin/pip install -q \
  -r "$WORKER/requirements.txt"

sudo mkdir -p /etc/exposure-academy /var/lib/exposure/monopoly-artifacts
sudo chown exposure-monopoly:exposure-monopoly /var/lib/exposure/monopoly-artifacts
if [ ! -f /etc/exposure-academy/monopoly-controller.env ]; then
  sudo install -m 0600 -o exposure-monopoly -g exposure-monopoly \
    "$WORKER/systemd/monopoly-controller.env.example" \
    /etc/exposure-academy/monopoly-controller.env
  echo "Wrote /etc/exposure-academy/monopoly-controller.env from the example."
fi

sudo cp "$WORKER/systemd/ai-monopoly-controller.service" /etc/systemd/system/
sudo systemctl daemon-reload

cat <<EOF

Done. Two things left, by hand:
  1. Put the real WORKER_TOKEN (must match the Academy service's own
     WORKER_TOKEN) and MONOPOLY_SITE into:
       /etc/exposure-academy/monopoly-controller.env
  2. sudo systemctl enable --now ai-monopoly-controller.service
EOF
