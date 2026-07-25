use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Where an unresolved name came from. A market listing is keyed by its slug; every other
/// source names items by what it prints, so those links are keyed by the printed name.
pub const MARKET: &str = "market";
pub const DROPS: &str = "drops";
pub const VENDOR: &str = "vendor";
pub const DOJO: &str = "dojo";

/// Decisions a person made, kept in a file so they survive every rebuild and stay
/// reviewable in the repository.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Curation {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub link: Vec<Link>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub name: Vec<Name>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pick: Vec<Pick>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub term: Vec<Term>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accept: Vec<Accept>,
}

/// A name one source uses tied to the catalog item it means, where the source's own
/// reference is missing, stale, or printed as free text.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Link {
    #[serde(default = "market")]
    pub source: String,
    #[serde(alias = "slug")]
    pub key: String,
    pub item: String,
}

fn market() -> String {
    MARKET.to_string()
}

/// A Russian name written by hand, where no source carries one.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Name {
    pub item: String,
    pub ru: String,
}

/// The value a person kept where sources disagreed.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pick {
    pub item: String,
    pub prop: String,
    pub value: String,
}

/// A Russian word for something that is not an item: a place, a star-chart node, a vendor,
/// a lab, a mission type, a faction, or a label of our own taxonomy.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Term {
    pub kind: String,
    pub key: String,
    pub ru: String,
}

/// A finding looked at and kept: the check is right, the data is what it is. The funnel
/// stops reporting it, and the note says why it was let through.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Accept {
    pub rule: String,
    pub entity: String,
    #[serde(default)]
    pub note: String,
}

impl Curation {
    /// Market listings keyed by slug.
    pub fn market_links(&self) -> BTreeMap<&str, &str> {
        self.link
            .iter()
            .filter(|l| l.source == MARKET)
            .map(|l| (l.key.as_str(), l.item.as_str()))
            .collect()
    }

    /// Printed names tied to an item by hand, for the sources that name items by name. One
    /// spelling means the same thing whichever source printed it, so the name index reads
    /// them all together.
    pub fn named(&self) -> BTreeMap<&str, &str> {
        self.link
            .iter()
            .filter(|l| l.source != MARKET)
            .map(|l| (l.key.as_str(), l.item.as_str()))
            .collect()
    }

    /// Hand-written Russian names keyed by catalog path.
    pub fn names(&self) -> BTreeMap<&str, &str> {
        self.name
            .iter()
            .map(|n| (n.item.as_str(), n.ru.as_str()))
            .collect()
    }

    /// Resolved conflicts keyed by item and property.
    pub fn picks(&self) -> BTreeMap<(&str, &str), &str> {
        self.pick
            .iter()
            .map(|p| ((p.item.as_str(), p.prop.as_str()), p.value.as_str()))
            .collect()
    }

    /// Hand-written words keyed by what they name.
    pub fn terms(&self) -> Terms<'_> {
        Terms(
            self.term
                .iter()
                .map(|t| ((t.kind.as_str(), t.key.as_str()), t.ru.as_str()))
                .collect(),
        )
    }

    /// Findings let through on purpose, keyed by check and entity.
    pub fn accepted(&self) -> BTreeSet<(&str, &str)> {
        self.accept
            .iter()
            .map(|a| (a.rule.as_str(), a.entity.as_str()))
            .collect()
    }

    /// Record a mapping, replacing any earlier one for the same key.
    pub fn set_link(&mut self, source: &str, key: &str, item: &str) {
        self.clear_link(source, key);
        if item.trim().is_empty() {
            return;
        }
        self.link.push(Link {
            source: source.to_string(),
            key: key.to_string(),
            item: item.trim().to_string(),
        });
        self.link
            .sort_by(|a, b| (&a.source, &a.key).cmp(&(&b.source, &b.key)));
    }

    /// Forget a mapping, letting the build derive it again.
    pub fn clear_link(&mut self, source: &str, key: &str) {
        self.link.retain(|l| l.source != source || l.key != key);
    }

    /// Record a Russian name. An empty name clears it.
    pub fn set_name(&mut self, item: &str, ru: &str) {
        self.name.retain(|n| n.item != item);
        if !ru.trim().is_empty() {
            self.name.push(Name {
                item: item.to_string(),
                ru: ru.trim().to_string(),
            });
            self.name.sort_by(|a, b| a.item.cmp(&b.item));
        }
    }

    /// Record the value kept for a property. An empty value clears the decision.
    pub fn set_pick(&mut self, item: &str, prop: &str, value: &str) {
        self.pick.retain(|p| p.item != item || p.prop != prop);
        if !value.trim().is_empty() {
            self.pick.push(Pick {
                item: item.to_string(),
                prop: prop.to_string(),
                value: value.trim().to_string(),
            });
            self.pick
                .sort_by(|a, b| (&a.item, &a.prop).cmp(&(&b.item, &b.prop)));
        }
    }

    /// Record a Russian word for something that is not an item. An empty word clears it.
    pub fn set_term(&mut self, kind: &str, key: &str, ru: &str) {
        self.term.retain(|t| t.kind != kind || t.key != key);
        if !ru.trim().is_empty() {
            self.term.push(Term {
                kind: kind.to_string(),
                key: key.to_string(),
                ru: ru.trim().to_string(),
            });
            self.term
                .sort_by(|a, b| (&a.kind, &a.key).cmp(&(&b.kind, &b.key)));
        }
    }

    /// Let a finding through, with the reason it is not a defect.
    pub fn set_accept(&mut self, rule: &str, entity: &str, note: &str) {
        self.clear_accept(rule, entity);
        self.accept.push(Accept {
            rule: rule.to_string(),
            entity: entity.to_string(),
            note: note.trim().to_string(),
        });
        self.accept
            .sort_by(|a, b| (&a.rule, &a.entity).cmp(&(&b.rule, &b.entity)));
    }

    /// Report a finding again.
    pub fn clear_accept(&mut self, rule: &str, entity: &str) {
        self.accept.retain(|a| a.rule != rule || a.entity != entity);
    }

    /// Every decision on record, for review and undo.
    pub fn decided(&self) -> studio::Decided {
        studio::Decided {
            links: self
                .link
                .iter()
                .map(|l| (l.source.clone(), l.key.clone(), l.item.clone()))
                .collect(),
            names: self
                .name
                .iter()
                .map(|n| (n.item.clone(), n.ru.clone()))
                .collect(),
            picks: self
                .pick
                .iter()
                .map(|p| (p.item.clone(), p.prop.clone(), p.value.clone()))
                .collect(),
            terms: self
                .term
                .iter()
                .map(|t| (t.kind.clone(), t.key.clone(), t.ru.clone()))
                .collect(),
        }
    }
}

/// Hand-written words, looked up by what they name.
#[derive(Debug, Default)]
pub struct Terms<'a>(BTreeMap<(&'a str, &'a str), &'a str>);

impl<'a> Terms<'a> {
    pub fn get(&self, kind: &str, key: &str) -> Option<&'a str> {
        self.0.get(&(kind, key)).copied()
    }

    /// The word, or what the source already carries.
    pub fn or(&self, kind: &str, key: &str, current: Option<String>) -> Option<String> {
        match self.get(kind, key) {
            Some(ru) => Some(ru.to_string()),
            None => current,
        }
    }
}

/// Read curated decisions. A missing file means none were made yet.
pub fn load(path: &Path) -> Result<Curation> {
    if !path.exists() {
        return Ok(Curation::default());
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// Write curated decisions back, keeping the header that explains the file.
pub fn save(path: &Path, curation: &Curation) -> Result<()> {
    const HEADER: &str = "# Decisions made by hand in Studio. These outrank every derived\n\
                          # value, and the build reads them on every run.\n\n";
    let body = toml::to_string_pretty(curation)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, format!("{HEADER}{body}"))
        .with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_later_decision_replaces_the_earlier_one() {
        let mut c = Curation::default();
        c.set_link(MARKET, "quick_thinking", "/A");
        c.set_link(MARKET, "quick_thinking", "/B");
        assert_eq!(c.link.len(), 1);
        assert_eq!(c.market_links()["quick_thinking"], "/B");
    }

    #[test]
    fn the_same_key_in_two_sources_is_two_decisions() {
        let mut c = Curation::default();
        c.set_link(MARKET, "endo", "/A");
        c.set_link(DROPS, "endo", "/B");
        assert_eq!(c.link.len(), 2);
        assert_eq!(c.market_links()["endo"], "/A");
        assert_eq!(c.named()["endo"], "/B");
    }

    #[test]
    fn links_stay_sorted_so_the_file_diffs_cleanly() {
        let mut c = Curation::default();
        c.set_link(MARKET, "zeta", "/Z");
        c.set_link(MARKET, "alpha", "/A");
        assert_eq!(c.link[0].key, "alpha");
    }

    #[test]
    fn clearing_a_link_removes_it() {
        let mut c = Curation::default();
        c.set_link(MARKET, "axi_a10_relic", "/Relic");
        c.clear_link(MARKET, "axi_a10_relic");
        assert!(c.link.is_empty());
    }

    #[test]
    fn a_link_written_before_sources_existed_still_reads() {
        let c: Curation = toml::from_str("[[link]]\nslug = \"forma\"\nitem = \"/F\"\n").unwrap();
        assert_eq!(c.market_links()["forma"], "/F");
    }

    #[test]
    fn an_empty_name_clears_rather_than_stores() {
        let mut c = Curation::default();
        c.set_name("/A", "Вольт Прайм");
        assert_eq!(c.names()["/A"], "Вольт Прайм");
        c.set_name("/A", "  ");
        assert!(c.name.is_empty());
    }

    #[test]
    fn picks_are_keyed_by_item_and_property() {
        let mut c = Curation::default();
        c.set_pick("/A", "prime", "true");
        c.set_pick("/A", "tradable", "false");
        assert_eq!(c.picks()[&("/A", "prime")], "true");
        assert_eq!(c.picks()[&("/A", "tradable")], "false");
    }

    #[test]
    fn a_term_names_a_thing_that_is_not_an_item() {
        let mut c = Curation::default();
        c.set_term("place", "Earth/Mariana", "Земля/Мариана");
        let terms = c.terms();
        assert_eq!(terms.get("place", "Earth/Mariana").unwrap(), "Земля/Мариана");
        assert_eq!(terms.get("region", "Earth/Mariana"), None);
    }

    #[test]
    fn an_accepted_finding_carries_its_reason() {
        let mut c = Curation::default();
        c.set_accept("dead-recipe", "/Control", "фармится напрямую");
        assert!(c.accepted().contains(&("dead-recipe", "/Control")));
        c.clear_accept("dead-recipe", "/Control");
        assert!(c.accept.is_empty());
    }
}
