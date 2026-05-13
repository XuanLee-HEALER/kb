# Deploy runbook · aliyun ECS (etmesh hub)

Target: `aliyun` (10.177.0.1 over etmesh, public 8.140.18.212) · Ubuntu 24.04.

Architecture:
```
mesh client  →  https://kb.lan  →  nginx (10.177.0.1:443, TLS termination)
                                    ├─ /mcp     →  kb-server  (127.0.0.1:5100)  [bearer-gated]
                                    └─ /        →  kb-web      (127.0.0.1:5101)
```

**TLS chain of trust** — root CA lives as `Secret cert-manager/etmesh-root-ca`
in the archmbp k8s cluster; `ClusterIssuer etmesh-ca` signs leaf certs. The
kb.lan leaf is requested via `deploy/k8s/kb-lan-cert.yaml` and copied out to
aliyun nginx in Phase 1. Same pattern as all other .lan services in your mesh.

The deploy is split into three phases:

| Phase | Where | When | Mode |
|---|---|---|---|
| **0** Local prep | archmbp (k8s) | one-time + on cert rotation | manual |
| **1** Remote provision | aliyun | one-time | manual |
| **2** Rolling deploy | from local | each release | `scripts/deploy-aliyun.sh <tag>` |

The split is deliberate: phase 0/1 contain everything that can plausibly fail
(network, signing, user creation, package install). Phase 2 is mechanical and
idempotent — extract, symlink, restart.

---

## Phase 0 · Local prep (archmbp)

Done once. The leaf cert auto-renews via cert-manager — re-export + scp only
when the underlying secret rotates (cert-manager bumps it ~30d before expiry).

### 0.1 Issue the `kb.lan` leaf cert via cert-manager

The root CA lives as `Secret cert-manager/etmesh-root-ca`, fronted by
`ClusterIssuer etmesh-ca` (already deployed in the k8s cluster on archmbp).
cert-manager handles keygen, signing, and renewal. No openssl, no on-disk
private key.

```sh
# from archmbp
kubectl apply -f deploy/k8s/kb-lan-cert.yaml
# wait for cert-manager to provision (usually <5s)
kubectl wait --for=condition=Ready --timeout=60s certificate/kb-lan-tls
```

Verify and extract the leaf to local files:

```sh
kubectl get certificate kb-lan-tls -o jsonpath='{.status.conditions}' | jq .
kubectl get secret kb-lan-tls -o jsonpath='{.data.tls\.crt}' | base64 -d > kb.lan.crt
kubectl get secret kb-lan-tls -o jsonpath='{.data.tls\.key}' | base64 -d > kb.lan.key
chmod 600 kb.lan.key

# Sanity
openssl x509 -in kb.lan.crt -noout -subject -issuer -dates -ext subjectAltName
```

You'll scp these in Phase 1.6.

### 0.2 Cut a release

```sh
just release-tag v0.1.0          # CI builds the linux-x86_64 artifact
```

Wait for the `release.yml` workflow to finish; the artifact will appear under
GitHub Releases as `kb-release-linux-x86_64.tar.gz`.

---

## Phase 1 · Remote provision (aliyun)

Done once. Steps that need network (`apt install`, `gh release download`)
can use the in-box sing-box proxy if needed:

```sh
# inside the ssh aliyun session, if direct outbound is flaky:
export https_proxy=http://127.0.0.1:7890 http_proxy=http://127.0.0.1:7890
```

### 1.1 System user + dirs

```sh
ssh aliyun
useradd -r -s /usr/sbin/nologin -d /var/lib/kb -m kb
mkdir -p /opt/kb /etc/kb /var/backups/kb /etc/nginx/ssl /etc/nginx/snippets
chown kb:kb /var/lib/kb /var/backups/kb
chown root:kb /etc/kb           # so kb user can traverse and read /etc/kb/env
chmod 750 /etc/kb
```

### 1.2 Share the existing bun runtime

The box already has bun at `/root/.bun/bin/bun` (used by etmesh-dashboard).
Make it readable by the kb user:

```sh
install -m 755 /root/.bun/bin/bun /usr/local/bin/bun
/usr/local/bin/bun --version
```

### 1.3 sqlite3 (for daily backup tooling)

```sh
apt install -y sqlite3
```

### 1.4 Token + env file

```sh
KB_TOKEN=$(openssl rand -hex 32)
cat > /etc/kb/env <<EOF
KB_TOKEN=${KB_TOKEN}
KB_DB=/var/lib/kb/kb.sqlite
KB_BIND=127.0.0.1:5100
KB_LIBSIMPLE_DIR=/opt/kb/libsimple
KB_LIBSIMPLE_DICT=/opt/kb/libsimple/dict
KB_SKILL_DIR=/opt/kb/skill
KB_URL=http://127.0.0.1:5100
PORT=5101
EOF
chown root:kb /etc/kb/env
chmod 640 /etc/kb/env

# echo the token; you'll paste it into nginx snippet + skill mcp.json on each client.
echo "TOKEN: ${KB_TOKEN}"
```

### 1.5 nginx auth snippet (holds the bearer secret out of git)

```sh
cat > /etc/nginx/snippets/kb-auth.conf <<EOF
if (\$http_authorization != "Bearer ${KB_TOKEN}") { return 401; }
EOF
chmod 640 /etc/nginx/snippets/kb-auth.conf
chown root:www-data /etc/nginx/snippets/kb-auth.conf
```

### 1.6 Ship the leaf cert from archmbp

From archmbp:
```sh
scp kb.lan.crt aliyun:/etc/nginx/ssl/kb.lan.crt
scp kb.lan.key aliyun:/etc/nginx/ssl/kb.lan.key
```

Then on aliyun:
```sh
chown root:root /etc/nginx/ssl/kb.lan.*
chmod 644 /etc/nginx/ssl/kb.lan.crt
chmod 600 /etc/nginx/ssl/kb.lan.key
```

### 1.7 DNS — explicit A record overriding the wildcard

```sh
# backup + bump serial + inject A record
cp /etc/coredns/zones/lan.db /etc/coredns/zones/lan.db.bak.$(date +%Y%m%d-%H%M%S)
sed -i -E 's/(2026[0-9]{6})([0-9]{2})/echo \1$(printf "%02d" $((10#\2 + 1)))/e' /etc/coredns/zones/lan.db
# Add: kb IN A 10.177.0.1 (after the aliyun record)
sed -i -E '/^aliyun /a kb          IN A   10.177.0.1' /etc/coredns/zones/lan.db
systemctl reload coredns

# Verify
dig @10.177.0.1 +short kb.lan
# → 10.177.0.1
```

### 1.8 Sanity-check before first deploy

```sh
id kb                                 # user exists
ls -la /etc/kb/env                    # 640 root:kb
ls -la /etc/nginx/ssl/kb.lan.*        # 644 / 600 root:root
ls -la /etc/nginx/snippets/kb-auth.conf
/usr/local/bin/bun --version          # bun reachable
dig @10.177.0.1 +short kb.lan         # → 10.177.0.1
```

---

## Phase 2 · Deploy / rolling update

From archmbp:

```sh
just deploy v0.1.0
# wraps scripts/deploy-aliyun.sh — gh release download + scp + ssh extract + restart
```

What the script does (and nothing more):

1. `gh release download <tag>` → `.tmp/kb-release-linux-x86_64.tar.gz`
2. `scp` artifact to aliyun:/tmp/
3. ssh and:
   - `tar -xzf` over `/opt/kb` (overlays binaries + web/ + libsimple/ + skill/ + deploy/)
   - `chown -R kb:kb /opt/kb`
   - symlink `deploy/kb-server.service`, `deploy/kb-web.service`, `deploy/kb.lan.conf` into /etc/systemd / /etc/nginx
   - `systemctl daemon-reload && restart kb-server kb-web`
   - `nginx -t && systemctl reload nginx`

No `apt install`, no `bun install`, no compilation, no key generation. All
side effects from phase 0/1 are reused.

---

## Phase 3 · Per-client setup

Done once per machine (archmbp, arch, macmini).

### Trust the root CA

```sh
# macOS
sudo security add-trusted-cert -d -r trustRoot \
    -k /Library/Keychains/System.keychain etmesh-root-ca.pem

# Arch
sudo trust anchor --store etmesh-root-ca.pem
```

### DNS resolver

Each machine should use `10.177.0.1` as its DNS resolver for `.lan` (already
the case if you've onboarded other lan services). Verify:

```sh
dig @10.177.0.1 +short kb.lan       # → 10.177.0.1
curl --resolve kb.lan:443:10.177.0.1 -s https://kb.lan/api/search?limit=1 \
    -H "Authorization: Bearer <token>"
```

### Claude Code MCP

Edit each client's `.claude/settings.local.json` or `~/.claude/skills/kb-skill/mcp.json`:

```json
{
  "mcpServers": {
    "kb": {
      "type": "http",
      "url": "https://kb.lan/mcp",
      "headers": { "Authorization": "Bearer <token from /etc/kb/env on aliyun>" }
    }
  }
}
```

Restart Claude Code; verify `kb.*` tools appear.

---

## Phase 4 · Backup

Daily WAL-safe sqlite backup:

```sh
# on aliyun, one-time
cat > /etc/cron.daily/kb-backup <<'EOF'
#!/bin/sh
set -e
DEST=/var/backups/kb/kb-$(date +%Y%m%d).sqlite
sudo -u kb sqlite3 /var/lib/kb/kb.sqlite ".backup ${DEST}"
find /var/backups/kb/ -mtime +30 -delete
EOF
chmod +x /etc/cron.daily/kb-backup
```

---

## Rollback

If a deploy goes wrong:

```sh
# aliyun
systemctl stop kb-web kb-server
# previous release tarball lives in /tmp/ (assuming you didn't clean it)
cd /opt/kb && tar -xzf /tmp/kb-release-linux-x86_64.tar.gz   # use the previous version's tarball
chown -R kb:kb /opt/kb
systemctl start kb-server kb-web
```

Or restore from sqlite backup:

```sh
systemctl stop kb-server
sudo -u kb cp /var/backups/kb/kb-YYYYMMDD.sqlite /var/lib/kb/kb.sqlite
systemctl start kb-server
```
