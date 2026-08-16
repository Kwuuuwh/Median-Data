use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result};
use graph::{Class, Kind, Taxonomy};
use serde::Deserialize;

use crate::extract::DeItem;

/// What a blueprint DE only knows as a recipe key is filed as.
pub const BLUEPRINT: &str = "Blueprint";

/// The taxonomy file: the declared tree, then the ordered rules that place an item in it.
#[derive(Debug, Default, Deserialize)]
struct File {
    #[serde(default)]
    class: Vec<Class>,
    #[serde(default)]
    rule: Vec<Rule>,
}

/// One classification rule. Every matcher it names has to match; within one matcher any
/// listed value counts.
#[derive(Debug, Deserialize)]
pub struct Rule {
    pub kind: String,
    pub reason: String,
    #[serde(default)]
    pub array: Vec<String>,
    #[serde(default)]
    pub category: Vec<String>,
    #[serde(default)]
    pub parent: Vec<String>,
    #[serde(default)]
    pub parent_starts: Vec<String>,
    #[serde(default)]
    pub path_contains: Vec<String>,
    #[serde(default)]
    pub mod_type: Vec<String>,
    /// A rule that catches whatever is left rather than recognising anything. What it assigns
    /// is a placeholder, and the funnel counts it as unclassified.
    #[serde(default)]
    pub fallback: bool,
}

/// What a rule reads about one item.
pub struct Facts<'a> {
    pub path: &'a str,
    pub array: &'a str,
    pub category: &'a str,
    pub parent: Option<&'a str>,
    pub mod_type: Option<&'a str>,
}

/// The declared tree with the rules that fill it.
#[derive(Debug)]
pub struct Policy {
    pub tree: Taxonomy,
    rules: Vec<Rule>,
}

impl Policy {
    /// The kind an item falls into. Nothing matching leaves the item unknown, which the
    /// funnel reports rather than guessing.
    pub fn classify(&self, facts: &Facts<'_>) -> Kind {
        match self.decide(facts) {
            Some(i) => Kind::new(self.rules[i].kind.clone()),
            None => Kind::unknown(),
        }
    }

    /// The kind of an item DE only knows as a recipe key.
    pub fn classify_blueprint(&self, path: &str) -> Kind {
        self.classify(&blueprint_facts(path))
    }

    /// Kinds that only a catch-all rule assigns, so nothing about them was recognised.
    pub fn provisional(&self) -> BTreeSet<String> {
        self.rules
            .iter()
            .filter(|r| r.fallback)
            .map(|r| r.kind.clone())
            .collect()
    }

    /// Rules that decide nothing in this data. A rule that never fires is either wrong or
    /// left over from a manifest DE has changed.
    pub fn unused<'f>(&self, facts: impl IntoIterator<Item = Facts<'f>>) -> Vec<&Rule> {
        let mut fired = vec![false; self.rules.len()];
        for f in facts {
            if let Some(i) = self.decide(&f) {
                fired[i] = true;
            }
        }
        self.rules
            .iter()
            .zip(fired)
            .filter(|(_, hit)| !hit)
            .map(|(rule, _)| rule)
            .collect()
    }

    /// Index of the first rule that matches.
    fn decide(&self, facts: &Facts<'_>) -> Option<usize> {
        self.rules.iter().position(|r| r.matches(facts))
    }
}

/// What the rules read about a DE item.
pub fn facts(de: &DeItem) -> Facts<'_> {
    Facts {
        path: &de.unique_name,
        array: &de.array,
        category: &de.category,
        parent: de.parent.as_deref(),
        mod_type: de.mod_type.as_deref(),
    }
}

/// What the rules read about an item built from a recipe key.
pub fn blueprint_facts(path: &str) -> Facts<'_> {
    Facts {
        path,
        array: "",
        category: BLUEPRINT,
        parent: None,
        mod_type: None,
    }
}

impl Rule {
    fn matches(&self, f: &Facts<'_>) -> bool {
        eq(&self.array, Some(f.array))
            && eq(&self.category, Some(f.category))
            && eq(&self.parent, f.parent)
            && any(&self.parent_starts, |p| {
                f.parent.is_some_and(|v| v.starts_with(p))
            })
            && any(&self.path_contains, |p| f.path.contains(p))
            && eq(&self.mod_type, f.mod_type)
    }
}

/// Read the tree and its rules. Every rule has to name a declared kind.
pub fn load(path: &Path) -> Result<Policy> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let file: File = toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    let tree = Taxonomy::new(file.class).map_err(anyhow::Error::msg)?;
    for rule in &file.rule {
        if !tree.has(&rule.kind) {
            anyhow::bail!(
                "{}: rule '{}' names a kind no class declares",
                path.display(),
                rule.kind
            );
        }
    }
    if !tree.has(Kind::UNKNOWN) {
        anyhow::bail!("{}: no class declares '{}'", path.display(), Kind::UNKNOWN);
    }
    Ok(Policy {
        tree,
        rules: file.rule,
    })
}

/// An empty matcher does not constrain the rule.
fn any(values: &[String], hit: impl Fn(&str) -> bool) -> bool {
    values.is_empty() || values.iter().any(|v| hit(v))
}

fn eq(values: &[String], value: Option<&str>) -> bool {
    any(values, |v| value == Some(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(rules: &str) -> Policy {
        let text = format!(
            "{}\n{rules}",
            r#"
[[class]]
slug = "mod"
en = "Mods"
ru = "Моды"
kind = [
  { slug = "aura", en = "Aura", ru = "Аура" },
  { slug = "riven", en = "Riven", ru = "Разлом" },
]

[[class]]
slug = "unknown"
en = "Unclassified"
ru = "Не определено"
kind = [{ slug = "unknown", en = "Unclassified", ru = "Не определено" }]
"#
        );
        let file: File = toml::from_str(&text).unwrap();
        Policy {
            tree: Taxonomy::new(file.class).unwrap(),
            rules: file.rule,
        }
    }

    fn facts<'a>(path: &'a str, array: &'a str, category: &'a str) -> Facts<'a> {
        Facts {
            path,
            array,
            category,
            parent: None,
            mod_type: None,
        }
    }

    #[test]
    fn the_first_matching_rule_wins() {
        let p = policy(
            r#"
[[rule]]
kind = "riven"
reason = "path"
path_contains = ["/Randomized/"]

[[rule]]
kind = "aura"
reason = "mod type"
mod_type = ["AURA"]
"#,
        );
        let mut f = facts(
            "/Lotus/Upgrades/Mods/Randomized/X",
            "ExportUpgrades",
            "ExportUpgrades",
        );
        f.mod_type = Some("AURA");
        assert_eq!(p.classify(&f).as_str(), "riven");
    }

    #[test]
    fn every_matcher_of_a_rule_has_to_match() {
        let p = policy(
            r#"
[[rule]]
kind = "aura"
reason = "type in the mod manifest"
array = ["ExportUpgrades"]
mod_type = ["AURA"]
"#,
        );
        let mut f = facts("/X", "ExportAvionics", "ExportUpgrades");
        f.mod_type = Some("AURA");
        assert!(p.classify(&f).is_unknown());
        let mut f = facts("/X", "ExportUpgrades", "ExportUpgrades");
        f.mod_type = Some("AURA");
        assert_eq!(p.classify(&f).as_str(), "aura");
    }

    #[test]
    fn an_unmatched_item_is_unknown_not_a_guess() {
        let p = policy("");
        assert!(
            p.classify(&facts("/X", "ExportResources", "ExportResources"))
                .is_unknown()
        );
    }

    #[test]
    fn a_parent_prefix_matches_the_whole_family() {
        let p = policy(
            r#"
[[rule]]
kind = "aura"
reason = "parentName"
parent_starts = ["/Lotus/Types/Items/Fish/"]
"#,
        );
        let mut f = facts("/X", "ExportResources", "ExportResources");
        f.parent = Some("/Lotus/Types/Items/Fish/Deimos/InfestedCommonAFishItem");
        assert_eq!(p.classify(&f).as_str(), "aura");
        f.parent = Some("/Lotus/Types/Items/Gems/GemItem");
        assert!(p.classify(&f).is_unknown());
    }

    #[test]
    fn a_rule_shadowed_by_an_earlier_one_counts_as_unused() {
        let p = policy(
            r#"
[[rule]]
kind = "riven"
reason = "path"
path_contains = ["/Mods/"]

[[rule]]
kind = "aura"
reason = "shadowed: every mod path already matched above"
mod_type = ["AURA"]
"#,
        );
        let mut f = facts("/Lotus/Upgrades/Mods/X", "ExportUpgrades", "ExportUpgrades");
        f.mod_type = Some("AURA");
        let unused = p.unused([f]);
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].kind, "aura");
    }

    #[test]
    fn a_rule_naming_an_undeclared_kind_is_refused() {
        let text = r#"
[[class]]
slug = "unknown"
en = "u"
ru = "u"
kind = [{ slug = "unknown", en = "u", ru = "u" }]

[[rule]]
kind = "dropship"
reason = "typo"
"#;
        let dir = std::env::temp_dir().join("median-taxonomy-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("taxonomy.toml");
        std::fs::write(&path, text).unwrap();
        let err = load(&path).unwrap_err().to_string();
        assert!(err.contains("dropship"), "{err}");
    }
}
