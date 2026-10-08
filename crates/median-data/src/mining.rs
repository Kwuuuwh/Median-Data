use graph::{DropInfo, Edge, Graph, Node, Place, PlaceKind, Rel, place_id, printed};
use serde::{Deserialize, Serialize};
use sources::notation::{Block, Value};

use crate::areas::Areas;
use crate::lineage::Lineage;
use crate::normalize;

/// The groups a mining manifest rolls, and the kind each is printed as.
const GROUPS: [(&str, Kind); 3] = [
    ("Ores", Kind::Ore),
    ("Gems", Kind::Gem),
    ("SpecialGems", Kind::Special),
];

/// The game writes chances in single precision; this many decimals of them are real.
const PLACES: f64 = 1e6;

/// What the open worlds' veins yield, in the game's own paths.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Mining {
    #[serde(default)]
    pub site: Vec<Site>,
}

/// One mining manifest: what its veins yield and the tools that work them.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Site {
    pub manifest: String,
    #[serde(rename = "yield")]
    pub yields: Vec<Yield>,
    pub tool: Vec<Tool>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Yield {
    pub kind: Kind,
    pub item: String,
    /// Chance within its group.
    pub chance: f64,
    /// Chance of the tier the item sits in: the first roll of its group.
    pub tier: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Ore,
    Gem,
    /// Rolled only by tools with a special chance above zero.
    Special,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub weapon: String,
    pub gem: f64,
    pub special: f64,
}

/// Every manifest that sets its own vein groups.
pub fn read(lineage: &Lineage) -> Mining {
    let mut site: Vec<Site> = lineage
        .paths()
        .filter(|path| GROUPS.iter().any(|(key, _)| lineage.sets(path, key)))
        .filter_map(|path| Some(read_site(path, &lineage.own(path)?)))
        .collect();
    site.sort_by(|one, other| one.manifest.cmp(&other.manifest));
    Mining { site }
}

/// A place for every mine an area names, dropping what its veins yield. Says how many
/// drops it drew.
pub fn link(graph: &mut Graph, mining: &Mining, areas: &Areas) -> usize {
    let mut drawn = 0;
    for mine in areas.area.iter().flat_map(|area| &area.mines) {
        let Some(site) = mining
            .site
            .iter()
            .find(|site| site.manifest == mine.manifest)
        else {
            continue;
        };
        graph.insert(Node::Place(Place {
            name: mine.name.clone(),
            name_ru: Some(mine.ru.clone()),
            kind: PlaceKind::Mining,
            bounty: None,
            table: None,
        }));
        let special = site
            .tool
            .iter()
            .map(|tool| tool.special)
            .fold(0.0, f64::max);
        for vein in &site.yields {
            let item = normalize::path(&vein.item).into_owned();
            if !graph.has(&item) {
                continue;
            }
            // A special gem turns up only in the share of veins the best tool unlocks.
            let (chance, tier) = match vein.kind {
                Kind::Special => (vein.chance * special, vein.tier * special),
                Kind::Ore | Kind::Gem => (vein.chance, vein.tier),
            };
            graph.link(Edge {
                from: place_id(&mine.name),
                to: item,
                rel: Rel::Drops(DropInfo {
                    rarity: printed(tier).to_string(),
                    chance,
                    rotation: None,
                    stage: None,
                    table_chance: None,
                    levels: None,
                    count: None,
                }),
            });
            drawn += 1;
        }
    }
    drawn
}

fn read_site(path: &str, own: &Block) -> Site {
    let mut yields = Vec::new();
    for (key, kind) in GROUPS {
        if let Some(group) = own.block(key) {
            roll(group, None, 1.0, kind, &mut yields);
        }
    }
    let tool = own
        .block("MiningTools")
        .map(|listed| {
            blocks(listed)
                .filter_map(|tool| {
                    Some(Tool {
                        weapon: tool.text("WeaponType")?.to_string(),
                        gem: round(number(tool, "GemChance")),
                        special: round(number(tool, "SpecialGemChance")),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Site {
        manifest: path.to_string(),
        yields,
        tool,
    }
}

/// Walk a distribution, nested ones included, multiplying the chances on the way down.
fn roll(group: &Block, tier: Option<f64>, odds: f64, kind: Kind, out: &mut Vec<Yield>) {
    let Some(listed) = group.block("ItemDistribution") else {
        return;
    };
    for line in blocks(listed) {
        let chance = number(line, "Probability");
        let tier = tier.unwrap_or(chance);
        let odds = odds * chance;
        if let Some(nested) = line.block("ResultItems") {
            roll(nested, Some(tier), odds, kind, out);
        } else if let Some(item) = line.text("Item") {
            out.push(Yield {
                kind,
                item: item.to_string(),
                chance: round(odds),
                tier: round(tier),
            });
        }
    }
}

fn round(chance: f64) -> f64 {
    (chance * PLACES).round() / PLACES
}

fn number(block: &Block, key: &str) -> f64 {
    block
        .text(key)
        .and_then(|text| text.parse().ok())
        .unwrap_or(0.0)
}

fn blocks(listed: &Block) -> impl Iterator<Item = &Block> {
    listed.items.iter().filter_map(Value::block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineage::tests::kind;

    const DEIMOS: &str = "Ores={
ItemDistribution={
{
IsDistribution=1
ResultItems={
ItemDistribution={
{
IsDistribution=0
Item=/Lotus/StoreItems/Types/Items/Gems/Deimos/DeimosCommonOreAItem
Probability=0.25
},
{
IsDistribution=0
Item=/Lotus/StoreItems/Types/Items/Gems/Deimos/DeimosCommonOreBItem
Probability=0.75
}
}
}
Probability=0.72000003
}
}
}
MiningTools={
{
WeaponType=/Lotus/Weapons/Tenno/Gear/MiningLaserCWeapon
GemChance=0.1
SpecialGemChance=0.15000001
}
}
";

    #[test]
    fn a_nested_roll_multiplies_out() {
        let lineage = Lineage::new(vec![kind(
            "/Lotus/Types/Game/Mining/InfestedMicroplanetMiningManifest",
            None,
            Some(DEIMOS),
        )]);

        let site = &read(&lineage).site[0];

        assert_eq!(
            site.yields[0],
            Yield {
                kind: Kind::Ore,
                item: "/Lotus/StoreItems/Types/Items/Gems/Deimos/DeimosCommonOreAItem".into(),
                chance: 0.18,
                tier: 0.72,
            }
        );
        assert_eq!(site.yields[1].chance, 0.54);
        assert_eq!(site.tool[0].special, 0.15);
    }
}
