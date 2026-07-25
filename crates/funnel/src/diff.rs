use std::collections::{BTreeMap, BTreeSet};

use graph::{Graph, Node};
use serde::{Deserialize, Serialize};

use crate::finding::Finding;

/// What one build held, kept so the next build can say what moved.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    /// The version this build was stamped with, so the next one can carry on from it.
    #[serde(default)]
    pub version: Option<String>,
    pub items: Vec<String>,
    pub sets: Vec<String>,
    pub findings: BTreeMap<String, usize>,
}

/// What changed between two builds.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Diff {
    pub items_added: Vec<String>,
    pub items_removed: Vec<String>,
    pub sets_added: Vec<String>,
    pub sets_removed: Vec<String>,
    /// Findings per rule, current minus previous.
    pub findings_delta: BTreeMap<String, i64>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.items_added.is_empty()
            && self.items_removed.is_empty()
            && self.sets_added.is_empty()
            && self.sets_removed.is_empty()
            && self.findings_delta.is_empty()
    }
}

/// Record what this build holds.
pub fn snapshot(graph: &Graph, findings: &[Finding]) -> State {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for f in findings {
        *counts.entry(f.rule.clone()).or_default() += 1;
    }
    State {
        version: None,
        items: graph.items().map(|i| i.unique_name.clone()).collect(),
        sets: graph
            .nodes()
            .filter_map(|n| match n {
                Node::Set(s) => Some(s.slug.clone()),
                _ => None,
            })
            .collect(),
        findings: counts,
    }
}

/// Compare this build with the one before it.
pub fn compare(previous: &State, current: &State) -> Diff {
    let (items_added, items_removed) = split(&previous.items, &current.items);
    let (sets_added, sets_removed) = split(&previous.sets, &current.sets);

    let mut findings_delta = BTreeMap::new();
    let rules: BTreeSet<&String> = previous
        .findings
        .keys()
        .chain(current.findings.keys())
        .collect();
    for rule in rules {
        let before = *previous.findings.get(rule).unwrap_or(&0) as i64;
        let after = *current.findings.get(rule).unwrap_or(&0) as i64;
        if before != after {
            findings_delta.insert(rule.clone(), after - before);
        }
    }

    Diff {
        items_added,
        items_removed,
        sets_added,
        sets_removed,
        findings_delta,
    }
}

fn split(previous: &[String], current: &[String]) -> (Vec<String>, Vec<String>) {
    let before: BTreeSet<&str> = previous.iter().map(String::as_str).collect();
    let after: BTreeSet<&str> = current.iter().map(String::as_str).collect();
    (
        after.difference(&before).map(|s| s.to_string()).collect(),
        before.difference(&after).map(|s| s.to_string()).collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(items: &[&str], findings: &[(&str, usize)]) -> State {
        State {
            version: None,
            items: items.iter().map(|s| s.to_string()).collect(),
            sets: Vec::new(),
            findings: findings
                .iter()
                .map(|(r, c)| ((*r).to_string(), *c))
                .collect(),
        }
    }

    #[test]
    fn reports_what_moved() {
        let before = state(&["/a", "/b"], &[("rule-x", 3)]);
        let after = state(&["/b", "/c"], &[("rule-x", 1)]);
        let diff = compare(&before, &after);
        assert_eq!(diff.items_added, vec!["/c"]);
        assert_eq!(diff.items_removed, vec!["/a"]);
        assert_eq!(diff.findings_delta.get("rule-x"), Some(&-2));
    }

    #[test]
    fn an_unchanged_build_diffs_to_nothing() {
        let s = state(&["/a"], &[("rule-x", 1)]);
        assert!(compare(&s, &s).is_empty());
    }
}
