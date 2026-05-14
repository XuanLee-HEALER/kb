//! `/skill/*` endpoints. Serves the pre-packaged SKILL.md, mcp.json template,
//! examples/, a tar.gz bundle of all of the above, and an install plan that
//! Claude Code can consume directly.

use std::path::PathBuf;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/skill/download", get(download))
        .route("/skill/version", get(version))
        .route("/skill/SKILL.md", get(skill_md))
        .route("/skill/install", get(install))
        .with_state(state)
}

fn skill_dir(state: &AppState) -> PathBuf {
    state
        .skill_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("./skill"))
}

async fn version(State(state): State<AppState>) -> impl IntoResponse {
    let path = skill_dir(&state).join("VERSION");
    match std::fs::read_to_string(&path) {
        Ok(s) => (StatusCode::OK, [(header::CONTENT_TYPE, "text/plain")], s).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "no VERSION file").into_response(),
    }
}

async fn skill_md(State(state): State<AppState>) -> impl IntoResponse {
    let path = skill_dir(&state).join("SKILL.md");
    match std::fs::read_to_string(&path) {
        Ok(s) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
            s,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "no SKILL.md").into_response(),
    }
}

async fn download(State(state): State<AppState>) -> impl IntoResponse {
    let path = skill_dir(&state).join("kb-skill.tar.gz");
    match std::fs::read(&path) {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/gzip".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=kb-skill.tar.gz".to_string(),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            "kb-skill.tar.gz not built — run scripts/build-skill.sh",
        )
            .into_response(),
    }
}

// =============================================================================
// install — return a plan for Claude Code to execute on the local machine
// =============================================================================
//
// Auth-free on purpose: kb.lan is etmesh-internal, anything that can reach
// this endpoint already lives inside the trust boundary. The response embeds
// the real bearer token so Claude can drop it straight into mcp.json without
// asking the user.

#[derive(Debug, Serialize)]
struct InstallPlan {
    version: String,
    requires_user_confirmation: bool,
    instructions: String,
    skill_download_url: String,
    mcp_url: String,
    scopes: Vec<ScopeOption>,
}

#[derive(Debug, Serialize)]
struct ScopeOption {
    /// What the user sees / chooses: "user" or "project".
    name: &'static str,
    description: &'static str,
    /// Where to extract the skill bundle.
    skill_dir: &'static str,
    /// The `--scope` value to pass to `claude mcp add`.
    /// (For `name=project` we use `local`, NOT `project`, because `project`
    /// writes `.mcp.json` which is commit-tracked and would leak the token.)
    mcp_cli_scope: &'static str,
    /// The exact shell command Claude should run to register the MCP server.
    mcp_add_command: String,
}

async fn install(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let base = derive_base_url(&headers);

    let version = std::fs::read_to_string(skill_dir(&state).join("VERSION"))
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    let token = state.bearer_token.as_deref().unwrap_or("");
    let mcp_url = format!("{base}/mcp");
    let skill_download_url = format!("{base}/skill/download");

    let mk_cmd = |cli_scope: &str| {
        format!(
            "claude mcp add --transport http kb {mcp_url} \
             --header 'Authorization: Bearer {token}' --scope {cli_scope}"
        )
    };

    let scopes = vec![
        ScopeOption {
            name: "user",
            description: "Every project on this machine can use kb. \
                          Skill lives in ~/.claude/skills; MCP registered \
                          at user scope in ~/.claude.json (not committed).",
            skill_dir: "~/.claude/skills",
            mcp_cli_scope: "user",
            mcp_add_command: mk_cmd("user"),
        },
        ScopeOption {
            name: "project",
            description: "Only this repo. Skill lives in ./.claude/skills \
                          (committable, no secrets); MCP registered at LOCAL \
                          scope in ~/.claude.json under this project's path \
                          (not committed). Never use --scope project — that \
                          writes .mcp.json and would commit the bearer token.",
            skill_dir: "./.claude/skills",
            mcp_cli_scope: "local",
            mcp_add_command: mk_cmd("local"),
        },
    ];

    let instructions = build_instructions(&skill_download_url);

    Json(InstallPlan {
        version,
        requires_user_confirmation: true,
        instructions,
        skill_download_url,
        mcp_url,
        scopes,
    })
}

fn derive_base_url(headers: &HeaderMap) -> String {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("kb.lan");
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_else(|| if host.starts_with("127.0.0.1") || host.starts_with("localhost") {
            "http"
        } else {
            "https"
        });
    format!("{proto}://{host}")
}

fn build_instructions(skill_download_url: &str) -> String {
    format!(
        "Install the kb skill + MCP server on this machine. Follow in order.

STEP 0 (MANDATORY — do not skip): Ask the user which scope they want. Word it \
exactly like this:
  \"Install kb at USER scope (every project on this machine can use kb) \
or PROJECT scope (only this repo)? Either way, the bearer token will not be \
committed to git.\"

Pick the matching entry from this response's `scopes` array based on the \
user's answer. Do not pick a default; wait for them to choose.

STEP 1: Download and unpack the skill bundle.
  mkdir -p <chosen scope's skill_dir>
  curl -fsSL {url} | tar -xz -C <chosen scope's skill_dir>/

This creates <skill_dir>/kb-skill/ with SKILL.md, examples/, VERSION, and a \
placeholder mcp.json. Do NOT modify mcp.json — Claude Code does not read it; \
it's only documentation. The real MCP wiring happens in the next step.

STEP 2: Register the MCP server. This step is idempotent — run it whether \
this is a fresh install or a reinstall/upgrade.

  2a) Run `claude mcp list` and check whether a server named `kb` already \
exists at the chosen scope. If it does, remove it first:
        claude mcp remove kb --scope <chosen scope's mcp_cli_scope>
      `claude mcp add` errors out if `kb` is already registered, so this \
remove step is required for reinstall/upgrade. Skip it on a clean machine.

  2b) Run the chosen scope's `mcp_add_command` shell command verbatim (it \
already has the URL, bearer token, and --scope baked in). The command writes \
into ~/.claude.json:
        - USER scope → top-level mcpServers.kb (loads in every project)
        - PROJECT scope → projects.<this-repo>.mcpServers.kb (loads only here)
Both are kept out of git. The CLI ships with Claude Code; do not edit \
~/.claude.json by hand, and do not use `--scope project` (that would write \
.mcp.json which is commit-tracked).

STEP 3: Tell the user installation is done — name the scope they chose, the \
skill_dir, and that they should restart Claude Code so the new MCP server is \
picked up.

Do not run other commands. Do not echo the bearer token back to the user — \
it's already in the mcp_add_command and they don't need to see it.",
        url = skill_download_url
    )
}
