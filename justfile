#!/usr/bin/env -S just --justfile
# kb · task runner. `just` for the recipe list.

set shell := ["bash", "-uc"]
set dotenv-load := true
set ignore-comments := true

# ── Local defaults. Override per-recipe or via .env / shell env. ─────────────
export KB_BIND        := env_var_or_default('KB_BIND',        '127.0.0.1:5100')
export KB_DB          := env_var_or_default('KB_DB',          'data/kb.sqlite')
export KB_TOKEN       := env_var_or_default('KB_TOKEN',       'dev-only-token-change-me')
export KB_URL         := env_var_or_default('KB_URL',         'http://127.0.0.1:5100')
export KB_SKILL_DIR   := env_var_or_default('KB_SKILL_DIR',   justfile_directory() + '/skill')
export RUST_LOG       := env_var_or_default('RUST_LOG',       'info,kb_server=debug')
export PORT           := env_var_or_default('PORT',           '5101')

# ─── meta ───────────────────────────────────────────────────────────────────

# List recipes (default)
default:
    @just --list --unsorted

# Print resolved env so you can sanity-check before `dev`/`serve`.
env:
    @echo "KB_BIND       = $KB_BIND"
    @echo "KB_DB         = $KB_DB"
    @echo "KB_TOKEN      = $KB_TOKEN"
    @echo "KB_URL        = $KB_URL"
    @echo "KB_SKILL_DIR  = $KB_SKILL_DIR"
    @echo "RUST_LOG      = $RUST_LOG"

# ─── setup ──────────────────────────────────────────────────────────────────

# One-time install. Verifies build deps + installs web deps.
setup: _check-deps
    cd web && bun install --frozen-lockfile
    mkdir -p data
    @echo ""
    @echo "✓ setup complete. next: just build  (or just dev)"

_check-deps:
    @command -v cargo  >/dev/null || { echo "✗ cargo not found — install rustup first"; exit 1; }
    @command -v bun    >/dev/null || { echo "✗ bun not found — curl -fsSL https://bun.sh/install | bash"; exit 1; }
    @command -v cmake  >/dev/null || { echo "✗ cmake not found — brew install cmake / apt install cmake"; exit 1; }
    @command -v sqlite3 >/dev/null 2>&1 || echo "⚠  sqlite3 CLI missing — db-shell/db-backup will not work"

# ─── build ──────────────────────────────────────────────────────────────────

# Build everything (release).
build: build-rust build-web

# Build Rust workspace, release profile.
build-rust:
    cargo build --release --workspace

# Build kb-server only.
build-server:
    cargo build --release --bin kb-server

# Build kb CLI only.
build-cli:
    cargo build --release --bin kb

# Build the Astro app (SSR, node adapter).
build-web:
    cd web && bun run build

# Build the Skill tarball (skill/kb-skill.tar.gz) and stamp VERSION.
build-skill:
    bash scripts/build-skill.sh

# Build the full release bundle locally — same shape CI produces.
build-release: build-rust build-web build-skill
    bash scripts/stage-release.sh

# ─── test / lint ────────────────────────────────────────────────────────────

# Run all Rust tests (unit + integration). libsimple is loaded automatically.
test:
    cargo test --workspace --all-targets

# Run a single test by name (substring match).
test-one NAME:
    cargo test --workspace --all-targets -- {{NAME}}

# Lint everything: clippy + biome + astro check.
lint: lint-rust lint-web

lint-rust:
    cargo clippy --workspace --all-targets -- -D warnings

lint-web:
    cd web && bun run check

# Format in place.
fmt: fmt-rust fmt-web
fmt-rust:
    cargo fmt --all
fmt-web:
    cd web && bunx biome format --write src

# Verify formatting + lint without writing changes (mirrors CI).
fmt-check:
    cargo fmt --all -- --check
    cd web && bunx biome check src

# Auto-fix what's auto-fixable in both languages.
fix:
    cargo clippy --workspace --all-targets --fix --allow-dirty --allow-staged
    cd web && bunx biome check --write src

# Full CI mirror: fmt-check + lint + test + web build.
check: fmt-check lint test build-web
    @echo ""
    @echo "✓ all checks pass — matches what CI runs"

# ─── dev ────────────────────────────────────────────────────────────────────

# Run kb-server + web in parallel. Ctrl+C terminates both.
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'jobs -p | xargs -r kill 2>/dev/null || true' EXIT INT TERM
    mkdir -p data
    echo "▶ kb-server  → http://${KB_BIND}"
    cargo run --bin kb-server &
    server_pid=$!
    sleep 1.5
    echo "▶ astro dev   → http://127.0.0.1:${PORT}"
    (cd web && bun run dev --port "${PORT}") &
    web_pid=$!
    wait "$server_pid" "$web_pid"

# Run only kb-server (debug build; auto-reloads if cargo-watch is installed).
dev-server:
    mkdir -p data
    @if command -v cargo-watch >/dev/null 2>&1; then \
        cargo watch -x 'run --bin kb-server'; \
    else \
        cargo run --bin kb-server; \
    fi

# Run only the Astro dev server. kb-server must already be running.
dev-web:
    cd web && bun run dev --port "${PORT}"

# Run the release binary locally (assumes `just build` has been run).
serve:
    mkdir -p data
    ./target/release/kb-server

# Run the Astro production build locally.
serve-web:
    cd web && bun ./dist/server/entry.mjs

# ─── kb CLI ────────────────────────────────────────────────────────────────

# Invoke the kb CLI against the local server. e.g. `just kb stats`
kb *ARGS:
    cargo run --quiet --release --bin kb -- {{ARGS}}

# Quick smoke test of the local server: stats + recent
smoke:
    @just kb stats || true
    @echo ""
    @just kb recent --n=5 || true

# ─── DB ─────────────────────────────────────────────────────────────────────

# Make sure data/ exists. The server creates the SQLite file on first start.
db-init:
    mkdir -p data
    @test -f "$KB_DB" \
        && echo "✓ db exists at $KB_DB" \
        || echo "✓ data/ ready — kb-server will create $KB_DB on first start"

# Online .backup (safe with WAL — no shutdown needed).
db-backup OUT="data/kb-backup-$(date +%Y%m%d-%H%M%S).sqlite":
    sqlite3 "$KB_DB" ".backup '{{OUT}}'"
    @echo "✓ backed up to {{OUT}}"

# Open the live db in sqlite3 (read-only — won't corrupt a running server).
db-shell:
    sqlite3 -readonly "$KB_DB"

# Quick dump of all entries as JSON (id / kind / title / NK), most recent first.
db-dump:
    sqlite3 "$KB_DB" "SELECT json_object('id', ulid, 'kind', kind, 'title', title, 'nk', natural_key_text) FROM entries ORDER BY rowid DESC;"

# Show table sizes.
db-stats:
    sqlite3 "$KB_DB" "SELECT 'entries' AS tbl, COUNT(*) AS n FROM entries UNION ALL SELECT 'entry_history', COUNT(*) FROM entry_history UNION ALL SELECT 'entries_fts', COUNT(*) FROM entries_fts;"

# ─── Skill ──────────────────────────────────────────────────────────────────

# Print the current skill VERSION.
skill-version:
    @cat skill/VERSION

# Serve the skill bundle locally for sanity-checking the /skill/download flow.
skill-preview: build-skill
    @echo "Skill bundle at: $(pwd)/skill/kb-skill.tar.gz"
    @ls -lh skill/kb-skill.tar.gz

# ─── clean ──────────────────────────────────────────────────────────────────

# Remove all build artifacts (target, dist, .astro, release/, skill tarball).
clean:
    cargo clean
    rm -rf web/dist web/.astro
    rm -f skill/kb-skill.tar.gz
    rm -rf release

# Force libsimple to recompile on next cargo build (its build.rs is in kb-server).
libsimple-clean:
    cargo clean -p kb-server

# clean + remove node_modules.
deep-clean: clean
    rm -rf web/node_modules

# ─── release / devops ──────────────────────────────────────────────────────

# Cut a release tag and push it. CI will pick it up and build the artifact.
#   just release-tag v0.2.0
release-tag VERSION:
    @test -z "$$(git status --porcelain)" || { echo "✗ working tree dirty"; exit 1; }
    git tag -a "{{VERSION}}" -m "release {{VERSION}}"
    git push origin "{{VERSION}}"
    @echo "✓ tag {{VERSION}} pushed — release workflow will build the bundle"

# Watch the latest GitHub Actions release run.
release-watch:
    gh run watch $$(gh run list --workflow=release.yml --limit=1 --json databaseId --jq '.[0].databaseId')

# List recent GitHub Actions runs across all workflows.
ci-status:
    gh run list --limit=10
