use std::collections::{BTreeMap, BTreeSet};

use graph::{Graph, Node, Rel};
use serde::{Deserialize, Serialize};

use crate::finding::Finding;

/// What one build held, kept so the next build can say what moved.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    /// The version this build was stamped with, so the next one can carry on from it.
    #[serde(default)]
    pub version: Option<String>,
    /// Fingerprint of the rules and code the build was made by.
    #[serde(default)]
    pub recipe: Option<String>,
    pub items: Vec<String>,
    pub sets: Vec<String>,
    pub findings: BTreeMap<String, usize>,
    /// What drops each item and how often, so the next build can say that a source moved:
    /// a thing that fell from one boss now falling off ordinary enemies changes what it is
    /// worth, and nothing else in the build would notice.
    #[serde(default)]
    pub drops: BTreeMap<String, String>,
    /// When each name no item answers to was first seen, per source. A patch day fills the
    /// sources with things DE has not exported yet, and those resolve themselves within days;
    /// a name that has been waiting for weeks is the one that needs a person.
    #[serde(default)]
    pub waiting: BTreeMap<String, BTreeMap<String, i64>>,
}

/// Carry forward when each unresolved name was first seen: the ones already known keep their
/// date, the new ones start now, and the ones that resolved are dropped.
pub fn waiting(
    was: &BTreeMap<String, BTreeMap<String, i64>>,
    now: impl Iterator<Item = (String, String)>,
    now_ms: i64,
) -> BTreeMap<String, BTreeMap<String, i64>> {
    let mut out: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    for (source, key) in now {
        let since = was
            .get(&source)
            .and_then(|keys| keys.get(&key))
            .copied()
            .unwrap_or(now_ms);
        out.entry(source).or_default().insert(key, since);
    }
    out
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
    /// Items whose sources changed, with what dropped them before and what does now.
    pub drops_changed: Vec<Moved>,
}

/// How an item's sources moved: only what differs, since most of a long list stays put.
#[derive(Debug, Serialize, Deserialize)]
pub struct Moved {
    pub item: String,
    /// Sources that now drop it and did not before.
    pub added: Vec<String>,
    /// Sources that dropped it before and no longer do.
    pub gone: Vec<String>,
    /// Sources that still drop it, at a different chance: `source 3.030% -> 2.650%`.
    pub rechanced: Vec<String>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.items_added.is_empty()
            && self.items_removed.is_empty()
            && self.sets_added.is_empty()
            && self.sets_removed.is_empty()
            && self.findings_delta.is_empty()
            && self.drops_changed.is_empty()
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
        recipe: None,
        // Filled by the build, which is where the unresolved names are known.
        waiting: BTreeMap::new(),
        items: graph.items().map(|i| i.unique_name.clone()).collect(),
        sets: graph
            .nodes()
            .filter_map(|n| match n {
                Node::Set(s) => Some(s.slug.clone()),
                _ => None,
            })
            .collect(),
        findings: counts,
        drops: drops(graph),
    }
}

/// What drops each item, read as one line per item: every source with the chance of the whole
/// roll, in name order so the same graph always reads the same way.
fn drops(graph: &Graph) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in graph.edges() {
        let Rel::Drops(drop) = &edge.rel else {
            continue;
        };
        let Some(source) = graph.get(&edge.from).map(source_name) else {
            continue;
        };
        out.entry(edge.to.clone())
            .or_default()
            .insert(format!("{source} {:.3}%", drop.total() * 100.0));
    }
    out.into_iter()
        .map(|(item, sources)| (item, sources.into_iter().collect::<Vec<_>>().join("; ")))
        .collect()
}

/// What a node is called where a drop comes from it.
fn source_name(node: &Node) -> String {
    match node {
        Node::Enemy(enemy) => enemy.name.clone(),
        Node::Place(place) => place.name.clone(),
        other => other.id(),
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

    let drops_changed = previous
        .drops
        .keys()
        .chain(current.drops.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|item| {
            let before = previous.drops.get(item).cloned().unwrap_or_default();
            let after = current.drops.get(item).cloned().unwrap_or_default();
            // An item that arrived or left is already reported as such.
            let known = previous.drops.contains_key(item) && current.drops.contains_key(item);
            (known && before != after).then(|| moved(item, &before, &after))
        })
        .collect();

    Diff {
        items_added,
        items_removed,
        sets_added,
        sets_removed,
        findings_delta,
        drops_changed,
    }
}

/// What differs between one item's sources before and after.
fn moved(item: &str, before: &str, after: &str) -> Moved {
    let (was, now) = (chances(before), chances(after));
    Moved {
        item: item.to_string(),
        added: now
            .iter()
            .filter(|(source, _)| !was.contains_key(*source))
            .map(|(source, chance)| format!("{source} {chance}"))
            .collect(),
        gone: was
            .iter()
            .filter(|(source, _)| !now.contains_key(*source))
            .map(|(source, chance)| format!("{source} {chance}"))
            .collect(),
        rechanced: was
            .iter()
            .filter_map(|(source, chance)| {
                let fresh = now.get(source)?;
                (fresh != chance).then(|| format!("{source} {chance} -> {fresh}"))
            })
            .collect(),
    }
}

/// One item's sources read back as source and chance. The source itself can hold spaces, so
/// the chance is taken off the end.
fn chances(line: &str) -> BTreeMap<&str, &str> {
    line.split("; ")
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| entry.rsplit_once(' '))
        .collect()
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

    #[test]
    fn an_item_that_starts_dropping_elsewhere_is_reported() {
        let before = with_drops(&[("/Lotus/Ash", "Stalker 5.000%")]);
        let after = with_drops(&[("/Lotus/Ash", "Grineer Lancer 0.500%; Stalker 5.000%")]);

        let diff = compare(&before, &after);

        assert_eq!(diff.drops_changed.len(), 1);
        assert_eq!(diff.drops_changed[0].item, "/Lotus/Ash");
        assert_eq!(diff.drops_changed[0].added, ["Grineer Lancer 0.500%"]);
        assert!(diff.drops_changed[0].gone.is_empty());
        assert!(diff.drops_changed[0].rechanced.is_empty());
        assert!(!diff.is_empty());
    }

    #[test]
    fn a_source_that_pays_out_less_often_is_reported_with_both_chances() {
        let before = with_drops(&[("/Lotus/Relic", "Zariman/Oro Works 3.030%; Stalker 5.000%")]);
        let after = with_drops(&[("/Lotus/Relic", "Zariman/Oro Works 2.650%")]);

        let moved = &compare(&before, &after).drops_changed[0];

        assert_eq!(moved.rechanced, ["Zariman/Oro Works 3.030% -> 2.650%"]);
        assert_eq!(moved.gone, ["Stalker 5.000%"]);
    }

    #[test]
    fn an_item_that_only_arrived_is_not_also_reported_as_moved() {
        let before = with_drops(&[]);
        let after = with_drops(&[("/Lotus/Ash", "Stalker 5.000%")]);

        assert!(compare(&before, &after).drops_changed.is_empty());
    }

    fn with_drops(drops: &[(&str, &str)]) -> State {
        State {
            drops: drops
                .iter()
                .map(|(item, from)| ((*item).to_string(), (*from).to_string()))
                .collect(),
            ..state(&[], &[])
        }
    }

    fn state(items: &[&str], findings: &[(&str, usize)]) -> State {
        State {
            version: None,
            recipe: None,
            items: items.iter().map(|s| s.to_string()).collect(),
            sets: Vec::new(),
            findings: findings
                .iter()
                .map(|(r, c)| ((*r).to_string(), *c))
                .collect(),
            drops: BTreeMap::new(),
            waiting: BTreeMap::new(),
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

    #[test]
    fn a_name_keeps_the_day_it_was_first_seen() {
        let was = BTreeMap::from([(
            "drops".to_string(),
            BTreeMap::from([
                ("Meso V17 Relic".to_string(), 1_000),
                ("Gone Relic".to_string(), 2_000),
            ]),
        )]);
        let now = [
            ("drops".to_string(), "Meso V17 Relic".to_string()),
            ("drops".to_string(), "Narin Blueprint".to_string()),
        ];

        let out = waiting(&was, now.into_iter(), 9_000);

        assert_eq!(out["drops"]["Meso V17 Relic"], 1_000);
        assert_eq!(out["drops"]["Narin Blueprint"], 9_000);
        assert!(!out["drops"].contains_key("Gone Relic"));
    }
}
