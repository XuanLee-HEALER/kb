//! MCP surface. Streamable HTTP transport mounted at `/mcp`.
//!
//! Exposes 7 tools — write / get / update / deprecate / search / recent / stats.
//! All delegate to the same `crate::store` functions as the REST routes.

use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
#[allow(unused_imports)]
use rmcp::{tool, tool_handler, tool_router, Json, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::store::{self, PartialEntry, SearchHit, SearchQuery, Stats};
use crate::AppState;

pub fn router(state: AppState) -> Result<Router> {
    let handler = KbHandler {
        state,
        tool_router: KbHandler::tool_router(),
    };
    let service = StreamableHttpService::new(
        move || Ok(handler.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    Ok(Router::new().nest_service("/mcp", service))
}

#[derive(Clone)]
struct KbHandler {
    state: AppState,
    // The `tool_router` field is read by the `#[tool_handler]` macro expansion
    // even though the source-level path looks dead.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

// =============================================================================
// Tool request/response wrappers (rmcp requires JsonSchema).
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteArgs {
    #[serde(flatten)]
    pub inner: kb_core::WriteInput,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetArgs {
    pub id: String,
    #[serde(default)]
    pub include_history: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateArgs {
    pub id: String,
    pub partial: PartialEntry,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeprecateArgs {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct SearchArgs {
    #[serde(default)]
    pub kinds: Vec<kb_core::EntryKind>,
    #[serde(default)]
    pub tag_prefixes: Vec<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub include_deprecated: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct RecentArgs {
    #[serde(default)]
    pub n: Option<u32>,
    #[serde(default)]
    pub since: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateOk {
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeprecateOk {
    pub ok: bool,
}

// =============================================================================
// tool implementations
// =============================================================================

#[tool_router]
impl KbHandler {
    #[tool(
        description = "Write a new KB entry. Layer 1+2 dedup runs unless dedup='force' + matching proceed_token."
    )]
    async fn write(
        &self,
        Parameters(args): Parameters<WriteArgs>,
    ) -> Result<Json<kb_core::WriteResult>, String> {
        let pool = self.state.pool.clone();
        let res = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| format!("pool: {e}"))?;
            store::write(&mut conn, args.inner).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("join: {e}"))??;
        Ok(Json(res))
    }

    #[tool(description = "Fetch a single entry by ULID. Returns even deprecated entries.")]
    async fn get(
        &self,
        Parameters(args): Parameters<GetArgs>,
    ) -> Result<Json<Option<kb_core::Entry>>, String> {
        let id: Ulid = args
            .id
            .parse()
            .map_err(|e: ulid::DecodeError| e.to_string())?;
        let pool = self.state.pool.clone();
        let res = tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| format!("pool: {e}"))?;
            store::get_by_ulid(&conn, id).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("join: {e}"))??;
        Ok(Json(res))
    }

    #[tool(
        description = "Update fields of an existing entry. Version is bumped; old snapshot kept in history."
    )]
    async fn update(
        &self,
        Parameters(args): Parameters<UpdateArgs>,
    ) -> Result<Json<UpdateOk>, String> {
        let id: Ulid = args
            .id
            .parse()
            .map_err(|e: ulid::DecodeError| e.to_string())?;
        let pool = self.state.pool.clone();
        let (uid, ver) = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| format!("pool: {e}"))?;
            store::update(&mut conn, id, args.partial).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("join: {e}"))??;
        Ok(Json(UpdateOk {
            id: uid.to_string(),
            version: ver,
        }))
    }

    #[tool(
        description = "Mark an entry as deprecated. No successor (use kb.write with `supersedes` for that)."
    )]
    async fn deprecate(
        &self,
        Parameters(args): Parameters<DeprecateArgs>,
    ) -> Result<Json<DeprecateOk>, String> {
        let id: Ulid = args
            .id
            .parse()
            .map_err(|e: ulid::DecodeError| e.to_string())?;
        let pool = self.state.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| format!("pool: {e}"))?;
            store::deprecate(&mut conn, id, args.reason).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("join: {e}"))??;
        Ok(Json(DeprecateOk { ok: true }))
    }

    #[tool(
        description = "Search entries. Structured filter (kinds + tag prefixes) + optional FTS query."
    )]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<Json<Vec<SearchHit>>, String> {
        let q = SearchQuery {
            kinds: args.kinds,
            tag_prefixes: args.tag_prefixes,
            query: args.query,
            limit: args.limit.unwrap_or(20),
            include_deprecated: args.include_deprecated.unwrap_or(false),
        };
        let pool = self.state.pool.clone();
        let hits = tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| format!("pool: {e}"))?;
            store::search(&conn, &q).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("join: {e}"))??;
        Ok(Json(hits))
    }

    #[tool(description = "Most recently updated entries.")]
    async fn recent(
        &self,
        Parameters(args): Parameters<RecentArgs>,
    ) -> Result<Json<Vec<SearchHit>>, String> {
        let n = args.n.unwrap_or(20);
        let pool = self.state.pool.clone();
        let hits = tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| format!("pool: {e}"))?;
            store::recent(&conn, n, args.since).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("join: {e}"))??;
        Ok(Json(hits))
    }

    #[tool(
        description = "Aggregate KB stats: total / active / deprecated, and a breakdown by kind."
    )]
    async fn stats(&self) -> Result<Json<Stats>, String> {
        let pool = self.state.pool.clone();
        let s = tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| format!("pool: {e}"))?;
            store::stats(&conn).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("join: {e}"))??;
        Ok(Json(s))
    }
}

#[tool_handler]
impl ServerHandler for KbHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_protocol_version(ProtocolVersion::V_2024_11_05)
        .with_instructions(
            "Personal KB for delta knowledge. See /skill/SKILL.md for write/query best practices."
                .to_string(),
        )
    }
}
