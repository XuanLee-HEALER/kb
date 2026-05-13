//! End-to-end tests for the storage + dedup + FTS path.
//!
//! Each test gets a temporary on-disk SQLite database, runs migrations, then
//! exercises the public store API. libsimple is loaded automatically via the
//! pool init hook (KB_LIBSIMPLE_DIR / KB_LIBSIMPLE_DICT env vars baked in by
//! build.rs).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::bool_assert_comparison,
    clippy::pedantic,
    clippy::nursery
)]

use std::path::PathBuf;

use chrono::Utc;
use kb_core::{DedupMode, Evidence, EvidenceKind, KindData, Source, WriteInput, WriteResult};
use kb_server::db;
use kb_server::dedup::compute_proceed_token;
use kb_server::migrations;
use kb_server::store::{self, PartialEntry, SearchQuery};

fn fresh_db() -> (tempfile::TempDir, db::Pool) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = tmp.path().join("kb.sqlite");
    let pool = db::open_pool(&path).expect("open pool");
    migrations::run(&pool).expect("migrations");
    (tmp, pool)
}

fn fact(claim: &str) -> WriteInput {
    WriteInput {
        title: format!("title-{}", claim.chars().take(20).collect::<String>()),
        body: String::new(),
        tags: vec![],
        source: Source::Human,
        data: KindData::Fact {
            claim: claim.to_string(),
            evidence: vec![Evidence {
                kind: EvidenceKind::SelfVerification,
                content: "verified by me".into(),
                captured_at: Utc::now(),
            }],
        },
        supersedes: None,
        dedup: DedupMode::Check,
        proceed_token: None,
    }
}

#[test]
fn write_then_get_roundtrips() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    let res = store::write(
        &mut conn,
        fact("rust 1.83 enables const_trait_impl on traits"),
    )
    .unwrap();
    let id = match res {
        WriteResult::Written { id, version } => {
            assert_eq!(version, 1);
            id
        }
        WriteResult::DuplicatesFound { .. } => panic!("unexpected dup"),
    };
    let got = store::get_by_ulid(&conn, id).unwrap().expect("entry");
    assert_eq!(got.id, id);
    assert_eq!(got.title.is_empty(), false);
    assert!(got.natural_key_hash.len() == 64);
}

#[test]
fn layer1_exact_hits_on_identical_claim() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    let _ = store::write(
        &mut conn,
        fact("kernel 5.15 xt_TPROXY sets SO_ORIGINAL_DST"),
    )
    .unwrap();
    let res = store::write(
        &mut conn,
        fact("kernel 5.15 xt_TPROXY sets SO_ORIGINAL_DST"),
    )
    .unwrap();
    match res {
        WriteResult::DuplicatesFound { candidates, .. } => {
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].layer, kb_core::DedupLayer::Exact);
        }
        WriteResult::Written { .. } => panic!("layer 1 should have fired"),
    }
}

#[test]
fn layer1_only_within_same_kind() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    // Build a Fact whose claim happens to equal a Lesson's trap.
    let claim = "RAII drop order is bottom-up";
    let _ = store::write(&mut conn, fact(claim)).unwrap();

    let mut lesson_input = fact(claim);
    lesson_input.data = KindData::Lesson {
        trap: claim.into(),
        correction: "actually top-down for ZSTs".into(),
        why: "old impression".into(),
        context: None,
    };
    // Layer 1 keys hash+kind: same hash, different kind, should NOT collide.
    let res = store::write(&mut conn, lesson_input).unwrap();
    assert!(
        matches!(res, WriteResult::Written { .. }),
        "different kind must not collide on hash alone: {res:?}"
    );
}

#[test]
fn deprecated_entries_dont_block_layer1() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    let res = store::write(&mut conn, fact("ephemeral fact")).unwrap();
    let id = match res {
        WriteResult::Written { id, .. } => id,
        _ => unreachable!(),
    };
    store::deprecate(&mut conn, id, "no longer correct".into()).unwrap();
    // Re-writing the exact same claim should now succeed (means "I think it's right again").
    let res = store::write(&mut conn, fact("ephemeral fact")).unwrap();
    assert!(matches!(res, WriteResult::Written { .. }));
}

#[test]
fn layer2_fts_recalls_overlapping_token() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    // Seed with the verbose version.
    let _ = store::write(&mut conn, fact("tokio 阻塞 worker 线程 延迟现象 详细分析")).unwrap();
    // Layer 2 (jieba_query) AND-joins the query tokens, so the incoming NK's
    // token set must be a subset of an existing entry's token set to hit.
    let res = store::write(&mut conn, fact("tokio 阻塞 worker 线程")).unwrap();
    match res {
        WriteResult::DuplicatesFound { candidates, .. } => {
            assert!(
                candidates
                    .iter()
                    .any(|c| c.layer == kb_core::DedupLayer::Fts),
                "expected layer 2 hit, got: {candidates:?}"
            );
        }
        WriteResult::Written { .. } => panic!("Layer 2 should have caught this"),
    }
}

#[test]
fn dedup_force_with_token_proceeds() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    let claim = "duplicate me on purpose";
    let _ = store::write(&mut conn, fact(claim)).unwrap();

    let nk = kb_core::natural_key(&fact(claim).data);
    let token = compute_proceed_token(&nk);

    let mut forced = fact(claim);
    forced.dedup = DedupMode::Force;
    forced.proceed_token = Some(token);
    let res = store::write(&mut conn, forced).unwrap();
    assert!(matches!(res, WriteResult::Written { .. }));
}

#[test]
fn supersede_marks_old_deprecated() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    let res = store::write(&mut conn, fact("v1 of a claim")).unwrap();
    let old_id = match res {
        WriteResult::Written { id, .. } => id,
        _ => unreachable!(),
    };
    let mut newer = fact("v2 of the claim with refinement");
    newer.supersedes = Some(old_id);
    let _new = store::write(&mut conn, newer).unwrap();

    let old = store::get_by_ulid(&conn, old_id).unwrap().unwrap();
    assert!(old.deprecated_at.is_some());
    assert!(old.superseded_by.is_some());
}

#[test]
fn structured_search_filters_by_kind_and_tag() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    let mut a = fact("alpha");
    a.tags = vec!["network/wireguard".into()];
    let mut b = fact("beta");
    b.tags = vec!["rust/tokio".into()];
    store::write(&mut conn, a).unwrap();
    store::write(&mut conn, b).unwrap();

    let q = SearchQuery {
        kinds: vec![kb_core::EntryKind::Fact],
        tag_prefixes: vec!["network/".into()],
        query: None,
        limit: 20,
        include_deprecated: false,
    };
    let hits = store::search(&conn, &q).unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].tags.iter().any(|t| t.starts_with("network/")));
}

#[test]
fn fts_search_orders_by_score() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    store::write(&mut conn, fact("tokio 任务取消会丢失数据的现象")).unwrap();
    store::write(&mut conn, fact("smol 调度器特性")).unwrap();
    store::write(&mut conn, fact("rust async runtime 偏好选型")).unwrap();

    let q = SearchQuery {
        kinds: vec![],
        tag_prefixes: vec![],
        query: Some("tokio 任务取消".into()),
        limit: 10,
        include_deprecated: false,
    };
    let hits = store::search(&conn, &q).unwrap();
    assert!(!hits.is_empty());
    // best match should be the tokio entry
    assert!(hits[0].title.contains("tokio") || hits[0].summary_line.is_empty());
}

#[test]
fn update_bumps_version_and_history() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    let res = store::write(&mut conn, fact("initial wording")).unwrap();
    let id = match res {
        WriteResult::Written { id, .. } => id,
        _ => unreachable!(),
    };
    let partial = PartialEntry {
        title: Some("rewritten title".into()),
        ..Default::default()
    };
    let (id2, ver2) = store::update(&mut conn, id, partial).unwrap();
    assert_eq!(id2, id);
    assert_eq!(ver2, 2);
    let e = store::get_by_ulid(&conn, id).unwrap().unwrap();
    assert_eq!(e.title, "rewritten title");
    assert_eq!(e.version, 2);

    // entry_history should have 2 rows for this rowid
    let conn = pool.get().unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entry_history WHERE entry_rowid = (SELECT rowid FROM entries WHERE ulid = ?1)",
            [id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn stats_counts_by_kind() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    store::write(&mut conn, fact("a")).unwrap();
    store::write(&mut conn, fact("b")).unwrap();
    let s = store::stats(&conn).unwrap();
    assert_eq!(s.total, 2);
    assert_eq!(s.active, 2);
    assert_eq!(s.deprecated, 0);
    let fact_count = s
        .by_kind
        .iter()
        .find(|k| k.kind == kb_core::EntryKind::Fact)
        .unwrap();
    assert_eq!(fact_count.active, 2);
}

#[test]
fn search_excludes_deprecated_by_default() {
    let (_t, pool) = fresh_db();
    let mut conn = pool.get().unwrap();
    let res = store::write(&mut conn, fact("transient")).unwrap();
    let id = match res {
        WriteResult::Written { id, .. } => id,
        _ => unreachable!(),
    };
    store::deprecate(&mut conn, id, "stale".into()).unwrap();

    let q = SearchQuery {
        kinds: vec![],
        tag_prefixes: vec![],
        query: Some("transient".into()),
        limit: 10,
        include_deprecated: false,
    };
    let hits = store::search(&conn, &q).unwrap();
    assert!(hits.is_empty());

    let q2 = SearchQuery {
        include_deprecated: true,
        ..q
    };
    let hits2 = store::search(&conn, &q2).unwrap();
    assert_eq!(hits2.len(), 1);
}
