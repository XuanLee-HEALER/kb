#!/usr/bin/env bash
# Build skill/kb-skill/kb-skill.tar.gz from the contents of skill/kb-skill/.
# Stamps VERSION with the current git short hash.
#
# Tarball entries are prefixed with kb-skill/ so that
#   tar xz -C ~/.claude/skills/
# extracts to ~/.claude/skills/kb-skill/* without polluting the parent
# directory with other skills.
#
# Sibling skills (e.g. skill/define-skill/) are NOT built or distributed
# by this script — kb-server's /skill/download endpoint serves kb-skill
# only. Add a parallel build target if/when a sibling needs distribution.
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo_root/skill/kb-skill"

if git -C "$repo_root" rev-parse --short HEAD >/dev/null 2>&1; then
    git -C "$repo_root" rev-parse --short HEAD > VERSION
fi

# Stage into a temp dir under kb-skill/ so tar produces prefixed entries.
# This avoids the GNU --transform vs BSD -s portability headache when this
# script runs both locally (macOS, bsdtar) and in CI (ubuntu, GNU tar).
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT

mkdir -p "$staging/kb-skill"
cp SKILL.md VERSION "$staging/kb-skill/"
cp -r examples "$staging/kb-skill/"

tar -czf kb-skill.tar.gz -C "$staging" kb-skill

echo "built skill/kb-skill/kb-skill.tar.gz $(wc -c < kb-skill.tar.gz | tr -d ' ') bytes"
echo "VERSION = $(cat VERSION)"
