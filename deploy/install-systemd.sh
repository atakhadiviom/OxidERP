#!/usr/bin/env bash
set -euo pipefail

APP_USER=${APP_USER:-oxiderp}
APP_DIR=${APP_DIR:-/opt/oxiderp}
ENV_DIR=${ENV_DIR:-/etc/oxiderp}
BIN_PATH=${BIN_PATH:-/usr/local/bin/oxiderp-core}
DATABASE_URL=${DATABASE_URL:-postgres://oxiderp:oxiderp_dev_password@127.0.0.1:5432/oxiderp}
OXIDERP_BIND=${OXIDERP_BIND:-0.0.0.0:8080}
RUST_LOG=${RUST_LOG:-info}

if [[ $EUID -ne 0 ]]; then
  echo "Please run as root: sudo bash deploy/install-systemd.sh" >&2
  exit 1
fi

if ! id "$APP_USER" >/dev/null 2>&1; then
  useradd --system --create-home --shell /usr/sbin/nologin "$APP_USER"
fi

mkdir -p "$APP_DIR" "$ENV_DIR"
chown -R "$APP_USER:$APP_USER" "$APP_DIR"

cargo build --release -p oxiderp-core
install -m 0755 target/release/oxiderp-core "$BIN_PATH"
install -m 0644 deploy/oxiderp.service /etc/systemd/system/oxiderp.service

cat > "$ENV_DIR/oxiderp.env" <<ENV
DATABASE_URL=$DATABASE_URL
OXIDERP_BIND=$OXIDERP_BIND
RUST_LOG=$RUST_LOG
ENV
chmod 0640 "$ENV_DIR/oxiderp.env"
chown root:"$APP_USER" "$ENV_DIR/oxiderp.env"

systemctl daemon-reload
systemctl enable oxiderp
systemctl restart oxiderp
systemctl --no-pager status oxiderp
