use anyhow::Result;
use graph::{Graph, Node};
use rusqlite::Transaction;

use crate::projection::{Context, Projection, Summary};

/// What every craftable item costs in things no recipe makes. The app can walk
/// `recipe_requires` itself for one item, but sorting or filtering thousands of items by what
/// they cost needs the walk done once, here.
pub struct Costs;

pub const SETUP: &str = "\
CREATE TABLE item_costs (
  item TEXT NOT NULL,
  leaf TEXT NOT NULL,
  qty  INTEGER NOT NULL,
  PRIMARY KEY (item, leaf)
) WITHOUT ROWID;
CREATE INDEX idx_item_costs_leaf ON item_costs(leaf);";

impl Projection for Costs {
    fn name(&self) -> &'static str {
        "costs"
    }

    fn db(&self, tx: &Transaction<'_>, ctx: &Context<'_>) -> Result<Option<Summary>> {
        tx.execute_batch(SETUP)?;
        let mut insert =
            tx.prepare("INSERT INTO item_costs (item, leaf, qty) VALUES (?1, ?2, ?3)")?;

        let built = craftable(ctx.graph);
        let costs = graph::rollup_all(ctx.graph, built.iter().copied());
        let mut rows = 0;
        for (item, leaves) in &costs {
            for (leaf, qty) in leaves {
                insert.execute((item, leaf, qty))?;
                rows += 1;
            }
        }

        Ok(Some(Summary {
            name: self.name(),
            detail: format!("{} items over {rows} base materials", costs.len()),
        }))
    }
}

/// Everything a recipe produces, in id order.
fn craftable(graph: &Graph) -> Vec<&str> {
    let mut out: Vec<&str> = graph
        .nodes()
        .filter_map(|node| match node {
            Node::Recipe(_) => graph::produced(graph, &node.id()),
            _ => None,
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}
