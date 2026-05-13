//! Entry storage: CRUD on `entries` + `entry_history`, plus search.
//!
//! All functions take a [`Conn`] (pooled rusqlite connection) and run
//! synchronously. Async handlers must wrap calls in `tokio::task::spawn_blocking`.
//!
//! [`Conn`]: crate::db::Conn

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
    let nk = natural_key(&input.data);
    let kind = input.data.kind();

    // Layer 1+2 dedup, unless the caller passed dedup='force' + matching token.
    if matches!(input.dedup, kb_core::DedupMode::Check) {
        let outcome = run_dedup(conn, kind, &nk)?;
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

    let tx = conn.transaction()?;
    let rowid = insert_entry(&tx, &entry, &nk)?;
    insert_fts(&tx, rowid, &entry)?;
    insert_history(&tx, rowid, &entry, now)?;

    if let Some(old_ulid) = input.supersedes {
        deprecate_in_tx(&tx, old_ulid, "superseded".into(), Some(id), now)?;
    }

    tx.commit()?;

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
