//! Refinery-driven schema migrations. Each connection is migrated independently
//! before any other queries run.

use anyhow::{Context, Result};

use crate::db::Pool;

mod embedded {
    refinery::embed_migrations!("migrations");
}

pub fn run(pool: &Pool) -> Result<()> {
    let mut conn = pool.get().context("acquire conn for migrations")?;
    let report = embedded::migrations::runner()
        .run(&mut *conn)
        .context("refinery migration")?;
    for m in report.applied_migrations() {
        tracing::info!("applied migration: {} (v{})", m.name(), m.version());
    }
    Ok(())
}
