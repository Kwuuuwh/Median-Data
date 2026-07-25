use std::path::Path;

use anyhow::{Context, Result};
use graph::{Graph, Node, Rel};
use serde::Deserialize;

use crate::finding::{Finding, Layer};

/// Facts a human checked by hand. The build has to keep reproducing them.
#[derive(Debug, Default, Deserialize)]
pub struct Anchors {
    #[serde(default)]
    pub item: Vec<Item>,
    #[serde(default)]
    pub set: Vec<Set>,
}

#[derive(Debug, Deserialize)]
pub struct Item {
    pub path: String,
    pub name_en: Option<String>,
    pub prime: Option<bool>,
    pub tradable: Option<bool>,
    pub ducats: Option<i64>,
    /// Why this fact is worth pinning, for whoever reads a failure.
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Set {
    pub slug: String,
    pub parts: Option<usize>,
    pub built: Option<String>,
}

/// Read anchors from a TOML file. A missing file means none are pinned yet.
pub fn load(path: &Path) -> Result<Anchors> {
    if !path.exists() {
        return Ok(Anchors::default());
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// Check every anchor against the built graph.
pub fn check(graph: &Graph, anchors: &Anchors) -> Vec<Finding> {
    let mut out = Vec::new();
    for want in &anchors.item {
        check_item(graph, want, &mut out);
    }
    for want in &anchors.set {
        check_set(graph, want, &mut out);
    }
    out
}

fn check_item(graph: &Graph, want: &Item, out: &mut Vec<Finding>) {
    let Some(Node::Item(got)) = graph.get(&want.path) else {
        out.push(fail(&want.path, "anchored item is not in the catalog", want.note.as_deref()));
        return;
    };

    let mut differs = |field: &str, expected: String, actual: String| {
        out.push(fail(
            &want.path,
            &format!("{field}: expected {expected}, built {actual}"),
            want.note.as_deref(),
        ));
    };

    if let Some(name) = &want.name_en
        && *name != got.names.en.value
    {
        differs("name_en", name.clone(), got.names.en.value.clone());
    }
    if let Some(prime) = want.prime
        && prime != got.prime.value
    {
        differs("prime", prime.to_string(), got.prime.value.to_string());
    }
    if let Some(tradable) = want.tradable {
        let actual = got.tradable.as_ref().is_some_and(|t| t.value);
        if tradable != actual {
            differs("tradable", tradable.to_string(), actual.to_string());
        }
    }
    if let Some(ducats) = want.ducats
        && Some(ducats) != got.ducats
    {
        differs(
            "ducats",
            ducats.to_string(),
            got.ducats.map_or("none".to_string(), |d| d.to_string()),
        );
    }
}

fn check_set(graph: &Graph, want: &Set, out: &mut Vec<Finding>) {
    let id = graph::set_id(&want.slug);
    if !graph.has(&id) {
        out.push(fail(&want.slug, "anchored set is not in the catalog", None));
        return;
    }
    if let Some(parts) = want.parts {
        let actual = graph
            .from(&id)
            .iter()
            .filter(|e| matches!(e.rel, Rel::Member))
            .count();
        if parts != actual {
            out.push(fail(
                &want.slug,
                &format!("parts: expected {parts}, built {actual}"),
                None,
            ));
        }
    }
    if let Some(built) = &want.built {
        let actual = graph
            .from(&id)
            .into_iter()
            .find(|e| matches!(e.rel, Rel::Represents))
            .map(|e| e.to.clone());
        if Some(built) != actual.as_ref() {
            out.push(fail(
                &want.slug,
                &format!(
                    "assembles: expected {built}, built {}",
                    actual.unwrap_or_else(|| "nothing".to_string())
                ),
                None,
            ));
        }
    }
}

fn fail(entity: &str, detail: &str, note: Option<&str>) -> Finding {
    let detail = match note {
        Some(note) => format!("{detail} ({note})"),
        None => detail.to_string(),
    };
    Finding::new(Layer::Anchor, "anchor-broken", entity, detail)
}
