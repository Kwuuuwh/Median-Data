use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use consensus::Source;
use graph::{Edge, Graph, Label, Node, PlaceKind, Region, Rel, region_id};
use serde::Deserialize;

use crate::extract::DeRegion;

/// Names for the numbered enums of `ExportRegions`, plus the nodes it leaves out.
#[derive(Debug, Default, Deserialize)]
pub struct Labels {
    #[serde(default)]
    mission: Vec<Named>,
    #[serde(default)]
    faction: Vec<Named>,
    #[serde(default)]
    node_type: Vec<Named>,
    #[serde(default)]
    chart: Vec<Chart>,
}

/// One index and what it is called. `wiki` records a disagreement we looked at and kept: the
/// wiki calls the index that, we call it something else on purpose, and the check stays quiet
/// until the wiki changes its mind.
#[derive(Debug, Deserialize)]
struct Named {
    index: i64,
    en: String,
    ru: Option<String>,
    wiki: Option<String>,
}

/// Russian names for the star chart, grouped by the planet the nodes sit in. DE translates
/// planet names but never node names, and the wiki is English only, so this is the only place
/// a node's Russian name can come from.
#[derive(Debug, Deserialize)]
struct Chart {
    planet: String,
    planet_ru: Option<String>,
    #[serde(default)]
    node: Vec<Hand>,
}

/// One node's Russian name, tied to DE's own key for it.
#[derive(Debug, Deserialize)]
struct Hand {
    #[serde(rename = "ref")]
    key: String,
    #[allow(dead_code)]
    en: String,
    ru: Option<String>,
}

impl Labels {
    fn look(list: &[Named], index: i64) -> Label {
        match list.iter().find(|n| n.index == index) {
            Some(n) => Label {
                en: Some(n.en.clone()),
                ru: n.ru.clone(),
            },
            None => Label::default(),
        }
    }

    pub fn mission(&self, index: i64) -> Label {
        Self::look(&self.mission, index)
    }

    pub fn faction(&self, index: i64) -> Label {
        Self::look(&self.faction, index)
    }

    pub fn node_type(&self, index: i64) -> Label {
        Self::look(&self.node_type, index)
    }

    /// Whether a disagreement with the wiki is already on record for this index.
    fn settled<'a>(list: &[Named], index: i64, theirs: impl IntoIterator<Item = &'a str>) -> bool {
        let Some(kept) = list
            .iter()
            .find(|n| n.index == index)
            .and_then(|n| n.wiki.as_deref())
        else {
            return false;
        };
        theirs.into_iter().any(|t| t.eq_ignore_ascii_case(kept))
    }

    /// Whether the table knows this name as a mission type at all.
    fn names_mission(&self, name: &str) -> bool {
        self.mission.iter().any(|m| m.en.eq_ignore_ascii_case(name))
    }

    /// A label for a type the wiki names but DE gives no index for (`Skirmish`). The Russian
    /// side comes from the table when it happens to hold that name.
    fn named(list: &[Named], en: &str) -> Label {
        Label {
            en: Some(en.to_string()),
            ru: list
                .iter()
                .find(|n| n.en.eq_ignore_ascii_case(en))
                .and_then(|n| n.ru.clone()),
        }
    }

    pub fn mission_named(&self, en: &str) -> Label {
        Self::named(&self.mission, en)
    }

    pub fn faction_named(&self, en: &str) -> Label {
        Self::named(&self.faction, en)
    }

    /// The Russian name written for a node, by DE's key.
    fn node_ru(&self, key: &str) -> Option<String> {
        self.chart
            .iter()
            .flat_map(|c| &c.node)
            .find(|n| n.key == key)
            .and_then(|n| n.ru.clone())
    }

    /// The Russian name written for a planet the wiki names in English.
    fn planet_ru(&self, planet: &str) -> Option<String> {
        self.chart
            .iter()
            .find(|c| c.planet == planet)
            .and_then(|c| c.planet_ru.clone())
    }
}

/// A label with whatever Russian was written for it in Studio. The English name is the key,
/// because that is what the screen shows and what several nodes share — and because a type the
/// wiki names but DE gives no index for has nowhere else to be keyed by.
fn with_term(mut label: Label, kind: &str, terms: &crate::curation::Terms) -> Label {
    if let Some(en) = &label.en
        && let Some(ru) = terms.get(kind, en)
    {
        label.ru = Some(ru.to_string());
    }
    label
}

/// Read the reference labels. A missing file leaves every index unnamed.
pub fn load(path: &Path) -> Result<Labels> {
    if !path.exists() {
        return Ok(Labels::default());
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// What tying the star chart to the drop tables produced.
pub struct Linked {
    pub regions: usize,
    /// Nodes only the wiki knows: DE exports no Railjack at all.
    pub from_wiki: usize,
    /// Drop-table places tied to a star-chart node.
    pub bridged: usize,
    /// Places naming a planet the star chart holds and a node on it that it does not. These
    /// are real gaps.
    pub unbridged: Vec<String>,
    /// Places printed like a node but naming no planet of the star chart: past events, the
    /// Conclave playlists, the Duviri tiers, Sanctuary Onslaught. Not star-chart nodes at all.
    pub not_a_node: usize,
    /// Star-chart nodes no drop table mentions.
    pub silent: Vec<String>,
    /// Mission names the drop tables print that the reference table calls something else.
    pub mismatched: Vec<String>,
}

/// Insert the star chart and tie every drop-table place to the node it names.
pub fn link(
    graph: &mut Graph,
    de: &[DeRegion],
    ru: &BTreeMap<String, DeRegion>,
    wiki: &[crate::wiki::Node],
    labels: &Labels,
    terms: &crate::curation::Terms,
) -> Linked {
    let mut by_name: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut by_node_name: BTreeMap<String, String> = BTreeMap::new();
    // DE has no field for either flag, so both always come from the wiki.
    let flags: BTreeMap<&str, (bool, bool)> = wiki
        .iter()
        .map(|w| (w.key.as_str(), (w.railjack, w.hidden)))
        .collect();
    let mut regions = 0;
    for r in de {
        let translated = ru.get(&r.node);
        let (railjack, hidden) = flags
            .get(r.node.as_str())
            .copied()
            .unwrap_or((false, false));
        let region = Region {
            node: r.node.clone(),
            name: r.name.clone(),
            name_ru: terms.or(
                "region",
                &r.node,
                labels.node_ru(&r.node).or_else(|| {
                    translated
                        .map(|t| t.name.clone())
                        .filter(|name| *name != r.name)
                }),
            ),
            planet: r.planet.clone(),
            planet_ru: terms.or("planet", &r.planet, translated.map(|t| t.planet.clone())),
            mission: r.mission,
            mission_label: with_term(labels.mission(r.mission), "mission", terms),
            faction: r.faction,
            faction_label: with_term(labels.faction(r.faction), "faction", terms),
            node_type: r.node_type,
            type_label: with_term(labels.node_type(r.node_type), "node_type", terms),
            mastery: r.mastery,
            min_level: r.min_level,
            max_level: r.max_level,
            origin: Source::De,
            railjack,
            hidden,
        };
        by_name.insert(
            (r.planet.to_lowercase(), r.name.to_lowercase()),
            r.node.clone(),
        );
        if graph.insert(Node::Region(region)) {
            regions += 1;
        }
    }

    // Nodes DE does not export at all — Railjack, the hubs, the onslaught rooms. The drop
    // tables print Railjack under the base planet ("Saturn/Kasio's Rest"), so these are also
    // matched by node name alone.
    let mut from_wiki = 0;
    for w in wiki {
        if graph.has(&region_id(&w.key)) {
            continue;
        }
        let region = Region {
            node: w.key.clone(),
            name: w.name.clone(),
            name_ru: terms.or("region", &w.key, labels.node_ru(&w.key)),
            planet: w.planet.clone(),
            planet_ru: terms.or("planet", &w.planet, labels.planet_ru(&w.planet)),
            mission: -1,
            mission_label: with_term(
                w.mission
                    .as_deref()
                    .map(|m| labels.mission_named(m))
                    .unwrap_or_default(),
                "mission",
                terms,
            ),
            faction: -1,
            faction_label: with_term(
                w.faction
                    .as_deref()
                    .map(|f| labels.faction_named(f))
                    .unwrap_or_default(),
                "faction",
                terms,
            ),
            node_type: -1,
            type_label: Label::default(),
            mastery: 0,
            min_level: w.min_level,
            max_level: w.max_level,
            origin: Source::Wiki,
            railjack: w.railjack,
            hidden: w.hidden,
        };
        by_node_name.insert(w.name.to_lowercase(), w.key.clone());
        if graph.insert(Node::Region(region)) {
            from_wiki += 1;
        }
    }

    let printed: Vec<(String, Printed)> = graph
        .nodes()
        .filter_map(|node| match node {
            Node::Place(p) if p.kind == PlaceKind::Node => parse(&p.name).map(|it| (node.id(), it)),
            _ => None,
        })
        .collect();

    let planets: BTreeSet<String> = by_name.keys().map(|(planet, _)| planet.clone()).collect();
    let mut out = Linked {
        regions,
        from_wiki,
        bridged: 0,
        unbridged: Vec::new(),
        not_a_node: 0,
        silent: Vec::new(),
        mismatched: Vec::new(),
    };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (place, it) in &printed {
        let found = by_name
            .get(&(it.planet.to_lowercase(), it.node.to_lowercase()))
            .or_else(|| by_node_name.get(&it.node.to_lowercase()));
        let Some(node) = found else {
            if it.is_a_node() && planets.contains(&it.planet.to_lowercase()) {
                out.unbridged.push(place.clone());
            } else {
                out.not_a_node += 1;
            }
            continue;
        };
        seen.insert(node.clone());
        out.bridged += 1;
        graph.link(Edge {
            from: place.clone(),
            to: region_id(node),
            rel: Rel::At,
        });
    }

    for node in graph.nodes() {
        if let Node::Region(r) = node
            && !seen.contains(&r.node)
        {
            out.silent
                .push(format!("{}/{} [{}]", r.planet, r.name, r.origin.as_str()));
        }
    }
    out.mismatched = mismatches(graph, &printed, &by_name, labels);
    out
}

/// Mission names the drop tables print beside a node, checked against the label the
/// reference table gives that node's `missionIndex`. A node's other reward tables are printed
/// under their own name (`Caches`), which names no mission type at all, so only a printed
/// name the reference table knows counts as a claim about the mission.
fn mismatches(
    graph: &Graph,
    printed: &[(String, Printed)],
    by_name: &BTreeMap<(String, String), String>,
    labels: &Labels,
) -> Vec<String> {
    let mut said: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (_, it) in printed {
        // A past event's table names the mission the node ran back then, not today's.
        if !it.is_a_node() || !labels.names_mission(&it.mission) {
            continue;
        }
        if let Some(node) = by_name.get(&(it.planet.to_lowercase(), it.node.to_lowercase())) {
            said.entry(node.as_str()).or_default().push(&it.mission);
        }
    }

    let mut out = Vec::new();
    for (node, names) in said {
        let Some(Node::Region(region)) = graph.get(&region_id(node)) else {
            continue;
        };
        let Some(label) = region.mission_label.en.as_deref() else {
            continue;
        };
        if !names.iter().any(|n| n.eq_ignore_ascii_case(label)) {
            out.push(format!(
                "{}: reference says {label}, drop tables print {}",
                region.name,
                names.join(", ")
            ));
        }
    }
    out
}

/// What the wiki says about our reference labels.
#[derive(Default)]
pub struct Witness {
    /// A label the wiki calls something else.
    pub disagree: Vec<String>,
    /// An index we use but do not name, and what the wiki calls it.
    pub fillable: Vec<String>,
    /// Indices the wiki names the same way we do.
    pub agreed: usize,
    /// Disagreements already looked at and kept.
    pub accepted: usize,
}

/// Check the reference labels against the wiki. Mission names come straight from its
/// `MissionTypes` table, which carries DE's own index; faction names come from what it calls
/// the enemy on each node, so the index is read through the nodes that carry it.
pub fn witness(de: &[DeRegion], chart: &crate::wiki::Chart, labels: &Labels) -> Witness {
    let mut out = Witness::default();

    let used: BTreeSet<i64> = de.iter().map(|r| r.mission).collect();
    for index in used {
        match (labels.mission(index).en, chart.mission_names.get(&index)) {
            (Some(ours), Some(theirs)) if theirs.iter().any(|t| t.eq_ignore_ascii_case(&ours)) => {
                out.agreed += 1
            }
            (Some(_), Some(theirs))
                if Labels::settled(&labels.mission, index, theirs.iter().map(String::as_str)) =>
            {
                out.accepted += 1
            }
            (Some(ours), Some(theirs)) => out.disagree.push(format!(
                "mission {index}: we say {ours}, the wiki says {}",
                theirs.join(" / ")
            )),
            (None, Some(theirs)) => out.fillable.push(format!(
                "mission {index} is unnamed; the wiki calls it {}",
                theirs.join(" / ")
            )),
            _ => {}
        }
    }

    let by_key: BTreeMap<&str, &crate::wiki::Node> =
        chart.nodes.iter().map(|w| (w.key.as_str(), w)).collect();
    let mut factions: BTreeMap<i64, BTreeMap<&str, usize>> = BTreeMap::new();
    for r in de {
        if let Some(name) = by_key
            .get(r.node.as_str())
            .and_then(|w| w.faction.as_deref())
        {
            *factions
                .entry(r.faction)
                .or_default()
                .entry(name)
                .or_default() += 1;
        }
    }
    for (index, names) in factions {
        let Some((theirs, _)) = names.iter().max_by_key(|(_, n)| **n) else {
            continue;
        };
        match labels.faction(index).en {
            Some(_) if Labels::settled(&labels.faction, index, [*theirs]) => out.accepted += 1,
            Some(ours) if !ours.eq_ignore_ascii_case(theirs) => out.disagree.push(format!(
                "faction {index}: we say {ours}, the wiki says {theirs} on {} of its nodes",
                names.values().sum::<usize>()
            )),
            Some(_) => out.agreed += 1,
            None => out.fillable.push(format!(
                "faction {index} is unnamed; the wiki calls it {theirs}"
            )),
        }
    }
    out
}

/// A drop-table place name split into what it says about the star chart.
struct Printed {
    planet: String,
    node: String,
    mission: String,
    /// The table belongs to a past event, not to the live star chart.
    event: bool,
}

impl Printed {
    /// Whether this names a node the star chart is supposed to hold at all. Past events keep
    /// their tables long after their nodes are gone; the Conclave playlists are printed under
    /// a planet but are not places; and a node name carrying a colon is a label, not a node
    /// ("Endless: Tier 1").
    fn is_a_node(&self) -> bool {
        !self.event && self.mission != "Conclave" && !self.node.contains(':')
    }
}

/// Read `Planet/Node (Mission type)` as the drop tables print it. An `Event: ` prefix and a
/// trailing ` Extra` mark the same node's other reward tables, so both are dropped.
fn parse(printed: &str) -> Option<Printed> {
    let event = printed.starts_with("Event:");
    let rest = printed.strip_prefix("Event:").unwrap_or(printed).trim();
    let rest = rest.strip_suffix("Extra").unwrap_or(rest).trim_end();
    let close = rest.strip_suffix(')')?;
    let (place, mission) = close.rsplit_once('(')?;
    let (planet, node) = place.trim_end().split_once('/')?;
    Some(Printed {
        planet: planet.trim().to_string(),
        node: node.trim().to_string(),
        mission: mission.trim().to_string(),
        event,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_planet_node_and_mission() {
        let p = parse("Saturn/Anthe (Rescue)").unwrap();
        assert_eq!(
            (p.planet.as_str(), p.node.as_str(), p.mission.as_str()),
            ("Saturn", "Anthe", "Rescue")
        );
    }

    #[test]
    fn drops_the_event_prefix_and_the_extra_suffix() {
        let p = parse("Event: Uranus/Miranda (Defense)").unwrap();
        assert_eq!(p.node, "Miranda");
        let p = parse("Ceres/Exta (Assassination) Extra").unwrap();
        assert_eq!(
            (p.node.as_str(), p.mission.as_str()),
            ("Exta", "Assassination")
        );
    }

    #[test]
    fn keeps_a_node_name_holding_its_own_brackets() {
        let p = parse("Uranus/Scoria's Angel (Skirmish)").unwrap();
        assert_eq!(p.node, "Scoria's Angel");
    }

    #[test]
    fn tells_a_node_from_what_only_looks_like_one() {
        assert!(parse("Saturn/Anthe (Rescue)").unwrap().is_a_node());
        // a past event keeps its table long after the node is gone
        assert!(!parse("Event: Eris/Candiru (Caches)").unwrap().is_a_node());
        // the Conclave playlists are printed under a planet but are not places
        assert!(!parse("Saturn/Annihilation (Conclave)").unwrap().is_a_node());
        // a reward tier of the Circuit, not a node
        assert!(!parse("Duviri/Endless: Tier 1  (Hard)").unwrap().is_a_node());
    }

    #[test]
    fn an_enemy_name_is_not_a_star_chart_node() {
        assert!(parse("Grineer Lancer").is_none());
        assert!(parse("Orokin Derelict Defense").is_none());
    }

    #[test]
    fn an_unnamed_index_gets_no_label() {
        let labels = Labels::default();
        assert_eq!(labels.mission(2), Label::default());
    }
}
