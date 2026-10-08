use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use graph::{Edge, Graph, Node, Rel, Spawn, region_id};
use serde::Deserialize;

use crate::spawns::{self, Spawns};

/// The node type of an ordinary mission.
const MISSION_NODE: i64 = 0;
/// A boss fight sends its boss, not the tileset's roster.
const ASSASSINATION: &str = "MT_ASSASSINATION";

/// Which of the game's tilesets each printed tileset stands for, as written by hand.
#[derive(Debug, Default, Deserialize)]
pub struct Tilesets {
    #[serde(default)]
    pub tileset: Vec<Written>,
}

#[derive(Debug, Deserialize)]
pub struct Written {
    /// The tileset as the star chart prints it.
    pub name: String,
    /// The faction the node fields, as the star chart prints it.
    pub faction: String,
    /// The game's own tileset type.
    pub game: String,
}

/// Read the tilesets. A missing file leaves every node without a roster.
pub fn load(path: &Path) -> Result<Tilesets> {
    if !path.exists() {
        return Ok(Tilesets::default());
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// What linking drew.
#[derive(Debug, Default)]
pub struct Haunted {
    /// Nodes that got a roster.
    pub nodes: usize,
    pub edges: usize,
    /// Enemies the rosters name that no drop table knows, so nothing to farm off them.
    pub unknown: usize,
}

/// An edge from every ordinary star-chart node to each enemy it sends, by the roster its
/// tileset draws for its mission type.
pub fn link(graph: &mut Graph, spawns: &Spawns, tilesets: &Tilesets) -> Haunted {
    let mut haunted = Haunted::default();
    let mut edges = Vec::new();
    // The drop tables and the client sometimes case a name differently: `002-Er`, `002-ER`.
    let enemies: BTreeMap<String, String> = graph
        .nodes()
        .filter_map(|node| match node {
            Node::Enemy(enemy) => Some((enemy.name.to_lowercase(), node.id())),
            _ => None,
        })
        .collect();
    for node in graph.nodes() {
        let Node::Region(region) = node else {
            continue;
        };
        if region.node_type != MISSION_NODE || region.hidden || region.railjack {
            continue;
        }
        let Some(kind) = region
            .mission_code
            .as_deref()
            .filter(|kind| *kind != ASSASSINATION)
        else {
            continue;
        };
        let (Some(printed), Some(faction)) = (
            region.tileset.en.as_deref(),
            region.faction_label.en.as_deref(),
        ) else {
            continue;
        };
        let Some(tileset) = tilesets
            .tileset
            .iter()
            .find(|t| t.name == printed && t.faction == faction)
        else {
            continue;
        };
        let shares = spawns::shares(spawns, &tileset.game, kind);
        if shares.is_empty() {
            continue;
        }
        haunted.nodes += 1;
        for (name, (share, tier)) in shares {
            let Some(enemy) = enemies.get(&name.to_lowercase()).cloned() else {
                haunted.unknown += 1;
                continue;
            };
            edges.push(Edge {
                from: region_id(&region.node),
                to: enemy,
                rel: Rel::Spawns(Spawn { share, tier }),
            });
        }
    }
    haunted.edges = edges.len();
    for edge in edges {
        graph.link(edge);
    }
    haunted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spawns::{Enemy, Mission, Spec, Tileset};
    use consensus::Source;
    use graph::{Label, Region, enemy_id};

    const SEALAB: &str = "/Lotus/Types/Game/RegionTilesets/GrineerOceanTileset";
    const ROSTER: &str = "/Lotus/Types/Game/EnemySpecs/SeaLabGrineerDefenseA";

    fn label(en: &str) -> Label {
        Label {
            en: Some(en.into()),
            ..Label::default()
        }
    }

    fn node(key: &str, faction: &str) -> Node {
        Node::Region(Region {
            node: key.into(),
            name: key.into(),
            name_ru: None,
            verbatim: false,
            location: "Uranus".into(),
            mission: 2,
            mission_label: label("Defense"),
            mission_code: Some("MT_DEFENSE".into()),
            faction: 0,
            faction_label: label(faction),
            node_type: MISSION_NODE,
            type_label: Label::default(),
            mastery_req: 0,
            mastery_xp: 0,
            min_level: 24,
            max_level: 29,
            tileset: label("Grineer Sealab"),
            origin: Source::De,
            railjack: false,
            hidden: false,
        })
    }

    fn spawns() -> Spawns {
        Spawns {
            tileset: vec![Tileset {
                path: SEALAB.into(),
                mission: vec![Mission {
                    kind: "MT_DEFENSE".into(),
                    specs: vec![ROSTER.into()],
                }],
            }],
            spec: vec![Spec {
                path: ROSTER.into(),
                enemy: vec![Enemy {
                    agent: "/Lotus/Types/Enemies/Grineer/SeaLab/DrekarLancer".into(),
                    name: Some("Drekar Lancer".into()),
                    name_ru: None,
                    probability: 1.0,
                    tier: 0,
                    most: 0,
                }],
            }],
        }
    }

    #[test]
    fn a_node_sends_the_roster_of_its_tileset_and_faction_only() {
        let mut graph = Graph::new();
        graph.insert(node("SolNode64", "Grineer"));
        graph.insert(node("SolNode99", "Infested"));
        graph.insert(Node::Enemy(graph::Enemy {
            name: "Drekar Lancer".into(),
            name_ru: None,
        }));
        let tilesets = Tilesets {
            tileset: vec![Written {
                name: "Grineer Sealab".into(),
                faction: "Grineer".into(),
                game: SEALAB.into(),
            }],
        };

        let haunted = link(&mut graph, &spawns(), &tilesets);

        assert_eq!((haunted.nodes, haunted.edges), (1, 1));
        let edge = &graph.from(&region_id("SolNode64"))[0];
        assert_eq!(edge.to, enemy_id("Drekar Lancer"));
        assert_eq!(
            edge.rel,
            Rel::Spawns(Spawn {
                share: 1.0,
                tier: 0
            })
        );
        assert!(graph.from(&region_id("SolNode99")).is_empty());
    }
}
