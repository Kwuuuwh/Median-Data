use std::path::Path;

use anyhow::{Context, Result};
use graph::{Extra, Graph, Node, Splice};
use serde::Deserialize;

/// The splicing table, written by hand.
#[derive(Debug, Default, Deserialize)]
pub struct Splicing {
    /// Rows of the wiki's splicing table when the file was last checked against it.
    pub checked_rows: usize,
    #[serde(default)]
    pub splice: Vec<Splice>,
}

/// Read the splicing table. A missing file means none is written yet.
pub fn load(path: &Path) -> Result<Splicing> {
    if !path.exists() {
        return Ok(Splicing::default());
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// Give every riven pool the splices whose three stats it rolls. Returns what does not fit:
/// a splice no pool takes, and a wiki table that no longer has the rows the file was checked
/// against.
pub fn link(graph: &mut Graph, splicing: &Splicing, wiki_rows: usize) -> Vec<String> {
    let mut unfit = Vec::new();
    if wiki_rows != splicing.checked_rows {
        unfit.push(format!(
            "the wiki's splicing table has {wiki_rows} rows, {} when this table was checked",
            splicing.checked_rows
        ));
    }
    let mut taken = vec![false; splicing.splice.len()];
    for node in graph.nodes_mut() {
        let Node::Item(item) = node else { continue };
        let Extra::RivenPool(pool) = &mut item.extra else {
            continue;
        };
        for (at, splice) in splicing.splice.iter().enumerate() {
            let rolls = |tag: &str| pool.stats.iter().any(|s| s.tag == tag);
            if rolls(&splice.first) && rolls(&splice.second) && rolls(&splice.result) {
                pool.splices.push(splice.clone());
                taken[at] = true;
            }
        }
    }
    for (splice, taken) in splicing.splice.iter().zip(taken) {
        if !taken {
            unfit.push(format!(
                "{} + {} -> {}: no riven pool rolls all three",
                splice.first, splice.second, splice.result
            ));
        }
    }
    unfit
}
