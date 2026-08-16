use std::collections::BTreeMap;

use consensus::Status;
use graph::{Graph, Taxonomy};

use crate::finding::{Finding, Layer};

/// The entity a report about the build itself is filed under, since no catalog path owns it.
const BUILD: &str = "build";

/// What silence looks like when it is measured rather than assumed. Two things pass here that
/// no other layer can see: a property nothing ever confirmed, and a rule that decided nothing.
/// Both are reported per class rather than per item — one line a person reads beats six
/// hundred nobody does.
pub fn check(
    graph: &Graph,
    taxonomy: &Taxonomy,
    dead_rules: &[String],
    unread_headings: &[String],
) -> Vec<Finding> {
    let mut alone: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut confirmed: BTreeMap<(&str, &str), usize> = BTreeMap::new();

    for item in graph.items() {
        let class = taxonomy.class_slug(&item.kind.value);
        let mut tally = |prop: &'static str, status: Status| {
            let counter = match status {
                Status::Single => &mut alone,
                Status::Confirmed => &mut confirmed,
                Status::Conflict => return,
            };
            *counter.entry((class, prop)).or_default() += 1;
        };
        tally("name_en", item.names.en.status);
        tally("kind", item.kind.status);
        tally("prime", item.prime.status);
        if let Some(ru) = &item.names.ru {
            tally("name_ru", ru.status);
        }
        if let Some(t) = &item.tradable {
            tally("tradable", t.status);
        }
        if let Some(v) = &item.vaulted {
            tally("vaulted", v.status);
        }
    }

    let mut out = Vec::new();
    for ((class, prop), count) in alone {
        if confirmed.contains_key(&(class, prop)) {
            continue;
        }
        out.push(Finding::new(
            Layer::Silence,
            "never-confirmed",
            BUILD,
            format!("{class}: {prop} rests on one source for all {count} of them"),
        ));
    }
    // The mirror of a dead rule: a kind declared and never assigned. One of the two is wrong,
    // and neither shows up as a wrong value anywhere.
    let mut filled: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for item in graph.items() {
        filled.insert(item.kind.value.as_str());
    }
    for class in taxonomy.classes() {
        for leaf in &class.kind {
            if leaf.slug != graph::Kind::UNKNOWN && !filled.contains(leaf.slug.as_str()) {
                out.push(Finding::new(
                    Layer::Silence,
                    "kind-holds-nothing",
                    BUILD,
                    format!("kind '{}' was declared and nothing landed in it", leaf.slug),
                ));
            }
        }
    }
    for heading in unread_headings {
        out.push(Finding::new(
            Layer::Silence,
            "heading-unread",
            BUILD,
            format!("a source printed '{heading}' where nothing says what it is"),
        ));
    }
    for rule in dead_rules {
        out.push(Finding::new(
            Layer::Silence,
            "rule-never-fired",
            BUILD,
            rule.clone(),
        ));
    }
    out
}
