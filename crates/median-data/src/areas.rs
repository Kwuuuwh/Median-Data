use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result};
use graph::{Area, Edge, Graph, Node, Rel, area_id};
use serde::Deserialize;

/// The areas, as written by hand.
#[derive(Debug, Default, Deserialize)]
pub struct Areas {
    #[serde(default)]
    pub area: Vec<Written>,
}

/// One area and what stands in it.
#[derive(Debug, Deserialize)]
pub struct Written {
    pub key: String,
    pub name: String,
    pub ru: String,
    /// Star-chart tilesets whose nodes stand in the area, and the places those nodes hold.
    #[serde(default)]
    pub tilesets: Vec<String>,
    /// Bounty settlements whose tables the area hands out.
    #[serde(default)]
    pub settlements: Vec<String>,
    /// Locations the drop tables print for the area's places.
    #[serde(default)]
    pub locations: Vec<String>,
    /// Places named outright, where nothing else ties them to the area.
    #[serde(default)]
    pub places: Vec<String>,
    /// Vendor keys.
    #[serde(default)]
    pub vendors: Vec<String>,
}

/// Read the areas. A missing file leaves every place without one.
pub fn load(path: &Path) -> Result<Areas> {
    if !path.exists() {
        return Ok(Areas::default());
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// An area node for each one written, and an edge from every star-chart node, place and vendor
/// that stands in it. Says how many edges it drew.
pub fn link(graph: &mut Graph, areas: &Areas) -> usize {
    let mut edges = Vec::new();
    for area in &areas.area {
        let to = area_id(&area.key);
        let regions: BTreeSet<String> = graph
            .nodes()
            .filter_map(|node| match node {
                Node::Region(region)
                    if region
                        .tileset
                        .en
                        .as_ref()
                        .is_some_and(|tileset| area.tilesets.contains(tileset)) =>
                {
                    Some(node.id())
                }
                _ => None,
            })
            .collect();
        for node in graph.nodes() {
            let within = match node {
                Node::Region(_) => regions.contains(&node.id()),
                Node::Place(place) => {
                    area.places.contains(&place.name)
                        || place
                            .bounty
                            .as_ref()
                            .is_some_and(|bounty| area.settlements.contains(&bounty.settlement))
                        || place
                            .table
                            .as_ref()
                            .is_some_and(|table| area.locations.contains(&table.location))
                        || graph
                            .from(&node.id())
                            .iter()
                            .any(|edge| edge.rel == Rel::At && regions.contains(&edge.to))
                }
                Node::Vendor(vendor) => area.vendors.contains(&vendor.key),
                _ => false,
            };
            if within {
                edges.push(Edge {
                    from: node.id(),
                    to: to.clone(),
                    rel: Rel::Within,
                });
            }
        }
    }

    for area in &areas.area {
        graph.insert(Node::Area(Area {
            key: area.key.clone(),
            name: area.name.clone(),
            name_ru: Some(area.ru.clone()),
        }));
    }
    let drawn = edges.len();
    for edge in edges {
        graph.link(edge);
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph::{Bounty, Place, PlaceKind, Vendor, place_id, vendor_id};

    fn place(name: &str, settlement: Option<&str>) -> Node {
        Node::Place(Place {
            name: name.to_string(),
            name_ru: None,
            kind: PlaceKind::Transient,
            bounty: settlement.map(|settlement| Bounty {
                settlement: settlement.to_string(),
                settlement_ru: None,
                giver: None,
                giver_ru: None,
                min_level: 5,
                max_level: 15,
                activity: String::new(),
                activity_ru: None,
            }),
            table: None,
        })
    }

    fn area(key: &str) -> Written {
        Written {
            key: key.to_string(),
            name: key.to_string(),
            ru: key.to_string(),
            tilesets: Vec::new(),
            settlements: Vec::new(),
            locations: Vec::new(),
            places: Vec::new(),
            vendors: Vec::new(),
        }
    }

    /// The area a node stands in, by its key.
    fn within(graph: &Graph, id: &str) -> Option<String> {
        graph
            .from(id)
            .into_iter()
            .find(|edge| edge.rel == Rel::Within)
            .map(|edge| edge.to.clone())
    }

    #[test]
    fn a_place_stands_in_an_area_by_name_or_settlement_and_a_vendor_by_key() {
        let mut graph = Graph::new();
        graph.insert(place("Duviri Lone Story", None));
        graph.insert(place("Level 5 - 15 Cetus Bounty", Some("Cetus")));
        graph.insert(place("Void Fissure", None));
        graph.insert(Node::Vendor(Vendor {
            key: "teshin-s-cave".to_string(),
            name: "Teshin's Cave".to_string(),
            name_ru: None,
            currency: None,
            kind: None,
            rotates: false,
        }));
        let areas = Areas {
            area: vec![
                Written {
                    places: vec!["Duviri Lone Story".to_string()],
                    vendors: vec!["teshin-s-cave".to_string()],
                    ..area("duviri")
                },
                Written {
                    settlements: vec!["Cetus".to_string()],
                    ..area("cetus")
                },
            ],
        };

        assert_eq!(link(&mut graph, &areas), 3);

        let duviri = Some(area_id("duviri"));
        assert_eq!(within(&graph, &place_id("Duviri Lone Story")), duviri);
        assert_eq!(within(&graph, &vendor_id("teshin-s-cave")), duviri);
        assert_eq!(
            within(&graph, &place_id("Level 5 - 15 Cetus Bounty")),
            Some(area_id("cetus"))
        );
        assert_eq!(within(&graph, &place_id("Void Fissure")), None);
        assert!(graph.has(&area_id("cetus")));
    }
}
