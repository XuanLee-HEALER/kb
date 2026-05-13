#!/usr/bin/env bash
# Stage a release tree under ./release/ that matches what CI produces.
# Assumes:
#   - `cargo build --release` has produced target/release/{kb-server,kb}
#   - `bun run build` in web/ has produced public/js/app.js
set -euo pipefail

cd "$(dirname "$0")/.."

# Locate libsimple install dir (under target/release/build/kb-server-*/out/).
lib_dir=$(find target/release/build -type d -path '*kb-server*/out/libsimple-install/bin' 2>/dev/null | head -1)
if [ -z "$lib_dir" ]; then
    echo "✗ libsimple-install dir not found under target/release/build/" >&2
    echo "   run 'cargo build --release' first" >&2
    exit 1
fi

# Host-native shared lib.
host_lib=$(find "$lib_dir" -maxdepth 1 -name 'libsimple.*' -o -name 'simple.dll' 2>/dev/null | head -1)
if [ -z "$host_lib" ]; then
    echo "✗ libsimple shared library not found in $lib_dir" >&2
    exit 1
fi

if [ ! -f web/public/js/app.js ]; then
    echo "✗ web/public/js/app.js missing — run 'bun run build' in web/" >&2
    exit 1
fi

rm -rf release
mkdir -p release/bin release/libsimple release/web/src release/skill

cp target/release/kb-server release/bin/
cp target/release/kb release/bin/
cp "$host_lib" release/libsimple/
cp -r "$lib_dir/dict" release/libsimple/dict

# Web is a Hono-on-bun app: ship src/ + public/ + package.json. The runtime
# invocation is `bun src/server.tsx`, no build artifact like dist/.
cp -r web/src/. release/web/src/
cp -r web/public release/web/public
cp web/package.json release/web/
cp web/bun.lock release/web/ 2>/dev/null || true
cp web/tsconfig.json release/web/

cp -r skill/. release/skill/
rm -f release/skill/kb-skill.tar.gz

cp README.md release/

cat > release/README.deploy.txt <<'EOF'
kb-server release bundle

Layout
  bin/kb-server               main server binary
  bin/kb                      CLI client
  libsimple/<libsimple.so>    SQLite FTS5 extension (dylib on macOS)
  libsimple/dict/             jieba dictionaries
  web/                        Hono-on-bun web app — `bun src/server.tsx`
    web/src/                  source (TSX, bun compiles in-process)
    web/public/               static assets (CSS, client JS bundle)
    web/package.json          deps for `bun install`
  skill/                      Skill sources (run scripts/build-skill.sh to tarball)

Runtime env
  KB_LIBSIMPLE_DIR=/opt/kb/libsimple
  KB_LIBSIMPLE_DICT=/opt/kb/libsimple/dict
  KB_DB=/var/lib/kb/kb.sqlite
  KB_TOKEN=<your-bearer-token>
  KB_BIND=127.0.0.1:5100
  KB_SKILL_DIR=/opt/kb/skill

Web (Hono on Bun)
  cd web && bun install --frozen-lockfile
  cd web && PORT=5101 KB_URL=http://127.0.0.1:5100 KB_TOKEN=$KB_TOKEN \
    bun src/server.tsx
EOF

host=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)
tar_name="kb-release-${host}-${arch}.tar.gz"
tar -czf "$tar_name" -C release .
echo "✓ ${tar_name} ($(wc -c < "$tar_name" | tr -d ' ') bytes)"
ls -lh "$tar_name"
