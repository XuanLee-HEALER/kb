//! `/skill/*` endpoints. Serves the pre-packaged SKILL.md, mcp.json template,
//! examples/, and a tar.gz bundle of all of the above.

use std::path::PathBuf;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/skill/download", get(download))
        .route("/skill/version", get(version))
        .route("/skill/SKILL.md", get(skill_md))
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
