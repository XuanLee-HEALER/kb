//! SQLite connection pool. Each freshly-checked-out connection is configured
//! with WAL, foreign keys, and loaded with the `libsimple` FTS5 tokenizer.
//!
//! The jieba dict path is registered once per connection via `simple_jieba_init`
//! (no-op if it's already initialized for this process).

use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

use crate::libsimple;

pub type Pool = r2d2::Pool<SqliteConnectionManager>;
pub type Conn = PooledConnection<SqliteConnectionManager>;

/// Open a pool against the given path, applying our pragmas + libsimple load
/// on every fresh connection.
pub fn open_pool(db_path: &Path) -> Result<Pool> {
    let dict_path = libsimple::dict_dir();
    let lib_path = libsimple::lib_path();

    let manager = SqliteConnectionManager::file(db_path).with_init(move |c| {
        // SAFETY: loading a vetted vendored SQLite extension from a path we
        // resolve ourselves. No data is sourced from user input here.
        #[allow(unsafe_code)]
        unsafe {
            c.load_extension_enable()?;
            c.load_extension::<_, &str>(&lib_path, None)?;
            c.load_extension_disable()?;
        }

        c.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA synchronous=NORMAL;
             PRAGMA temp_store=MEMORY;
             PRAGMA busy_timeout=5000;",
        )?;

        // Point jieba at its dict directory. Per-connection, idempotent.
        // Returns the resolved path (TEXT) — discard it.
        let dict_str = dict_path.to_string_lossy().to_string();
        c.query_row("SELECT jieba_dict(?1)", [&dict_str], |row| {
            row.get::<_, String>(0)
        })?;
        Ok(())
    });

    let pool = r2d2::Pool::builder()
        .max_size(8)
        .build(manager)
        .context("building r2d2 pool")?;

    // Eager smoke test: open one connection now so misconfiguration fails fast.
    let _ = pool.get().context("acquiring initial sqlite connection")?;
    Ok(pool)
}

/// One-time check (logged once) that the FTS query functions are loaded.
pub fn verify_fts_functions(conn: &Connection) -> Result<()> {
    static CHECKED: OnceLock<()> = OnceLock::new();
    if CHECKED.get().is_some() {
        return Ok(());
    }
    let _: String = conn
        .query_row("SELECT simple_query('test')", [], |row| row.get(0))
        .context("simple_query() not callable — libsimple not loaded?")?;
    let _: String = conn
        .query_row("SELECT jieba_query('test')", [], |row| row.get(0))
        .context("jieba_query() not callable — libsimple jieba init failed?")?;
    let _ = CHECKED.set(());
    Ok(())
}
