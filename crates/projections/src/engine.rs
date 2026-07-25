use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result};
use rusqlite::Connection;

use crate::catalog::Catalog;
use crate::changes::Changes;
use crate::costs::Costs;
use crate::icons::Icons;
use crate::projection::{Context, Projection, Summary};
use crate::search::Search;

/// Every view the build publishes. Adding one means adding it here and nowhere else.
fn all() -> Vec<Box<dyn Projection>> {
    vec![
        Box::new(Catalog),
        Box::new(Costs),
        Box::new(Search),
        Box::new(Icons),
        Box::new(Changes),
    ]
}

const PRAGMA: &str = "\
PRAGMA page_size=4096;
PRAGMA journal_mode=OFF;
PRAGMA user_version=3;";

/// Render every projection from one graph. The database is built fresh so a build never
/// inherits anything from the last one.
pub fn run(db_path: &Path, ctx: &Context<'_>) -> Result<Vec<Summary>> {
    if db_path.exists() {
        fs::remove_file(db_path)?;
    }
    let mut conn = Connection::open(db_path)?;
    conn.execute_batch(PRAGMA)?;

    let mut summaries = Vec::new();
    let projections = all();

    let tx = conn.transaction()?;
    for p in &projections {
        if let Some(summary) = p
            .db(&tx, ctx)
            .with_context(|| format!("projection {}", p.name()))?
        {
            summaries.push(summary);
        }
    }
    tx.commit()?;
    conn.execute_batch("VACUUM")?;

    for p in &projections {
        if let Some(summary) = p
            .file(ctx)
            .with_context(|| format!("projection {}", p.name()))?
        {
            summaries.push(summary);
        }
    }
    Ok(summaries)
}
