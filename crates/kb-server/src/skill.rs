//! `/skill/*` endpoints. Serves the pre-packaged SKILL.md, mcp.json template,
//! examples/, a tar.gz bundle of all of the above, and an install plan that
//! Claude Code can consume directly.

use std::collections::BTreeMap;
use std::path::PathBuf;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::{json, Value};

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
    scopes: BTreeMap<&'static str, &'static str>,
    mcp_config: Value,
}

async fn install(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let base = derive_base_url(&headers);

    let version = std::fs::read_to_string(skill_dir(&state).join("VERSION"))
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    let token = state.bearer_token.as_deref().unwrap_or("").to_string();

    let mcp_url = format!("{base}/mcp");
    let mcp_config = json!({
        "mcpServers": {
            "kb": {
                "type": "http",
                "url": mcp_url,
                "headers": {
                    "Authorization": format!("Bearer {token}"),
                },
            },
        },
    });

    let skill_download_url = format!("{base}/skill/download");

    let mut scopes = BTreeMap::new();
    scopes.insert("user", "~/.claude");
    scopes.insert("project", "./.claude");

    let instructions = build_instructions(&skill_download_url);

    Json(InstallPlan {
        version,
        requires_user_confirmation: true,
        instructions,
        skill_download_url,
        mcp_url,
        scopes,
        mcp_config,
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
        "Install the kb skill on this machine. Follow these steps in order.

STEP 0 (MANDATORY — do not skip): Ask the user which install scope they want.
Word it like this:
  \"Install kb at user scope (~/.claude — every project on this machine sees it) \
or project scope (./.claude — only this repo)?\"

Set $TARGET to ~/.claude (user scope) or ./.claude (project scope) based on \
their answer. Do not pick a default; wait for the user to choose.

STEP 1: Download and unpack the skill bundle.
  mkdir -p $TARGET/skills
  curl -fsSL {url} | tar -xz -C $TARGET/skills/

This creates $TARGET/skills/kb-skill/ containing SKILL.md, mcp.json, examples/, \
VERSION. The mcp.json in the tarball has placeholder URL/token values.

STEP 2: Overwrite $TARGET/skills/kb-skill/mcp.json with the `mcp_config` field \
from this response (it already contains the real URL and bearer token). Write \
it with two-space indentation, no trailing whitespace.

STEP 3: Tell the user installation is complete and to restart Claude Code so \
the new MCP server is registered. Mention the scope they chose.

Do not run other commands. Do not edit anything outside $TARGET/skills/kb-skill/. \
Do not echo the bearer token to the user — just write it into mcp.json.",
        url = skill_download_url
    )
}
