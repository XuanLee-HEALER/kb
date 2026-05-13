#!/usr/bin/env bash
# Ship a released artifact to aliyun + restart kb services.
#
# Prereqs (one-time, manual — see deploy/README.md):
#   - Phase 0: kb.lan leaf cert signed locally with etmesh-root-ca
#   - Phase 1: aliyun has kb user, /opt/kb tree, /etc/kb/env, /etc/nginx/ssl/kb.lan.*,
#              /etc/nginx/snippets/kb-auth.conf, and DNS A record for kb.lan
#
# This script only does mechanical work — download, scp, extract, restart.
# Every step is idempotent; running twice with the same tag is a no-op
# beyond the systemctl restart.
#
# Usage: scripts/deploy-aliyun.sh <release-tag>           (e.g. v0.1.0)
#        KB_DEPLOY_HOST=other-host scripts/deploy-aliyun.sh v0.1.0

set -euo pipefail

TAG="${1:?usage: $0 <release-tag>}"
ART="kb-release-linux-x86_64.tar.gz"
HOST="${KB_DEPLOY_HOST:-aliyun}"
REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
STAGING="$REPO_ROOT/.tmp/release"

mkdir -p "$STAGING"

echo "▶ download $TAG from GitHub release"
gh release download "$TAG" -p "$ART" -D "$STAGING" --clobber

echo "▶ scp to $HOST"
scp "$STAGING/$ART" "$HOST:/tmp/"

echo "▶ remote install + restart"
ssh "$HOST" "set -euo pipefail
  cd /opt/kb
  tar -xzf '/tmp/$ART' --no-same-owner
  chown -R kb:kb /opt/kb

  ln -sf /opt/kb/deploy/kb-server.service /etc/systemd/system/kb-server.service
  ln -sf /opt/kb/deploy/kb-web.service    /etc/systemd/system/kb-web.service
  ln -sf /opt/kb/deploy/kb.lan.conf       /etc/nginx/sites-available/kb.lan
  ln -sf /etc/nginx/sites-available/kb.lan /etc/nginx/sites-enabled/kb.lan

  systemctl daemon-reload
  systemctl enable --now kb-server kb-web
  systemctl restart kb-server kb-web

  nginx -t
  systemctl reload nginx

  systemctl is-active --quiet kb-server && echo '  ✓ kb-server active'
  systemctl is-active --quiet kb-web    && echo '  ✓ kb-web active'
"

echo "▶ verify via mesh"
ssh "$HOST" "curl -sSk --connect-timeout 5 -o /dev/null -w 'GET https://kb.lan/api/stats → %{http_code}\\n' \
    --resolve kb.lan:443:10.177.0.1 -H \"Authorization: Bearer \$(grep ^KB_TOKEN= /etc/kb/env | cut -d= -f2)\" \
    https://kb.lan/api/stats || true"

echo "✓ deployed $TAG → $HOST"
