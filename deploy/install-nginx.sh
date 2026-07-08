#!/usr/bin/env bash
set -euo pipefail

DOMAIN=${1:-_}

if [[ $EUID -ne 0 ]]; then
  echo "Please run as root: sudo bash deploy/install-nginx.sh [domain]" >&2
  exit 1
fi

apt-get update
apt-get install -y nginx

cp deploy/nginx.conf /etc/nginx/sites-available/oxiderp
if [[ "$DOMAIN" != "_" ]]; then
  sed -i "s/server_name _;/server_name $DOMAIN;/" /etc/nginx/sites-available/oxiderp
fi
ln -sf /etc/nginx/sites-available/oxiderp /etc/nginx/sites-enabled/oxiderp
rm -f /etc/nginx/sites-enabled/default
nginx -t
systemctl enable nginx
systemctl reload nginx

echo "Nginx configured. For HTTPS run: certbot --nginx -d $DOMAIN"
