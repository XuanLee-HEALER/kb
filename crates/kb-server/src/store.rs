//! Entry storage: CRUD on `entries` + `entry_history`, plus search.
//!
//! All functions take a [`Conn`] (pooled rusqlite connection) and run
//! synchronously. Async handlers must wrap calls in `tokio::task::spawn_blocking`.
//!
//! [`Conn`]: crate::db::Conn

use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};
use kb_core::{
    natural_key, Entry, EntryKind, KindData, NaturalKey, Source, WriteInput, WriteResult,
};
use rusqlite::{params, OptionalExtension, Row, Transaction};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::Conn;
use crate::dedup::{compute_proceed_token, run_dedup, DedupOutcome};
use crate::error::{KbError, KbResult};

const SUMMARY_LEN: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchQuery {
    #[serde(default)]
    pub kinds: Vec<EntryKind>,
    #[serde(default)]
    pub tag_prefixes: Vec<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub include_deprecated: bool,
}

const fn default_limit() -> u32 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchHit {
    #[schemars(with = "String")]
    pub id: Ulid,
    pub kind: EntryKind,
    pub title: String,
    pub summary_line: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deprecated_at: Option<DateTime<Utc>>,
    pub score: Option<f64>,
}

// batch_search result types. See `batch_search()` for semantics.

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchSearchHit {
    #[serde(flatten)]
    pub hit: SearchHit,
    /// 0-based indices into the input `queries` array that returned this entry.
    /// `len() > 1` means multiple sub-queries hit the same entry — strong signal.
    pub matched_queries: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchSearchError {
    pub query_index: u32,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchSearchOutput {
    pub hits: Vec<BatchSearchHit>,
    pub errors: Vec<BatchSearchError>,
}

// candidates pool — raw, unrefined inbox; rows live only while pending. See
// `deposit_candidate` / `list_candidates` / `promote_candidate` / `discard_candidate`.

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    #[schemars(with = "String")]
    pub id: Ulid,
    pub content: String,
    pub source: Source,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct CandidateFilter {
    #[serde(default)]
    pub since: Option<DateTime<Utc>>,
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Server-side cap on candidate `content` size (bytes). Hooks should deposit
/// short observations, not full transcripts.
pub const CANDIDATE_MAX_CONTENT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct PartialEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", flatten)]
    pub data: Option<KindData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Stats {
    pub total: i64,
    pub active: i64,
    pub deprecated: i64,
    pub by_kind: Vec<KindCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KindCount {
    pub kind: EntryKind,
    pub active: i64,
    pub deprecated: i64,
}

// =============================================================================
// write
// =============================================================================

pub fn write(conn: &mut Conn, input: WriteInput) -> KbResult<WriteResult> {
    let tx = conn.transaction()?;
    let r = write_in_tx(&tx, input)?;
    tx.commit()?;
    Ok(r)
}

/// Core write logic without commit. Promote-candidate uses this to attach
/// a `DELETE FROM candidates` into the same transaction.
///
/// On `DuplicatesFound` the caller should drop the transaction (rollback);
/// no rows were inserted but cleaning up before returning is the caller's
/// responsibility.
pub(crate) fn write_in_tx(tx: &Transaction<'_>, input: WriteInput) -> KbResult<WriteResult> {
    let nk = natural_key(&input.data);
    let kind = input.data.kind();

    // Layer 1+2 dedup, unless the caller passed dedup='force' + matching token.
    if matches!(input.dedup, kb_core::DedupMode::Check) {
        let outcome = run_dedup(tx, kind, &nk)?;
        if let DedupOutcome::Found {
            candidates,
            proceed_token,
        } = outcome
        {
            return Ok(WriteResult::DuplicatesFound {
                candidates,
                proceed_token,
            });
        }
    } else {
        // dedup='force' requires a proceed_token that matches the NK hash; this
        // prevents accidental force-writes that bypass dedup. For absolutely
        // greenfield writes the client can pass dedup='force' with the token
        // it received from the previous duplicates_found response.
        let token = input
            .proceed_token
            .as_deref()
            .ok_or_else(|| KbError::BadRequest("dedup='force' requires proceed_token".into()))?;
        let expected = compute_proceed_token(&nk);
        if token != expected {
            return Err(KbError::BadRequest("proceed_token mismatch".into()));
        }
    }

    let now = Utc::now();
    let id = Ulid::new();
    let entry = Entry {
        id,
        title: input.title.clone(),
        body: input.body.clone(),
        tags: input.tags.clone(),
        source: input.source.clone(),
        data: input.data.clone(),
        natural_key_text: nk.text.clone(),
        natural_key_hash: nk.hash_hex(),
        created_at: now,
        updated_at: now,
        version: 1,
        deprecated_at: None,
        deprecation_reason: None,
        superseded_by: None,
    };

    let rowid = insert_entry(tx, &entry, &nk)?;
    insert_fts(tx, rowid, &entry)?;
    insert_history(tx, rowid, &entry, now)?;

    if let Some(old_ulid) = input.supersedes {
        deprecate_in_tx(tx, old_ulid, "superseded".into(), Some(id), now)?;
    }

    Ok(WriteResult::Written { id, version: 1 })
}

fn insert_entry(tx: &Transaction<'_>, e: &Entry, nk: &NaturalKey) -> KbResult<i64> {
    let tags_json = serde_json::to_string(&e.tags)?;
    let source_json = serde_json::to_string(&e.source)?;
    let type_data_json = serde_json::to_string(&e.data)?;

    tx.execute(
        "INSERT INTO entries (
            ulid, kind, title, body, tags, source, type_data,
            natural_key_text, natural_key_hash,
            created_at, updated_at, version
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            e.id.to_string(),
            e.data.kind().as_str(),
            e.title,
            e.body,
            tags_json,
            source_json,
            type_data_json,
            e.natural_key_text,
            nk.hash.as_slice(),
            e.created_at.to_rfc3339(),
            e.updated_at.to_rfc3339(),
            e.version,
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

fn insert_fts(tx: &Transaction<'_>, rowid: i64, e: &Entry) -> KbResult<()> {
    tx.execute(
        "INSERT INTO entries_fts (rowid, natural_key_text, title, body)
         VALUES (?1, ?2, ?3, ?4)",
        params![rowid, e.natural_key_text, e.title, e.body],
    )?;
    Ok(())
}

fn delete_fts(tx: &Transaction<'_>, rowid: i64, prev: &Entry) -> KbResult<()> {
    // FTS5 contentless tables require a "delete by rowid" hack: insert a
    // special command row.
    tx.execute(
        "INSERT INTO entries_fts (entries_fts, rowid, natural_key_text, title, body)
         VALUES ('delete', ?1, ?2, ?3, ?4)",
        params![rowid, prev.natural_key_text, prev.title, prev.body],
    )?;
    Ok(())
}

fn insert_history(
    tx: &Transaction<'_>,
    rowid: i64,
    e: &Entry,
    when: DateTime<Utc>,
) -> KbResult<()> {
    let snapshot = serde_json::to_string(&e)?;
    tx.execute(
        "INSERT INTO entry_history (entry_rowid, version, snapshot, recorded_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![rowid, e.version, snapshot, when.to_rfc3339()],
    )?;
    Ok(())
}

// =============================================================================
// get / update / deprecate
// =============================================================================

pub fn get_by_ulid(conn: &Conn, id: Ulid) -> KbResult<Option<Entry>> {
    let mut stmt = conn.prepare(
        "SELECT rowid, ulid, kind, title, body, tags, source, type_data,
                natural_key_text, natural_key_hash, created_at, updated_at, version,
                deprecated_at, deprecation_reason, superseded_by
           FROM entries WHERE ulid = ?1",
    )?;
    let row = stmt
        .query_row(params![id.to_string()], row_to_entry)
        .optional()?;
    Ok(row)
}

fn lookup_rowid(tx: &Transaction<'_>, id: Ulid) -> KbResult<i64> {
    tx.query_row(
        "SELECT rowid FROM entries WHERE ulid = ?1",
        params![id.to_string()],
        |row| row.get::<_, i64>(0),
    )
    .optional()?
    .ok_or(KbError::NotFound)
}

pub fn update(conn: &mut Conn, id: Ulid, partial: PartialEntry) -> KbResult<(Ulid, u32)> {
    let tx = conn.transaction()?;
    let rowid = lookup_rowid(&tx, id)?;
    let prev = tx
        .query_row(
            "SELECT rowid, ulid, kind, title, body, tags, source, type_data,
                    natural_key_text, natural_key_hash, created_at, updated_at, version,
                    deprecated_at, deprecation_reason, superseded_by
               FROM entries WHERE rowid = ?1",
            params![rowid],
            row_to_entry,
        )
        .map_err(KbError::Db)?;

    let new_data = partial.data.clone().unwrap_or_else(|| prev.data.clone());
    if new_data.kind() != prev.data.kind() {
        return Err(KbError::BadRequest(
            "kind cannot change on update; supersede instead".into(),
        ));
    }
    let new_title = partial.title.unwrap_or_else(|| prev.title.clone());
    let new_body = partial.body.unwrap_or_else(|| prev.body.clone());
    let new_tags = partial.tags.unwrap_or_else(|| prev.tags.clone());
    let new_nk = natural_key(&new_data);
    let now = Utc::now();
    let new_version = prev.version + 1;

    let tags_json = serde_json::to_string(&new_tags)?;
    let type_data_json = serde_json::to_string(&new_data)?;

    tx.execute(
        "UPDATE entries
            SET title = ?1, body = ?2, tags = ?3, type_data = ?4,
                natural_key_text = ?5, natural_key_hash = ?6,
                updated_at = ?7, version = ?8
          WHERE rowid = ?9",
        params![
            new_title,
            new_body,
            tags_json,
            type_data_json,
            new_nk.text,
            new_nk.hash.as_slice(),
            now.to_rfc3339(),
            new_version,
            rowid,
        ],
    )?;

    // Replace FTS row.
    delete_fts(&tx, rowid, &prev)?;
    let new_entry = Entry {
        id: prev.id,
        title: new_title,
        body: new_body,
        tags: new_tags,
        source: prev.source.clone(),
        data: new_data,
        natural_key_text: new_nk.text.clone(),
        natural_key_hash: new_nk.hash_hex(),
        created_at: prev.created_at,
        updated_at: now,
        version: new_version,
        deprecated_at: prev.deprecated_at,
        deprecation_reason: prev.deprecation_reason.clone(),
        superseded_by: prev.superseded_by,
    };
    insert_fts(&tx, rowid, &new_entry)?;
    insert_history(&tx, rowid, &new_entry, now)?;

    tx.commit()?;
    Ok((id, new_version))
}

pub fn deprecate(conn: &mut Conn, id: Ulid, reason: String) -> KbResult<()> {
    let tx = conn.transaction()?;
    let now = Utc::now();
    deprecate_in_tx(&tx, id, reason, None, now)?;
    tx.commit()?;
    Ok(())
}

fn deprecate_in_tx(
    tx: &Transaction<'_>,
    id: Ulid,
    reason: String,
    superseded_by: Option<Ulid>,
    when: DateTime<Utc>,
) -> KbResult<()> {
    let rowid = lookup_rowid(tx, id)?;
    let n = tx.execute(
        "UPDATE entries
            SET deprecated_at = ?1,
                deprecation_reason = ?2,
                superseded_by = ?3,
                updated_at = ?4
          WHERE rowid = ?5 AND deprecated_at IS NULL",
        params![
            when.to_rfc3339(),
            reason,
            superseded_by.map(|u| u.to_string()),
            when.to_rfc3339(),
            rowid,
        ],
    )?;
    if n == 0 {
        return Err(KbError::Conflict("already deprecated".into()));
    }
    Ok(())
}

// =============================================================================
// purge — physical delete, cascades over supersede chain, rewrites [[ulid]] refs
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PurgeSummary {
    pub dry_run: bool,
    pub purged: Vec<PurgedEntry>,
    pub rewritten: Vec<RewrittenEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PurgedEntry {
    #[schemars(with = "String")]
    pub id: Ulid,
    pub kind: EntryKind,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RewrittenEntry {
    #[schemars(with = "String")]
    pub id: Ulid,
    pub refs_count: u32,
}

pub fn purge(conn: &mut Conn, id: Ulid, dry_run: bool) -> KbResult<PurgeSummary> {
    let tx = conn.transaction()?;

    // 1. BFS over the supersede chain: collect `id` and every entry whose
    //    `superseded_by` (transitively) equals one of the entries we're about
    //    to delete. The first lookup must succeed; later ones can dangle and
    //    are skipped (a deprecated entry can reference a missing successor
    //    if the chain was edited out-of-band).
    let mut to_purge: Vec<(i64, Ulid, EntryKind, String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<Ulid> = VecDeque::new();
    queue.push_back(id);

    while let Some(cur) = queue.pop_front() {
        if !seen.insert(cur.to_string()) {
            continue;
        }
        let row: Option<(i64, String, String)> = tx
            .query_row(
                "SELECT rowid, kind, title FROM entries WHERE ulid = ?1",
                params![cur.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((rowid, kind_str, title)) = row else {
            if to_purge.is_empty() {
                return Err(KbError::NotFound);
            }
            continue;
        };
        let kind = parse_kind(&kind_str)
            .map_err(|e| KbError::BadRequest(format!("corrupt kind in row {rowid}: {e}")))?;
        to_purge.push((rowid, cur, kind, title));

        let mut stmt = tx.prepare("SELECT ulid FROM entries WHERE superseded_by = ?1")?;
        let kids = stmt.query_map(params![cur.to_string()], |r| r.get::<_, String>(0))?;
        for k in kids {
            let k_str = k?;
            let k_ulid = Ulid::from_string(&k_str).map_err(|e| {
                KbError::BadRequest(format!("corrupt superseded_by ulid {k_str}: {e}"))
            })?;
            queue.push_back(k_ulid);
        }
    }

    // 2. Find every surviving entry whose body contains `[[<ulid>]]` for any
    //    of the to-be-purged ids, and compute the rewritten body
    //    (`[[<ulid> → deleted]]`). Count occurrences for the summary.
    let purge_ids: Vec<String> = to_purge.iter().map(|(_, u, _, _)| u.to_string()).collect();
    let purge_set: HashSet<&str> = purge_ids.iter().map(String::as_str).collect();

    let mut rewritten: Vec<RewrittenEntry> = Vec::new();
    let mut body_updates: Vec<(i64, String)> = Vec::new();

    {
        let mut stmt = tx.prepare("SELECT rowid, ulid, body FROM entries")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (rowid, ulid_s, body) = row?;
            if purge_set.contains(ulid_s.as_str()) {
                continue;
            }
            let mut new_body = body.clone();
            let mut count = 0u32;
            for pid in &purge_ids {
                let needle = format!("[[{pid}]]");
                let n = new_body.matches(&needle).count() as u32;
                if n > 0 {
                    count += n;
                    let replacement = format!("[[{pid} → deleted]]");
                    new_body = new_body.replace(&needle, &replacement);
                }
            }
            if count > 0 {
                let ulid_typed = Ulid::from_string(&ulid_s).map_err(|e| {
                    KbError::BadRequest(format!("corrupt ulid {ulid_s}: {e}"))
                })?;
                rewritten.push(RewrittenEntry {
                    id: ulid_typed,
                    refs_count: count,
                });
                body_updates.push((rowid, new_body));
            }
        }
    }

    let summary = PurgeSummary {
        dry_run,
        purged: to_purge
            .iter()
            .map(|(_, u, k, t)| PurgedEntry {
                id: *u,
                kind: *k,
                title: t.clone(),
            })
            .collect(),
        rewritten: rewritten.clone(),
    };

    if dry_run {
        // Transaction rolls back on drop — explicit for clarity.
        drop(tx);
        return Ok(summary);
    }

    // 3. Apply body rewrites: bump version, refresh FTS, append history.
    let now = Utc::now();
    for (rowid, new_body) in &body_updates {
        let prev = tx.query_row(
            "SELECT rowid, ulid, kind, title, body, tags, source, type_data,
                    natural_key_text, natural_key_hash, created_at, updated_at, version,
                    deprecated_at, deprecation_reason, superseded_by
               FROM entries WHERE rowid = ?1",
            params![rowid],
            row_to_entry,
        )?;
        let new_version = prev.version + 1;
        tx.execute(
            "UPDATE entries SET body = ?1, updated_at = ?2, version = ?3 WHERE rowid = ?4",
            params![new_body, now.to_rfc3339(), new_version, rowid],
        )?;
        delete_fts(&tx, *rowid, &prev)?;
        let mut new_entry = prev.clone();
        new_entry.body = new_body.clone();
        new_entry.version = new_version;
        new_entry.updated_at = now;
        insert_fts(&tx, *rowid, &new_entry)?;
        insert_history(&tx, *rowid, &new_entry, now)?;
    }

    // 4. Delete each entry: drop FTS row, then DELETE FROM entries.
    //    entry_history rows go with the parent via ON DELETE CASCADE.
    for (rowid, _id, _kind, _title) in &to_purge {
        let prev = tx.query_row(
            "SELECT rowid, ulid, kind, title, body, tags, source, type_data,
                    natural_key_text, natural_key_hash, created_at, updated_at, version,
                    deprecated_at, deprecation_reason, superseded_by
               FROM entries WHERE rowid = ?1",
            params![rowid],
            row_to_entry,
        )?;
        delete_fts(&tx, *rowid, &prev)?;
        tx.execute("DELETE FROM entries WHERE rowid = ?1", params![rowid])?;
    }

    tx.commit()?;
    Ok(summary)
}

// =============================================================================
// search
// =============================================================================

pub fn search(conn: &Conn, q: &SearchQuery) -> KbResult<Vec<SearchHit>> {
    let limit = q.limit.clamp(1, 200);
    if let Some(query_text) = q.query.as_deref().filter(|s| !s.trim().is_empty()) {
        search_fts(conn, q, query_text, limit)
    } else {
        search_structured(conn, q, limit)
    }
}

fn search_structured(conn: &Conn, q: &SearchQuery, limit: u32) -> KbResult<Vec<SearchHit>> {
    let mut sql = String::from(
        "SELECT ulid, kind, title, substr(body, 1, ?) AS summary_line, tags,
                created_at, updated_at, deprecated_at
           FROM entries
          WHERE 1=1",
    );
    let mut idx = 1;
    let mut bindings: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(SUMMARY_LEN as i64)];
    idx += 1;

    if !q.kinds.is_empty() {
        let placeholders: Vec<String> = q
            .kinds
            .iter()
            .map(|_| {
                let p = format!("?{idx}");
                idx += 1;
                p
            })
            .collect();
        sql.push_str(&format!(" AND kind IN ({})", placeholders.join(",")));
        for k in &q.kinds {
            bindings.push(Box::new(k.as_str().to_string()));
        }
    }

    for prefix in &q.tag_prefixes {
        sql.push_str(&format!(" AND tags LIKE ?{idx}"));
        idx += 1;
        bindings.push(Box::new(format!("%\"{}%", escape_like(prefix))));
    }

    if !q.include_deprecated {
        sql.push_str(" AND deprecated_at IS NULL");
    }

    sql.push_str(&format!(" ORDER BY updated_at DESC LIMIT ?{idx}"));
    bindings.push(Box::new(i64::from(limit)));

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = bindings
        .iter()
        .map(|b| b.as_ref() as &dyn rusqlite::ToSql)
        .collect();
    let rows = stmt.query_map(refs.as_slice(), |r| row_to_hit(r, None))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn search_fts(conn: &Conn, q: &SearchQuery, text: &str, limit: u32) -> KbResult<Vec<SearchHit>> {
    // simple_query: wide recall; bm25 weights (5,5,1) per spec §6.
    let mut sql = String::from(
        "SELECT e.ulid, e.kind, e.title, substr(e.body, 1, ?1) AS summary_line, e.tags,
                e.created_at, e.updated_at, e.deprecated_at,
                bm25(entries_fts, 5.0, 5.0, 1.0) AS score
           FROM entries_fts
           JOIN entries e ON entries_fts.rowid = e.rowid
          WHERE entries_fts MATCH simple_query(?2)",
    );
    let mut idx = 3;
    let mut bindings: Vec<Box<dyn rusqlite::ToSql>> =
        vec![Box::new(SUMMARY_LEN as i64), Box::new(text.to_string())];

    if !q.kinds.is_empty() {
        let placeholders: Vec<String> = q
            .kinds
            .iter()
            .map(|_| {
                let p = format!("?{idx}");
                idx += 1;
                p
            })
            .collect();
        sql.push_str(&format!(" AND e.kind IN ({})", placeholders.join(",")));
        for k in &q.kinds {
            bindings.push(Box::new(k.as_str().to_string()));
        }
    }

    for prefix in &q.tag_prefixes {
        sql.push_str(&format!(" AND e.tags LIKE ?{idx}"));
        idx += 1;
        bindings.push(Box::new(format!("%\"{}%", escape_like(prefix))));
    }

    if !q.include_deprecated {
        sql.push_str(" AND e.deprecated_at IS NULL");
    }

    sql.push_str(&format!(" ORDER BY score LIMIT ?{idx}"));
    bindings.push(Box::new(i64::from(limit)));

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = bindings
        .iter()
        .map(|b| b.as_ref() as &dyn rusqlite::ToSql)
        .collect();
    let rows = stmt.query_map(refs.as_slice(), |r| row_to_hit(r, Some(8)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

// =============================================================================
// batch_search — fan out N independent SearchQuery, dedup by ULID across all
// =============================================================================

/// Fan out N independent search queries; collect into a single dedup'd hit list.
///
/// - Each sub-query goes through the same `search()` (struct or FTS mode);
/// - The same ULID returned by multiple sub-queries appears once, with
///   `matched_queries` listing the indices that hit it;
/// - A failing sub-query is isolated into `errors` and does not abort the batch;
/// - Result order: by `matched_queries[0]` asc (early query wins),
///   then bm25 score asc (FTS only — None drops to the back),
///   then `updated_at` desc.
///
/// Caps (max query count, etc.) are enforced at the API layer, not here.
pub fn batch_search(conn: &Conn, queries: &[SearchQuery]) -> KbResult<BatchSearchOutput> {
    let mut by_id: HashMap<Ulid, BatchSearchHit> = HashMap::new();
    let mut order: Vec<Ulid> = Vec::new(); // first-seen ULID order
    let mut errors: Vec<BatchSearchError> = Vec::new();

    for (idx, q) in queries.iter().enumerate() {
        let qi = idx as u32;
        match search(conn, q) {
            Ok(hits) => {
                for h in hits {
                    if let Some(agg) = by_id.get_mut(&h.id) {
                        agg.matched_queries.push(qi);
                        // bm25 scores from different sub-queries aren't
                        // rigorously comparable, but for the common case
                        // (both FTS hits) "lower is better" is the right
                        // sentiment. If any matching sub-query was
                        // structured (None score), keep None — there's no
                        // meaningful merged score.
                        agg.hit.score = match (agg.hit.score, h.score) {
                            (Some(a), Some(b)) => Some(a.min(b)),
                            _ => None,
                        };
                    } else {
                        order.push(h.id);
                        by_id.insert(
                            h.id,
                            BatchSearchHit {
                                hit: h,
                                matched_queries: vec![qi],
                            },
                        );
                    }
                }
            }
            Err(e) => errors.push(BatchSearchError {
                query_index: qi,
                error: e.to_string(),
            }),
        }
    }

    let mut hits: Vec<BatchSearchHit> = order
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect();
    hits.sort_by(|a, b| {
        a.matched_queries[0]
            .cmp(&b.matched_queries[0])
            .then_with(|| score_cmp(&a.hit.score, &b.hit.score))
            .then_with(|| b.hit.updated_at.cmp(&a.hit.updated_at))
    });

    Ok(BatchSearchOutput { hits, errors })
}

/// Order Option<f64> bm25 scores: lower is better; None drops to the back.
fn score_cmp(a: &Option<f64>, b: &Option<f64>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

// =============================================================================
// recent / stats
// =============================================================================

pub fn recent(conn: &Conn, n: u32, since: Option<DateTime<Utc>>) -> KbResult<Vec<SearchHit>> {
    let limit = n.clamp(1, 200);
    let mut sql = String::from(
        "SELECT ulid, kind, title, substr(body, 1, ?1) AS summary_line, tags,
                created_at, updated_at, deprecated_at
           FROM entries
          WHERE deprecated_at IS NULL",
    );
    let mut bindings: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(SUMMARY_LEN as i64)];
    let mut next = 2;
    if let Some(s) = since {
        sql.push_str(&format!(" AND updated_at >= ?{next}"));
        next += 1;
        bindings.push(Box::new(s.to_rfc3339()));
    }
    sql.push_str(&format!(" ORDER BY updated_at DESC LIMIT ?{next}"));
    bindings.push(Box::new(i64::from(limit)));

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = bindings
        .iter()
        .map(|b| b.as_ref() as &dyn rusqlite::ToSql)
        .collect();
    let rows = stmt.query_map(refs.as_slice(), |r| row_to_hit(r, None))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn stats(conn: &Conn) -> KbResult<Stats> {
    let mut stmt = conn.prepare(
        "SELECT kind,
                SUM(CASE WHEN deprecated_at IS NULL THEN 1 ELSE 0 END) AS active,
                SUM(CASE WHEN deprecated_at IS NOT NULL THEN 1 ELSE 0 END) AS deprecated
           FROM entries
          GROUP BY kind",
    )?;
    let by_kind_rows = stmt.query_map([], |r| {
        let kind_str: String = r.get(0)?;
        let kind = parse_kind(&kind_str).map_err(json_err)?;
        Ok(KindCount {
            kind,
            active: r.get(1)?,
            deprecated: r.get(2)?,
        })
    })?;
    let mut by_kind = Vec::new();
    for kc in by_kind_rows {
        by_kind.push(kc?);
    }
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))?;
    let active: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entries WHERE deprecated_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    Ok(Stats {
        total,
        active,
        deprecated: total - active,
        by_kind,
    })
}

// =============================================================================
// row mapping
// =============================================================================

fn row_to_entry(row: &Row<'_>) -> rusqlite::Result<Entry> {
    let ulid_s: String = row.get("ulid")?;
    let kind_s: String = row.get("kind")?;
    let tags_s: String = row.get("tags")?;
    let source_s: String = row.get("source")?;
    let type_data_s: String = row.get("type_data")?;
    let nk_hash: Vec<u8> = row.get("natural_key_hash")?;
    let created: String = row.get("created_at")?;
    let updated: String = row.get("updated_at")?;
    let deprecated: Option<String> = row.get("deprecated_at")?;
    let dep_reason: Option<String> = row.get("deprecation_reason")?;
    let sup_by: Option<String> = row.get("superseded_by")?;

    let id = Ulid::from_string(&ulid_s).map_err(json_err)?;
    let kind = parse_kind(&kind_s).map_err(json_err)?;
    let tags: Vec<String> = serde_json::from_str(&tags_s).map_err(json_err)?;
    let source: Source = serde_json::from_str(&source_s).map_err(json_err)?;
    let data: KindData = parse_type_data(kind, &type_data_s).map_err(json_err)?;
    let created_at = parse_rfc3339(&created).map_err(json_err)?;
    let updated_at = parse_rfc3339(&updated).map_err(json_err)?;
    let deprecated_at = deprecated
        .map(|s| parse_rfc3339(&s))
        .transpose()
        .map_err(json_err)?;
    let superseded_by = sup_by
        .map(|s| Ulid::from_string(&s).map_err(|e| format!("invalid superseded_by ulid: {e}")))
        .transpose()
        .map_err(json_err)?;

    Ok(Entry {
        id,
        title: row.get("title")?,
        body: row.get("body")?,
        tags,
        source,
        data,
        natural_key_text: row.get("natural_key_text")?,
        natural_key_hash: hex(&nk_hash),
        created_at,
        updated_at,
        version: row.get::<_, i64>("version")? as u32,
        deprecated_at,
        deprecation_reason: dep_reason,
        superseded_by,
    })
}

fn row_to_hit(row: &Row<'_>, score_idx: Option<usize>) -> rusqlite::Result<SearchHit> {
    let ulid_s: String = row.get(0)?;
    let kind_s: String = row.get(1)?;
    let title: String = row.get(2)?;
    let summary: String = row.get::<_, Option<String>>(3)?.unwrap_or_default();
    let tags_s: String = row.get(4)?;
    let created: String = row.get(5)?;
    let updated: String = row.get(6)?;
    let deprecated: Option<String> = row.get(7)?;
    let score: Option<f64> = match score_idx {
        Some(i) => Some(row.get(i)?),
        None => None,
    };
    Ok(SearchHit {
        id: Ulid::from_string(&ulid_s).map_err(json_err)?,
        kind: parse_kind(&kind_s).map_err(json_err)?,
        title,
        summary_line: summary,
        tags: serde_json::from_str(&tags_s).map_err(json_err)?,
        created_at: parse_rfc3339(&created).map_err(json_err)?,
        updated_at: parse_rfc3339(&updated).map_err(json_err)?,
        deprecated_at: deprecated
            .map(|s| parse_rfc3339(&s))
            .transpose()
            .map_err(json_err)?,
        score,
    })
}

fn json_err<E: std::fmt::Display>(e: E) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.to_string().into())
}

fn parse_kind(s: &str) -> Result<EntryKind, String> {
    match s {
        "Fact" => Ok(EntryKind::Fact),
        "ProblemSolution" => Ok(EntryKind::ProblemSolution),
        "Lesson" => Ok(EntryKind::Lesson),
        "Decision" => Ok(EntryKind::Decision),
        "Heuristic" => Ok(EntryKind::Heuristic),
        other => Err(format!("unknown kind: {other}")),
    }
}

fn parse_type_data(kind: EntryKind, json: &str) -> Result<KindData, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut obj = match v {
        serde_json::Value::Object(m) => m,
        _ => return Err("type_data not object".into()),
    };
    obj.insert(
        "kind".into(),
        serde_json::Value::String(kind.as_str().to_string()),
    );
    serde_json::from_value(serde_json::Value::Object(obj)).map_err(|e| e.to_string())
}

fn parse_rfc3339(s: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| e.to_string())
}

fn hex(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 0x0F) as usize] as char);
    }
    s
}

fn escape_like(s: &str) -> String {
    s.replace('%', "\\%").replace('_', "\\_")
}

// =============================================================================
// candidates pool
// =============================================================================

pub fn deposit_candidate(
    conn: &mut Conn,
    content: String,
    source: Source,
) -> KbResult<Ulid> {
    if content.is_empty() {
        return Err(KbError::BadRequest("candidate content is empty".into()));
    }
    if content.len() > CANDIDATE_MAX_CONTENT_BYTES {
        return Err(KbError::BadRequest(format!(
            "candidate content too large; max {} bytes, got {}",
            CANDIDATE_MAX_CONTENT_BYTES,
            content.len()
        )));
    }
    let id = Ulid::new();
    let now = Utc::now();
    let source_json = serde_json::to_string(&source)?;
    conn.execute(
        "INSERT INTO candidates (ulid, content, source, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id.to_string(), content, source_json, now.to_rfc3339()],
    )?;
    Ok(id)
}

pub fn list_candidates(conn: &Conn, f: &CandidateFilter) -> KbResult<Vec<Candidate>> {
    let limit = f.limit.unwrap_or(50).clamp(1, 200);
    let mut sql = String::from(
        "SELECT ulid, content, source, created_at
           FROM candidates
          WHERE 1=1",
    );
    let mut bindings: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(since) = f.since {
        sql.push_str(" AND created_at >= ?1");
        bindings.push(Box::new(since.to_rfc3339()));
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT ");
    sql.push_str(&limit.to_string());

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> =
        bindings.iter().map(|b| b.as_ref() as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_candidate)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn get_candidate(conn: &Conn, id: Ulid) -> KbResult<Option<Candidate>> {
    let mut stmt = conn.prepare(
        "SELECT ulid, content, source, created_at FROM candidates WHERE ulid = ?1",
    )?;
    let row = stmt
        .query_row(params![id.to_string()], row_to_candidate)
        .optional()?;
    Ok(row)
}

pub fn discard_candidate(conn: &mut Conn, id: Ulid) -> KbResult<()> {
    let n = conn.execute(
        "DELETE FROM candidates WHERE ulid = ?1",
        params![id.to_string()],
    )?;
    if n == 0 {
        Err(KbError::NotFound)
    } else {
        Ok(())
    }
}

/// Atomic: open tx, ensure candidate exists, run the normal write path
/// through `write_in_tx`. On `Written`, DELETE the candidate row in the same
/// transaction. On `DuplicatesFound`, transaction rolls back via drop —
/// candidate stays pending so the caller can re-promote with `dedup="force"`
/// + the returned `proceed_token`, or discard.
pub fn promote_candidate(
    conn: &mut Conn,
    id: Ulid,
    input: WriteInput,
) -> KbResult<WriteResult> {
    let tx = conn.transaction()?;

    // existence check up-front; cheap, and gives a clean NotFound before
    // any dedup work.
    let exists: i64 = tx.query_row(
        "SELECT COUNT(*) FROM candidates WHERE ulid = ?1",
        params![id.to_string()],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Err(KbError::NotFound);
    }

    let result = write_in_tx(&tx, input)?;
    if matches!(result, WriteResult::Written { .. }) {
        tx.execute(
            "DELETE FROM candidates WHERE ulid = ?1",
            params![id.to_string()],
        )?;
        tx.commit()?;
    }
    // DuplicatesFound: drop tx → rollback. Candidate row preserved.
    Ok(result)
}

fn row_to_candidate(row: &Row<'_>) -> rusqlite::Result<Candidate> {
    let ulid_s: String = row.get(0)?;
    let content: String = row.get(1)?;
    let source_json: String = row.get(2)?;
    let created_s: String = row.get(3)?;
    Ok(Candidate {
        id: Ulid::from_string(&ulid_s).map_err(json_err)?,
        content,
        source: serde_json::from_str(&source_json).map_err(json_err)?,
        created_at: parse_rfc3339(&created_s).map_err(json_err)?,
    })
}
