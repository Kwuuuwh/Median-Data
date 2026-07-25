use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use graph::Graph;
use serde::Deserialize;

/// What the product ships. Everything is in scope unless a rule says otherwise, so a new
/// DE mechanic shows up rather than silently vanishing.
#[derive(Debug, Default, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub exclude: Vec<Rule>,
}

/// One exclusion, with the reason it exists.
#[derive(Debug, Deserialize)]
pub struct Rule {
    pub reason: String,
    #[serde(default)]
    pub path_contains: Vec<String>,
    #[serde(default)]
    pub name_equals: Vec<String>,
    #[serde(default)]
    pub category_equals: Vec<String>,
    /// Alternate copies under a tier folder that answer to a real item's name.
    #[serde(default)]
    pub tier_duplicates: bool,
    /// Keep anything the market trades, whatever else this rule says.
    #[serde(default)]
    pub except_tradable: bool,
}

/// Which items the artifacts carry, and why the rest were left out.
#[derive(Debug, Default)]
pub struct Scope {
    excluded: BTreeMap<String, String>,
    in_scope: usize,
}

impl Scope {
    pub fn allows(&self, path: &str) -> bool {
        !self.excluded.contains_key(path)
    }

    /// Why an item was left out, if it was.
    pub fn reason(&self, path: &str) -> Option<&str> {
        self.excluded.get(path).map(String::as_str)
    }

    pub fn in_scope(&self) -> usize {
        self.in_scope
    }

    pub fn excluded(&self) -> usize {
        self.excluded.len()
    }

    /// How many items each reason accounts for.
    pub fn tally(&self) -> BTreeMap<&str, usize> {
        let mut counts = BTreeMap::new();
        for reason in self.excluded.values() {
            *counts.entry(reason.as_str()).or_default() += 1;
        }
        counts
    }
}

/// Read the policy from a TOML file. A missing file means everything ships.
pub fn load(path: &Path) -> Result<Policy> {
    if !path.exists() {
        return Ok(Policy::default());
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// Apply the policy to every item in the graph.
pub fn apply(graph: &Graph, policy: &Policy) -> Scope {
    let untiered = untiered_names(graph);
    let mut excluded = BTreeMap::new();
    let mut in_scope = 0;

    for item in graph.items() {
        let tradable = item.tradable.as_ref().is_some_and(|t| t.value);
        let hit = policy.exclude.iter().find(|rule| {
            if rule.except_tradable && tradable {
                return false;
            }
            let duplicate = rule.tier_duplicates
                && graph::tiered(&item.unique_name)
                && untiered.contains(&item.names.en.value);
            duplicate
                || rule
                    .path_contains
                    .iter()
                    .any(|p| item.unique_name.contains(p))
                || rule.name_equals.iter().any(|n| *n == item.names.en.value)
                || rule
                    .category_equals
                    .iter()
                    .any(|c| *c == item.category.value)
        });
        match hit {
            Some(rule) => {
                excluded.insert(item.unique_name.clone(), rule.reason.clone());
            }
            None => in_scope += 1,
        }
    }

    Scope { excluded, in_scope }
}

/// Names answered to by an item that does not sit under a tier folder.
fn untiered_names(graph: &Graph) -> BTreeSet<String> {
    graph
        .items()
        .filter(|i| !graph::tiered(&i.unique_name))
        .map(|i| i.names.en.value.clone())
        .collect()
}

/// Paths the artifacts carry, in id order.
pub fn kept(graph: &Graph, scope: &Scope) -> BTreeSet<String> {
    graph
        .items()
        .filter(|i| scope.allows(&i.unique_name))
        .map(|i| i.unique_name.clone())
        .collect()
}
