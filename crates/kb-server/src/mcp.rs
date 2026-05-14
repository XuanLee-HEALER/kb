//! MCP surface. Streamable HTTP transport mounted at `/mcp`.
//!
//! Exposes 7 tools — write / get / update / deprecate / search / recent / stats.
//! All delegate to the same `crate::store` functions as the REST routes.
//!
//! Each `#[tool(description = …)]` below is the client-side contract: when
//! to call this tool vs. an adjacent one, what the args/returns mean, and
//! what error strings to expect. SKILL.md covers the higher-level question
//! of *whether* to write to the KB at all and *which kind* to choose; this
//! module describes mechanics. The two are designed to be read together
//! (SKILL.md auto-loaded as a skill, tool descriptions loaded with the
//! tool schema).

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
use schemars::{json_schema, JsonSchema, Schema};
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

// `kb_core::WriteInput` flattens an internally-tagged `KindData` enum, which
// schemars renders as a top-level `oneOf`. Anthropic's tool API rejects any
// `input_schema` whose root contains `oneOf` / `allOf` / `anyOf`
// (claude-code error: "input_schema does not support oneOf, allOf, or anyOf
// at the top level"). We keep the serde wire format unchanged but emit a
// flat object schema here; per-kind required fields are documented in the
// `write` tool description and still enforced server-side by serde during
// deserialization.
#[derive(Debug, Deserialize)]
pub struct WriteArgs {
    #[serde(flatten)]
    pub inner: kb_core::WriteInput,
}

impl JsonSchema for WriteArgs {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "WriteArgs".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "description": "Args for the `write` tool. Field requirements vary by `kind` (validated server-side); see the tool description for per-kind required fields.",
            "properties": {
                "title": { "type": "string" },
                "body": { "type": "string", "default": "" },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "default": []
                },
                "source": {
                    "type": "object",
                    "description": "Internally tagged by `type`. Variants: {type:'Human'} | {type:'ClaudeCode', session_id, project, cwd} | {type:'Imported', from, original_date}.",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["Human", "ClaudeCode", "Imported"]
                        },
                        "session_id": { "type": "string" },
                        "project": { "type": "string" },
                        "cwd": { "type": "string" },
                        "from": { "type": "string" },
                        "original_date": { "type": "string", "format": "date-time" }
                    },
                    "required": ["type"]
                },
                "kind": {
                    "type": "string",
                    "enum": ["Fact", "ProblemSolution", "Lesson", "Decision", "Heuristic"],
                    "description": "Discriminator for the kind-specific data fields below."
                },
                "claim": { "type": "string", "description": "Fact: required." },
                "evidence": {
                    "type": "array",
                    "description": "Fact: required. Each item: {kind: 'Output'|'Url'|'SelfVerification', content: string, captured_at: RFC3339}.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "kind": {
                                "type": "string",
                                "enum": ["Output", "Url", "SelfVerification"]
                            },
                            "content": { "type": "string" },
                            "captured_at": { "type": "string", "format": "date-time" }
                        },
                        "required": ["kind", "content", "captured_at"]
                    }
                },
                "problem": { "type": "string", "description": "ProblemSolution: required." },
                "environment": { "type": "string", "description": "ProblemSolution: required." },
                "symptoms": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "ProblemSolution: required."
                },
                "root_cause": { "type": "string", "description": "ProblemSolution: required." },
                "solution": { "type": "string", "description": "ProblemSolution: required." },
                "verification": { "type": "string", "description": "ProblemSolution: optional." },
                "trap": { "type": "string", "description": "Lesson: required." },
                "correction": { "type": "string", "description": "Lesson: required." },
                "why": { "type": "string", "description": "Lesson: required." },
                "context": { "type": "string", "description": "Lesson: optional. Decision: required." },
                "decision": { "type": "string", "description": "Decision: required." },
                "rationale": { "type": "string", "description": "Decision: required." },
                "alternatives": {
                    "type": "array",
                    "description": "Decision: optional. Each item: {option: string, why_not: string}.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "option": { "type": "string" },
                            "why_not": { "type": "string" }
                        },
                        "required": ["option", "why_not"]
                    }
                },
                "tradeoffs": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Decision: optional."
                },
                "problem_class": { "type": "string", "description": "Heuristic: required." },
                "pattern": { "type": "string", "description": "Heuristic: required." },
                "limits": { "type": "string", "description": "Heuristic: optional." },
                "supersedes": {
                    "type": ["string", "null"],
                    "description": "ULID of an existing entry. If set, that entry is atomically deprecated when this one is written."
                },
                "dedup": {
                    "type": "string",
                    "enum": ["check", "force"],
                    "default": "check"
                },
                "proceed_token": {
                    "type": ["string", "null"],
                    "description": "Required when dedup='force'; must match the token from a prior duplicates_found response."
                }
            },
            "required": ["title", "source", "kind"]
        })
    }
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

// MCP outputSchema must have root `"type": "object"`. WriteResult is a
// tagged enum (oneOf), Option<Entry> is anyOf/null, Vec<SearchHit> is an
// array — none of those are objects at the root. Wrap them.

#[derive(Debug, Serialize, JsonSchema)]
pub struct WriteOutput {
    pub result: kb_core::WriteResult,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<kb_core::Entry>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchOutput {
    pub hits: Vec<SearchHit>,
}

// =============================================================================
// tool implementations
// =============================================================================

#[tool_router]
impl KbHandler {
    #[tool(description = r#"Write a new KB entry.

When to call: after the SKILL.md "should I write" filter passes — i.e. the
content is delta knowledge (not in a generic LLM's training data) and the kind
is chosen. Cheap pre-check: run `search` with the title or NK tokens first.

Required: kind (Fact/ProblemSolution/Lesson/Decision/Heuristic), title, source,
and the kind-specific data fields (see Entry schema). `tags` and `body` optional.
`dedup` defaults to 'check'.

Returns one of two `WriteResult.status` values — branch on it:

  - "written"            → success. Use the returned `id` and `version`. Done.
  - "duplicates_found"   → server found Layer-1 (exact NK hash) or Layer-2
                           (FTS5 jieba_query, bm25 weights 10/3/1) matches.
                           The response carries `candidates` (id/kind/title)
                           and a `proceed_token`. Three valid responses:
      a) Same thing, fresh context  → call `update` on the candidate's id.
      b) Similar but distinct       → call `write` again with
                                       dedup="force" + proceed_token, and
                                       reference the related ids as
                                       [[ULID]] in body.
      c) Supersedes an older one    → call `write` with supersedes=<old_id>;
                                       the old entry is atomically deprecated
                                       in the same transaction.

Errors (returned as plain strings):
  - "dedup='force' requires proceed_token"  — passed force without token.
  - "proceed_token mismatch"                — token doesn't match this NK;
                                              re-run with dedup='check' to
                                              get a fresh one.
  - validation/serde errors                 — kind-specific field missing or
                                              wrong type.

Never call with dedup='force' without first getting a proceed_token from a
prior duplicates_found response. That guard exists to prevent accidental
bypass of dedup."#)]
    async fn write(
        &self,
        Parameters(args): Parameters<WriteArgs>,
    ) -> Result<Json<WriteOutput>, String> {
        let pool = self.state.pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = pool.get().map_err(|e| format!("pool: {e}"))?;
            store::write(&mut conn, args.inner).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("join: {e}"))??;
        Ok(Json(WriteOutput { result }))
    }

    #[tool(description = r#"Fetch a full entry by ULID.

When to call: after `search`/`recent` returned a hit (those only carry a
120-char summary, no body, no kind-specific fields); or to chase `[[ULID]]`
references inside another entry's body.

Args: `id` (ULID string). `include_history` is accepted but currently unused
by the server (history is internal-only).

Returns `{ entry: Entry | null }`. Note:
  - Deprecated entries ARE returned (unlike `search`, which excludes them by
    default). Caller should check `entry.deprecated_at` if relevance matters.
  - Body may contain `[[ULID]]` markers — call `get` recursively on those to
    walk the supersede / cross-reference graph.

Errors:
  - ULID parse failure (the `id` is not a valid Crockford-base32 26-char
    ULID) → returned as the underlying DecodeError text."#)]
    async fn get(&self, Parameters(args): Parameters<GetArgs>) -> Result<Json<GetOutput>, String> {
        let id: Ulid = args
            .id
            .parse()
            .map_err(|e: ulid::DecodeError| e.to_string())?;
        let pool = self.state.pool.clone();
        let entry = tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| format!("pool: {e}"))?;
            store::get_by_ulid(&conn, id).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("join: {e}"))??;
        Ok(Json(GetOutput { entry }))
    }

    #[tool(description = r#"Patch an existing entry in place.

When to call: the entry is still the right entry, you just need to refine it
(rephrased title, expanded body, corrected tag, added evidence to a Fact,
etc.). Bumps `version`; the prior snapshot goes into `entry_history`.

When NOT to call:
  - The kind needs to change          → use `write` + `supersedes` instead.
    (Updating across kinds is rejected because it would change the entry's
    identity / natural key.)
  - You're capturing a *different* fact that happens to be related
                                       → use `write` (fresh entry, link via
                                         [[ULID]] in body).
  - The entry is wrong and you have a better one
                                       → use `write` + `supersedes` so the
                                         old one is auto-deprecated.

Args: `id` (ULID) + `partial` object with any subset of {title, body, tags,
plus kind-specific data fields}. Omitted fields keep their old values.

Returns `{ id, version }` (new version number, monotonically increased).

Errors:
  - "kind cannot change on update; supersede instead"  — partial.data had a
    different kind than the existing entry.
  - NotFound  — `id` doesn't match any row.
  - ULID parse failure for `id`."#)]
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

    #[tool(description = r#"Mark an entry as deprecated WITHOUT a successor.

When to call: the entry is no longer correct/relevant and you have no
replacement. The entry stays in the DB (auditability) but is excluded from
`search`/`recent` by default. The `reason` is recorded.

When NOT to call:
  - You have a better/refined version of the same claim
                              → use `write` with `supersedes=<old_id>`. That
                                deprecates the old AND links the new in a
                                single atomic transaction, which is the right
                                shape for "I understand this better now".
  - The entry is salvageable with a tweak
                              → use `update`.

Args: `id` (ULID), `reason` (non-empty string explaining why).

Returns `{ ok: true }`.

Errors:
  - "already deprecated"  — entry is already in deprecated state. Idempotent
                            re-deprecate is rejected on purpose to surface
                            the question of whether you meant `update` or
                            `supersedes`.
  - NotFound, ULID parse failure."#)]
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

    #[tool(description = r#"Search entries. Has two modes, picked by whether `query` is set.

Mode A — Structured (no `query`, or `query` is empty/whitespace):
  Filters by `kinds` + `tag_prefixes`, orders by `updated_at DESC`. No FTS.
  Use when you already know the rough shape ("latest 5 ProblemSolutions tagged
  network/*").

Mode B — FTS (`query` non-empty):
  Runs through libsimple's `simple_query` tokenizer (Chinese + English),
  scores with bm25 weights (5, 5, 1) on (title, body, NK). `kinds` and
  `tag_prefixes` further filter. Orders by score ascending (lower = better
  in SQLite's bm25). Use when you remember keywords but not which entry.

Common args:
  - `kinds`              — restrict to these EntryKinds. Empty = all.
  - `tag_prefixes`       — substring match against the JSON-encoded tags
                           array; works as a cheap facet for path-style tags
                           like "network/wireguard".
  - `limit`              — clamped to [1, 200]. Default 20.
  - `include_deprecated` — default false. Pass true to include deprecated
                           rows (which `get` always returns).

Returns `{ hits: SearchHit[] }`. Each hit carries `id`, `kind`, `title`,
`summary_line` (first 120 bytes of body), `tags`, timestamps, optional
`score` (only in FTS mode), and `deprecated_at` if applicable.

To get the full entry call `get(id)` — `search` deliberately doesn't return
bodies / kind-specific fields to keep token cost low when scanning.

Errors: rusqlite errors as string (FTS syntax issues are rare since
simple_query escapes most input)."#)]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<Json<SearchOutput>, String> {
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
        Ok(Json(SearchOutput { hits }))
    }

    #[tool(description = r#"Most recently updated entries, no query / no filtering.

When to call: catching up on what changed lately, or browsing without a
specific target. For filtered browsing use `search` Mode A instead.

Args:
  - `n`     — clamped to [1, 200]. Default 20.
  - `since` — optional RFC3339 timestamp; only entries with
              `updated_at >= since` are returned.

Returns the same `SearchHit[]` shape as `search` (no `score`). Always
excludes deprecated entries — there is no override here (by design;
recent-deprecated is rarely what you want)."#)]
    async fn recent(
        &self,
        Parameters(args): Parameters<RecentArgs>,
    ) -> Result<Json<SearchOutput>, String> {
        let n = args.n.unwrap_or(20);
        let pool = self.state.pool.clone();
        let hits = tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| format!("pool: {e}"))?;
            store::recent(&conn, n, args.since).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("join: {e}"))??;
        Ok(Json(SearchOutput { hits }))
    }

    #[tool(description = r#"Aggregate counts. No args.

Returns `{ total, active, deprecated, by_kind: [{ kind, active, deprecated }] }`
covering the whole KB.

Use to: sanity-check after a bulk import, decide which kind is under-used,
verify a deprecation went through, etc. Not appropriate for finding specific
entries — use `search` / `recent` for that."#)]
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
