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
# Usage:
#   scripts/deploy-aliyun.sh <release-tag>                  # full deploy
#   scripts/deploy-aliyun.sh <release-tag> --only=server    # only bin/ + restart kb-server
#   scripts/deploy-aliyun.sh <release-tag> --only=web       # only web/  + restart kb-web
#   scripts/deploy-aliyun.sh <release-tag> --only=skill     # only skill/ (no restart — server reads at request time)
#   scripts/deploy-aliyun.sh <release-tag> --only=configs   # only deploy/ + daemon-reload + nginx -s reload
#
# Env: KB_DEPLOY_HOST=other-host overrides the default `aliyun`.

set -euo pipefail

TAG=""
ONLY=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --only=*) ONLY="${1#--only=}"; shift ;;
        --only)   ONLY="${2:-}"; shift 2 ;;
        -*)       echo "✗ unknown flag: $1" >&2; exit 2 ;;
        *)
            if [[ -z "$TAG" ]]; then TAG="$1"; shift
            else echo "✗ unexpected positional arg: $1" >&2; exit 2; fi ;;
    esac
done
[[ -n "$TAG" ]] || { echo "usage: $0 <release-tag> [--only=server|web|skill|configs]" >&2; exit 2; }
case "$ONLY" in
    ""|server|web|skill|configs) ;;
    *) echo "✗ --only must be one of: server, web, skill, configs (got: $ONLY)" >&2; exit 2 ;;
esac

ART="kb-release-linux-x86_64.tar.gz"
HOST="${KB_DEPLOY_HOST:-aliyun}"
REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
STAGING="$REPO_ROOT/.tmp/release"

mkdir -p "$STAGING"

echo "▶ download $TAG from GitHub release"
gh release download "$TAG" -p "$ART" -D "$STAGING" --clobber

echo "▶ scp to $HOST"
scp "$STAGING/$ART" "$HOST:/tmp/"

# Remote step. For a full deploy we keep the original 'extract everything +
# (re-)symlink units + enable+restart both services + nginx reload' flow.
# For --only=X we touch the minimum needed: extract only that subtree, then
# restart only the affected service.
if [[ -z "$ONLY" ]]; then
    echo "▶ remote install + restart (full)"
    ssh "$HOST" "set -euo pipefail
      cd /opt/kb
      # skill/ is fully owned by the tarball — wipe stale entries (old
      # README.md, mcp.json, dropped examples) before extracting so deploys
      # don't accumulate cruft. Other top-level dirs (bin, libsimple, web,
      # deploy) only ever grow, so tar's overwrite semantics are fine.
      rm -rf skill
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
else
    echo "▶ remote install + restart (--only=$ONLY)"
    case "$ONLY" in
        server)
            ssh "$HOST" "set -euo pipefail
              cd /opt/kb
              tar -xzf '/tmp/$ART' --no-same-owner ./bin/ ./libsimple/
              chown -R kb:kb /opt/kb/bin /opt/kb/libsimple
              systemctl restart kb-server
              systemctl is-active --quiet kb-server && echo '  ✓ kb-server active'
              systemctl is-active --quiet kb-web    && echo '  ✓ kb-web active (carried by PartOf=)'
            "
            ;;
        web)
            ssh "$HOST" "set -euo pipefail
              cd /opt/kb
              tar -xzf '/tmp/$ART' --no-same-owner ./web/
              chown -R kb:kb /opt/kb/web
              systemctl restart kb-web
              systemctl is-active --quiet kb-web && echo '  ✓ kb-web active'
            "
            ;;
        skill)
            ssh "$HOST" "set -euo pipefail
              cd /opt/kb
              # Wipe before extract so dropped files (renamed examples,
              # removed mcp.json, etc.) don't linger.
              rm -rf skill
              tar -xzf '/tmp/$ART' --no-same-owner ./skill/
              chown -R kb:kb /opt/kb/skill
              echo '  ✓ skill files refreshed (no restart — kb-server reads /opt/kb/skill at request time)'
            "
            ;;
        configs)
            ssh "$HOST" "set -euo pipefail
              cd /opt/kb
              tar -xzf '/tmp/$ART' --no-same-owner ./deploy/
              chown -R kb:kb /opt/kb/deploy
              ln -sf /opt/kb/deploy/kb-server.service /etc/systemd/system/kb-server.service
              ln -sf /opt/kb/deploy/kb-web.service    /etc/systemd/system/kb-web.service
              ln -sf /opt/kb/deploy/kb.lan.conf       /etc/nginx/sites-available/kb.lan
              ln -sf /etc/nginx/sites-available/kb.lan /etc/nginx/sites-enabled/kb.lan
              systemctl daemon-reload
              nginx -t
              systemctl reload nginx
              echo '  ✓ unit files + nginx vhost reloaded (services not restarted)'
            "
            ;;
    esac
fi

echo "▶ verify via mesh"
ssh "$HOST" "curl -sSk --connect-timeout 5 -o /dev/null -w 'GET https://kb.lan/skill/version → %{http_code}\\n' \
    --resolve kb.lan:443:10.177.0.1 \
    https://kb.lan/skill/version || true"

echo "✓ deployed $TAG → $HOST${ONLY:+  (--only=$ONLY)}"
