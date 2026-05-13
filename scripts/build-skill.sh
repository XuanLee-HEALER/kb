#!/usr/bin/env bash
# Build skill/kb-skill.tar.gz from the contents of skill/ (excluding the tarball itself).
# Stamps VERSION with the current git short hash.
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo_root/skill"

if git -C "$repo_root" rev-parse --short HEAD >/dev/null 2>&1; then
    git -C "$repo_root" rev-parse --short HEAD > VERSION
fi

tar --exclude='kb-skill.tar.gz' -czf kb-skill.tar.gz \
    SKILL.md mcp.json README.md VERSION examples/

echo "built skill/kb-skill.tar.gz $(wc -c < kb-skill.tar.gz | tr -d ' ') bytes"
echo "VERSION = $(cat VERSION)"
