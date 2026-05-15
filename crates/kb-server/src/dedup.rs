//! Three-layer dedup pipeline. Only Layer 1+2 are implemented in v2.2.
//!
//! - Layer 1: exact hash match on `natural_key_hash` + kind, ignoring deprecated.
//! - Layer 2: bm25 FTS5 search via `jieba_query()` on the natural-key text only,
//!   column weights (10, 3, 1), top-K=10.
//!
//! On any candidate the caller receives a `proceed_token` derived
//! deterministically from the natural key hash. Re-sending `dedup='force'` with
//! that token bypasses the check for this exact NK.

use kb_core::{DedupLayer, DuplicateCandidate, EntryKind, NaturalKey};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use ulid::Ulid;

use crate::error::KbResult;

#[derive(Debug)]
pub enum DedupOutcome {
    Clean,
    Found {
        candidates: Vec<DuplicateCandidate>,
        proceed_token: String,
    },
}

const LAYER2_LIMIT: u32 = 10;

pub fn run_dedup(conn: &Connection, kind: EntryKind, nk: &NaturalKey) -> KbResult<DedupOutcome> {
    if let Some(c) = layer1_exact(conn, kind, nk)? {
        return Ok(DedupOutcome::Found {
            candidates: vec![c],
            proceed_token: compute_proceed_token(nk),
        });
    }

    let l2 = layer2_fts(conn, kind, nk)?;
    if l2.is_empty() {
        Ok(DedupOutcome::Clean)
    } else {
        Ok(DedupOutcome::Found {
            candidates: l2,
            proceed_token: compute_proceed_token(nk),
        })
    }
}

fn layer1_exact(
    conn: &Connection,
    kind: EntryKind,
    nk: &NaturalKey,
) -> KbResult<Option<DuplicateCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT ulid, kind, title, natural_key_text
           FROM entries
          WHERE natural_key_hash = ?1
            AND kind = ?2
            AND deprecated_at IS NULL
          LIMIT 1",
    )?;
    let mut rows = stmt.query(params![nk.hash.as_slice(), kind.as_str()])?;
    if let Some(row) = rows.next()? {
        let ulid_s: String = row.get(0)?;
        let kind_s: String = row.get(1)?;
        let title: String = row.get(2)?;
        let nk_text: String = row.get(3)?;
        Ok(Some(DuplicateCandidate {
            id: Ulid::from_string(&ulid_s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    e.to_string().into(),
                )
            })?,
            kind: parse_kind(&kind_s)?,
            title,
            natural_key_text: nk_text,
            layer: DedupLayer::Exact,
            score: 1.0,
        }))
    } else {
        Ok(None)
    }
}

fn layer2_fts(
    conn: &Connection,
    kind: EntryKind,
    nk: &NaturalKey,
) -> KbResult<Vec<DuplicateCandidate>> {
    let mut stmt = conn.prepare(
        "SELECT e.ulid, e.kind, e.title, e.natural_key_text,
                bm25(entries_fts, 10.0, 3.0, 1.0) AS score
           FROM entries_fts
           JOIN entries e ON entries_fts.rowid = e.rowid
          WHERE entries_fts MATCH jieba_query(?1)
            AND e.kind = ?2
            AND e.deprecated_at IS NULL
          ORDER BY score
          LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![&nk.text, kind.as_str(), LAYER2_LIMIT], |row| {
        let ulid_s: String = row.get(0)?;
        let kind_s: String = row.get(1)?;
        let title: String = row.get(2)?;
        let nk_text: String = row.get(3)?;
        let score: f64 = row.get(4)?;
        Ok((ulid_s, kind_s, title, nk_text, score))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (ulid_s, kind_s, title, nk_text, score) = r?;
        out.push(DuplicateCandidate {
            id: Ulid::from_string(&ulid_s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    e.to_string().into(),
                )
            })?,
            kind: parse_kind(&kind_s)?,
            title,
            natural_key_text: nk_text,
            layer: DedupLayer::Fts,
            score,
        });
    }
    Ok(out)
}

fn parse_kind(s: &str) -> Result<EntryKind, rusqlite::Error> {
    match s {
        "Fact" => Ok(EntryKind::Fact),
        "ProblemSolution" => Ok(EntryKind::ProblemSolution),
        "Lesson" => Ok(EntryKind::Lesson),
        "Decision" => Ok(EntryKind::Decision),
        "Heuristic" => Ok(EntryKind::Heuristic),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            format!("unknown kind: {other}").into(),
        )),
    }
}

/// Deterministic per-NK proceed token.
///
/// The single-user model doesn't need replay protection — the token's job is
/// "client saw the candidates we just returned for this NK." `sha256` over the
/// normalized text + a fixed salt is enough.
#[must_use]
pub fn compute_proceed_token(nk: &NaturalKey) -> String {
    let mut h = Sha256::new();
    h.update(b"kb:proceed:v1:");
    h.update(nk.text.as_bytes());
    let out = h.finalize();
    hex(&out[..])
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
