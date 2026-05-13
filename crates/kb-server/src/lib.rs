//! kb-server library entry. Wires storage + HTTP + MCP.
#![deny(unsafe_code)]

pub mod db;
pub mod dedup;
pub mod error;
pub mod libsimple;
pub mod mcp;
pub mod migrations;
pub mod rest;
pub mod skill;
pub mod store;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Debug, Clone)]
pub struct Config {
    pub db_path: PathBuf,
    pub bind: String,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: db::Pool,
    pub bearer_token: Option<Arc<str>>,
    pub skill_dir: Option<PathBuf>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field(
                "bearer_token",
                &self.bearer_token.as_deref().map(|_| "<set>"),
            )
            .field("skill_dir", &self.skill_dir)
            .finish()
    }
}

pub async fn run_server(cfg: Config) -> Result<()> {
    if let Some(parent) = cfg.db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let pool = db::open_pool(&cfg.db_path).context("opening sqlite pool")?;
    migrations::run(&pool).context("running migrations")?;

    let bearer_token = std::env::var("KB_TOKEN").ok().map(Arc::from);
    let skill_dir = std::env::var("KB_SKILL_DIR").ok().map(PathBuf::from);

    let state = AppState {
        pool,
        bearer_token,
        skill_dir,
    };

    let app = Router::new()
        .merge(rest::router(state.clone()))
        .merge(skill::router(state.clone()))
        .merge(mcp::router(state.clone())?)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let addr: SocketAddr = cfg.bind.parse().context("parsing bind address")?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("kb-server listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
