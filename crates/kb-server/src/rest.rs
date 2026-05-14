//! REST HTTP surface. Mirrors the MCP `kb.*` methods 1:1.
//!
//! Routes (all rooted at `/api`):
//!   POST   /entries              kb.write
//!   GET    /entries/:ulid        kb.get
//!   PATCH  /entries/:ulid        kb.update
//!   DELETE /entries/:ulid        kb.purge (admin — not exposed via MCP)
//!   POST   /entries/:ulid/deprecate
//!   GET    /search               kb.search (query string params)
//!   POST   /search               kb.search (json body — supports kinds[], tag_prefixes[], etc.)
//!   POST   /batch_search         kb.batch_search (json body — { queries: SearchQuery[] }, max 10)
//!   GET    /recent
//!   GET    /stats

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use kb_core::{Entry, WriteInput, WriteResult};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::error::{KbError, KbResult};
use crate::store::{
    self, BatchSearchOutput, PartialEntry, PurgeSummary, SearchHit, SearchQuery, Stats,
};
use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/entries", post(post_entry))
        .route(
            "/api/entries/{ulid}",
            get(get_entry).patch(patch_entry).delete(delete_entry),
        )
        .route("/api/entries/{ulid}/deprecate", post(post_deprecate))
        .route("/api/search", get(get_search).post(post_search))
        .route("/api/batch_search", post(post_batch_search))
        .route("/api/recent", get(get_recent))
        .route("/api/stats", get(get_stats))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// auth: optional bearer token check on every `/api/*` request.
// ---------------------------------------------------------------------------

fn check_auth(state: &AppState, headers: &HeaderMap) -> KbResult<()> {
    let Some(expected) = state.bearer_token.as_deref() else {
        return Ok(());
    };
    let got = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    if got == Some(expected) {
        Ok(())
    } else {
        Err(KbError::Unauthorized)
    }
}

// ---------------------------------------------------------------------------
// handlers
// ---------------------------------------------------------------------------

async fn post_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<WriteInput>,
) -> KbResult<Json<WriteResult>> {
    check_auth(&state, &headers)?;
    let pool = state.pool.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool.get()?;
        store::write(&mut conn, input)
    })
    .await
    .map_err(|e| KbError::Other(anyhow::anyhow!("join error: {e}")))??;
    Ok(Json(result))
}

async fn get_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ulid): Path<String>,
) -> KbResult<Json<Entry>> {
    check_auth(&state, &headers)?;
    let id: Ulid = ulid.parse()?;
    let pool = state.pool.clone();
    let entry = tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        store::get_by_ulid(&conn, id)
    })
    .await
    .map_err(|e| KbError::Other(anyhow::anyhow!("join error: {e}")))??
    .ok_or(KbError::NotFound)?;
    Ok(Json(entry))
}

async fn patch_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ulid): Path<String>,
    Json(partial): Json<PartialEntry>,
) -> KbResult<Json<UpdateResponse>> {
    check_auth(&state, &headers)?;
    let id: Ulid = ulid.parse()?;
    let pool = state.pool.clone();
    let (id, version) = tokio::task::spawn_blocking(move || {
        let mut conn = pool.get()?;
        store::update(&mut conn, id, partial)
    })
    .await
    .map_err(|e| KbError::Other(anyhow::anyhow!("join error: {e}")))??;
    Ok(Json(UpdateResponse { id, version }))
}

#[derive(Debug, Serialize)]
pub struct UpdateResponse {
    pub id: Ulid,
    pub version: u32,
}

#[derive(Debug, Deserialize)]
pub struct DeprecateBody {
    pub reason: String,
}

async fn post_deprecate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ulid): Path<String>,
    Json(body): Json<DeprecateBody>,
) -> KbResult<StatusCode> {
    check_auth(&state, &headers)?;
    let id: Ulid = ulid.parse()?;
    let pool = state.pool.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get()?;
        store::deprecate(&mut conn, id, body.reason)
    })
    .await
    .map_err(|e| KbError::Other(anyhow::anyhow!("join error: {e}")))??;
    Ok(StatusCode::NO_CONTENT)
}

// purge

#[derive(Debug, Deserialize, Default)]
pub struct PurgeParams {
    #[serde(default)]
    pub dry_run: Option<bool>,
}

async fn delete_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(ulid): Path<String>,
    Query(p): Query<PurgeParams>,
) -> KbResult<Json<PurgeSummary>> {
    check_auth(&state, &headers)?;
    let id: Ulid = ulid.parse()?;
    let dry_run = p.dry_run.unwrap_or(false);
    let pool = state.pool.clone();
    let summary = tokio::task::spawn_blocking(move || {
        let mut conn = pool.get()?;
        store::purge(&mut conn, id, dry_run)
    })
    .await
    .map_err(|e| KbError::Other(anyhow::anyhow!("join error: {e}")))??;
    Ok(Json(summary))
}

// search

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub include_deprecated: Option<bool>,
}

async fn get_search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<SearchParams>,
) -> KbResult<Json<Vec<SearchHit>>> {
    check_auth(&state, &headers)?;
    let q = SearchQuery {
        kinds: p
            .kind
            .as_deref()
            .map(|s| {
                s.split(',')
                    .filter_map(|k| match k.trim() {
                        "Fact" => Some(kb_core::EntryKind::Fact),
                        "ProblemSolution" => Some(kb_core::EntryKind::ProblemSolution),
                        "Lesson" => Some(kb_core::EntryKind::Lesson),
                        "Decision" => Some(kb_core::EntryKind::Decision),
                        "Heuristic" => Some(kb_core::EntryKind::Heuristic),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        tag_prefixes: p
            .tag
            .as_deref()
            .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
            .unwrap_or_default(),
        query: p.q,
        limit: p.limit.unwrap_or(20),
        include_deprecated: p.include_deprecated.unwrap_or(false),
    };
    let pool = state.pool.clone();
    let hits = tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        store::search(&conn, &q)
    })
    .await
    .map_err(|e| KbError::Other(anyhow::anyhow!("join error: {e}")))??;
    Ok(Json(hits))
}

async fn post_search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(q): Json<SearchQuery>,
) -> KbResult<Json<Vec<SearchHit>>> {
    check_auth(&state, &headers)?;
    let pool = state.pool.clone();
    let hits = tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        store::search(&conn, &q)
    })
    .await
    .map_err(|e| KbError::Other(anyhow::anyhow!("join error: {e}")))??;
    Ok(Json(hits))
}

#[derive(Debug, Deserialize)]
pub struct BatchSearchBody {
    pub queries: Vec<SearchQuery>,
}

async fn post_batch_search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<BatchSearchBody>,
) -> KbResult<Json<BatchSearchOutput>> {
    check_auth(&state, &headers)?;
    if body.queries.len() > crate::mcp::BATCH_SEARCH_MAX_QUERIES {
        return Err(KbError::BadRequest(format!(
            "too many queries; max {}, got {}",
            crate::mcp::BATCH_SEARCH_MAX_QUERIES,
            body.queries.len()
        )));
    }
    let pool = state.pool.clone();
    let out = tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        store::batch_search(&conn, &body.queries)
    })
    .await
    .map_err(|e| KbError::Other(anyhow::anyhow!("join error: {e}")))??;
    Ok(Json(out))
}

#[derive(Debug, Deserialize)]
pub struct RecentParams {
    #[serde(default)]
    pub n: Option<u32>,
    #[serde(default)]
    pub since: Option<DateTime<Utc>>,
}

async fn get_recent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<RecentParams>,
) -> KbResult<Json<Vec<SearchHit>>> {
    check_auth(&state, &headers)?;
    let n = p.n.unwrap_or(20);
    let pool = state.pool.clone();
    let hits = tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        store::recent(&conn, n, p.since)
    })
    .await
    .map_err(|e| KbError::Other(anyhow::anyhow!("join error: {e}")))??;
    Ok(Json(hits))
}

async fn get_stats(State(state): State<AppState>, headers: HeaderMap) -> KbResult<Json<Stats>> {
    check_auth(&state, &headers)?;
    let pool = state.pool.clone();
    let s = tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        store::stats(&conn)
    })
    .await
    .map_err(|e| KbError::Other(anyhow::anyhow!("join error: {e}")))??;
    Ok(Json(s))
}
