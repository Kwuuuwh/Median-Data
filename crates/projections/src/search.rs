use anyhow::Result;
use graph::Node;
use rusqlite::Transaction;

use crate::projection::{Context, Projection, Summary};

/// A full-text index over what the product ships, so the app can search names without
/// scanning the catalog.
pub struct Search;

const SETUP: &str = "\
CREATE VIRTUAL TABLE search USING fts5(
  unique_name UNINDEXED,
  name_en,
  name_ru,
  tokenize = 'unicode61 remove_diacritics 2'
);";

impl Projection for Search {
    fn name(&self) -> &'static str {
        "search"
    }

    fn db(&self, tx: &Transaction<'_>, ctx: &Context<'_>) -> Result<Option<Summary>> {
        tx.execute_batch(SETUP)?;
        let mut insert = tx.prepare(
            "INSERT INTO search (unique_name, name_en, name_ru) VALUES (?1, ?2, ?3)",
        )?;

        let mut rows = 0;
        for item in ctx.graph.items() {
            if !ctx.scope.allows(&item.unique_name) {
                continue;
            }
            insert.execute((
                &item.unique_name,
                &item.names.en.value,
                item.names.ru.as_ref().map(|r| r.value.as_str()),
            ))?;
            rows += 1;
        }

        // Imprints are their own tradable entity, not items, so index them too.
        for node in ctx.graph.nodes() {
            if let Node::Imprint(imprint) = node {
                insert.execute((
                    node.id(),
                    &imprint.names.en.value,
                    imprint.names.ru.as_ref().map(|r| r.value.as_str()),
                ))?;
                rows += 1;
            }
        }

        Ok(Some(Summary {
            name: self.name(),
            detail: format!("{rows} names indexed"),
        }))
    }
}
