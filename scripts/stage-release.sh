#!/usr/bin/env bash
# Stage a release tree under ./release/ and tar it.
#
# Assumes:
#   - `cargo build --release` has produced target/release/{kb-server,kb}
#   - `bun run build` in web/ has produced public/js/app.js
#   - `bun install --frozen-lockfile` in web/ has populated node_modules/
#     (so the deploy target never needs network for npm/bun-install)
set -euo pipefail

cd "$(dirname "$0")/.."

# ── locate libsimple build artifacts
lib_dir=$(find target/release/build -type d -path '*kb-server*/out/libsimple-install/bin' 2>/dev/null | head -1)
if [ -z "$lib_dir" ]; then
    echo "✗ libsimple-install dir not found under target/release/build/" >&2
    echo "   run 'cargo build --release' first" >&2
    exit 1
fi

host_lib=$(find "$lib_dir" -maxdepth 1 -name 'libsimple.*' -o -name 'simple.dll' 2>/dev/null | head -1)
if [ -z "$host_lib" ]; then
    echo "✗ libsimple shared library not found in $lib_dir" >&2
    exit 1
fi

if [ ! -f web/public/js/app.js ]; then
    echo "✗ web/public/js/app.js missing — run 'bun run build' in web/" >&2
    exit 1
fi

if [ ! -d web/node_modules ]; then
    echo "✗ web/node_modules missing — run 'bun install --frozen-lockfile' in web/" >&2
    echo "  (the release bundle ships node_modules so the deploy target needs no network)" >&2
    exit 1
fi

# ── stage
rm -rf release
mkdir -p release/bin release/libsimple release/web/src release/skill release/deploy

cp target/release/kb-server release/bin/
cp target/release/kb release/bin/
cp "$host_lib" release/libsimple/
cp -r "$lib_dir/dict" release/libsimple/dict

# Web app — source + static assets + pre-installed deps (no network needed at deploy).
cp -r web/src/. release/web/src/
cp -r web/public release/web/public
cp -r web/node_modules release/web/node_modules
cp web/package.json release/web/
cp web/bun.lock release/web/ 2>/dev/null || true
cp web/tsconfig.json release/web/

# Systemd units + nginx vhost. Symlinked into /etc by deploy-aliyun.sh.
cp -r deploy/. release/deploy/

# Skill sources + the pre-built tarball. kb-server's /skill/download
# endpoint reads kb-skill.tar.gz directly off disk; without it that
# endpoint 404s and `claude mcp add` via /skill/install can't bootstrap.
#
# Deployed layout on aliyun is flat (`/opt/kb/skill/{SKILL.md,VERSION,…}`)
# so we read from the kb-skill subdir, NOT skill/ root — define-skill is
# version-controlled but not part of the kb-server distribution surface.
cp -r skill/kb-skill/. release/skill/

# Sediment hook script — kb-server serves it at /skill/hook/sediment.sh
# (KB_SEDIMENT_HOOK_DIR defaults to ./hooks/sediment relative to the server's
# working dir, which is /opt/kb on the deploy target).
mkdir -p release/hooks/sediment
cp -r hooks/sediment/. release/hooks/sediment/

cp README.md release/

cat > release/README.deploy.txt <<'EOF'
kb-server release bundle

Layout
  bin/kb-server               Rust server binary (kb-server)
  bin/kb                      CLI client
  libsimple/<libsimple.so>    SQLite FTS5 extension (dylib on macOS, .so on linux)
  libsimple/dict/             jieba dictionaries
  web/                        Hono-on-bun web app — `bun src/server.tsx`
    web/src/                  source TSX (bun compiles in-process)
    web/public/               static assets (CSS, client JS bundle)
    web/node_modules/         pre-installed deps (no network at deploy)
    web/package.json
  skill/                      kb-skill sources + pre-built tarball
  hooks/sediment/             sediment hook script — served at /skill/hook/sediment.sh
  deploy/                     systemd units + nginx vhost (symlinked into /etc by deploy script)

For deployment instructions see deploy/README.md in the source tree.
EOF

host=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)
tar_name="kb-release-${host}-${arch}.tar.gz"
tar -czf "$tar_name" -C release .
echo "✓ ${tar_name} ($(wc -c < "$tar_name" | tr -d ' ') bytes)"
ls -lh "$tar_name"
