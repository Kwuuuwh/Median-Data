use anyhow::Result;
use graph::Node;
use rusqlite::Transaction;

use crate::projection::{Context, Projection, Summary};

/// A full-text index over what the product ships, so the app can search names without
/// scanning the catalog.
pub struct Search;

/// Name of the rule the indexed names are folded by.
pub const FOLD: &str = "ru-yo-1";

const SETUP: &str = "\
CREATE VIRTUAL TABLE search USING fts5(
  unique_name UNINDEXED,
  name_en UNINDEXED,
  name_ru UNINDEXED,
  fold_en,
  fold_ru,
  tokenize = 'unicode61 remove_diacritics 2'
);";

/// A name as the index holds it and as a query is matched against it: lower case, with
/// `ё` written as `е`.
pub fn fold(name: &str) -> String {
    name.to_lowercase().replace('ё', "е")
}

impl Projection for Search {
    fn name(&self) -> &'static str {
        "search"
    }

    fn db(&self, tx: &Transaction<'_>, ctx: &Context<'_>) -> Result<Option<Summary>> {
        tx.execute_batch(SETUP)?;
        let mut insert =
            tx.prepare("INSERT INTO search (unique_name, name_en, name_ru, fold_en, fold_ru) VALUES (?1, ?2, ?3, ?4, ?5)")?;

        let mut rows = 0;
        for item in ctx.graph.items() {
            if !ctx.scope.allows(&item.unique_name) {
                continue;
            }
            let ru = item.names.ru.as_ref().map(|r| r.value.as_str());
            insert.execute((
                &item.unique_name,
                &item.names.en.value,
                ru,
                fold(&item.names.en.value),
                ru.map(fold),
            ))?;
            rows += 1;
        }

        // Imprints are their own tradable entity, not items, so index them too.
        for node in ctx.graph.nodes() {
            if let Node::Imprint(imprint) = node {
                let ru = imprint.names.ru.as_ref().map(|r| r.value.as_str());
                insert.execute((
                    node.id(),
                    &imprint.names.en.value,
                    ru,
                    fold(&imprint.names.en.value),
                    ru.map(fold),
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

#[cfg(test)]
mod tests {
    use super::fold;

    #[test]
    fn folds_case_and_yo() {
        assert_eq!(fold("Дирижёр"), "дирижер");
        assert_eq!(fold("ВОЛЬТ ПРАЙМ"), "вольт прайм");
        assert_eq!(fold("Volt Prime"), "volt prime");
    }
}
