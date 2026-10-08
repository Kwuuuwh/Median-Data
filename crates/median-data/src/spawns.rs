use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sources::notation::{Block, Value};

use crate::incubator::Phrases;
use crate::lineage::{Lineage, resolve};

const TILESETS: &str = "/Lotus/Types/Game/RegionTilesets/";

/// Which enemies each map sends at each kind of mission, in the game's own paths.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Spawns {
    #[serde(default)]
    pub tileset: Vec<Tileset>,
    #[serde(default)]
    pub spec: Vec<Spec>,
}

/// A map and the enemy specs each mission type played on it draws from.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Tileset {
    pub path: String,
    pub mission: Vec<Mission>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Mission {
    /// The mission type as the world state prints it, e.g. `MT_DEFENSE`.
    pub kind: String,
    /// One of these is picked for a mission.
    pub specs: Vec<String>,
}

/// A roster of enemies a mission spawns from.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Spec {
    pub path: String,
    pub enemy: Vec<Enemy>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Enemy {
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_ru: Option<String>,
    /// Weight against the rest of the roster.
    pub probability: f64,
    /// How far into the mission the enemy starts to turn up, from 0.
    pub tier: i64,
    /// Most of them alive at once; 0 for no cap.
    pub most: i64,
}

/// Every tileset the game lists, and every spec they draw from, enemies named as the client
/// names them.
pub fn read(lineage: &Lineage, en: &Phrases, ru: &Phrases) -> Spawns {
    let mut spawns = Spawns::default();
    let mut wanted = BTreeSet::new();
    let mut paths: Vec<&str> = lineage
        .paths()
        .filter(|path| path.starts_with(TILESETS) && lineage.sets(path, "MissionPermutations"))
        .collect();
    paths.sort();
    for path in paths {
        let Some(own) = lineage.own(path) else {
            continue;
        };
        let Some(permutations) = own.block("MissionPermutations") else {
            continue;
        };
        let mission: Vec<Mission> = permutations
            .fields
            .iter()
            .filter_map(|(kind, value)| {
                let specs: Vec<String> = value
                    .block()?
                    .block("enemySpecs")?
                    .items
                    .iter()
                    .filter_map(Value::text)
                    .map(|spec| resolve(path, spec))
                    .collect();
                wanted.extend(specs.iter().cloned());
                Some(Mission {
                    kind: kind.clone(),
                    specs,
                })
            })
            .collect();
        spawns.tileset.push(Tileset {
            path: path.to_string(),
            mission,
        });
    }
    for path in wanted {
        if let Some(spec) = spec(lineage, &path, en, ru) {
            spawns.spec.push(spec);
        }
    }
    spawns
}

fn spec(lineage: &Lineage, path: &str, en: &Phrases, ru: &Phrases) -> Option<Spec> {
    let (owner, Value::Block(listed)) = lineage.field(path, "Enemies")? else {
        return None;
    };
    let enemy = blocks(&listed)
        .filter_map(|line| {
            let agent = resolve(owner, line.text("agent")?);
            let tag = name_tag(lineage, &agent);
            Some(Enemy {
                name: tag.as_ref().and_then(|tag| en.get(tag)).cloned(),
                name_ru: tag.as_ref().and_then(|tag| ru.get(tag)).cloned(),
                agent,
                probability: number(line, "probability"),
                tier: line.int("tier").unwrap_or(0),
                most: line.int("maxSimultaneous").unwrap_or(0),
            })
        })
        .collect();
    Some(Spec {
        path: path.to_string(),
        enemy,
    })
}

/// The localisation tag of the name an agent shows: its standard avatar's.
fn name_tag(lineage: &Lineage, agent: &str) -> Option<String> {
    let (owner, Value::Block(avatars)) = lineage.field(agent, "AvatarTypes")? else {
        return None;
    };
    let avatar = resolve(owner, avatars.text("STANDARD")?);
    match lineage.field(&avatar, "LocTag")? {
        (_, Value::Text(tag)) if !tag.is_empty() => Some(tag),
        _ => None,
    }
}

fn number(block: &Block, key: &str) -> f64 {
    block
        .text(key)
        .and_then(|text| text.parse::<f64>().ok())
        .map(|value| (value * 1e6).round() / 1e6)
        .unwrap_or(0.0)
}

fn blocks(listed: &Block) -> impl Iterator<Item = &Block> {
    listed.items.iter().filter_map(Value::block)
}

/// How often each enemy turns up on one mission type of one tileset: its weight against its
/// roster, averaged over the rosters the mission picks from, with the earliest tier it joins.
pub fn shares(spawns: &Spawns, tileset: &str, kind: &str) -> BTreeMap<String, (f64, i64)> {
    let Some(mission) = spawns
        .tileset
        .iter()
        .find(|t| t.path == tileset)
        .and_then(|t| t.mission.iter().find(|m| m.kind == kind))
    else {
        return BTreeMap::new();
    };
    let rosters: Vec<&Spec> = mission
        .specs
        .iter()
        .filter_map(|path| spawns.spec.iter().find(|spec| spec.path == *path))
        .collect();
    let mut out: BTreeMap<String, (f64, i64)> = BTreeMap::new();
    for roster in &rosters {
        let total: f64 = roster.enemy.iter().map(|e| e.probability).sum();
        if total <= 0.0 {
            continue;
        }
        for enemy in &roster.enemy {
            let Some(name) = &enemy.name else {
                continue;
            };
            let share = enemy.probability / total / rosters.len() as f64;
            let entry = out.entry(name.clone()).or_insert((0.0, enemy.tier));
            entry.0 += share;
            entry.1 = entry.1.min(enemy.tier);
        }
    }
    for (share, _) in out.values_mut() {
        *share = (*share * 1e4).round() / 1e4;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineage::tests::kind;

    const TILESET: &str = "Name=GrineerAsteroidTileset
MissionPermutations={
MT_DEFENSE={
procLevel=/Lotus/Levels/Proc/Grineer/GrineerAsteroidDefense
enemySpecs={
/Lotus/Types/Game/EnemySpecs/GrineerDefenseA
}
}
}
";

    const SPEC: &str = "Enemies={
{
agent=/Lotus/Types/Enemies/Grineer/AIWeek/RifleLancer
probability=1
maxSimultaneous=0
tier=0
},
{
agent=/Lotus/Types/Enemies/Grineer/AIWeek/CatMaster
probability=0.30000001
maxSimultaneous=1
tier=1
}
}
";

    fn lineage() -> Lineage {
        Lineage::new(vec![
            kind(
                "/Lotus/Types/Game/RegionTilesets/GrineerAsteroidTileset",
                None,
                Some(TILESET),
            ),
            kind(
                "/Lotus/Types/Game/EnemySpecs/GrineerDefenseA",
                None,
                Some(SPEC),
            ),
            kind(
                "/Lotus/Types/Enemies/Grineer/GrineerMarine",
                None,
                Some("AvatarTypes={\nSTANDARD=AIWeek/Avatars/RifleLancerAvatar\n}\n"),
            ),
            kind(
                "/Lotus/Types/Enemies/Grineer/AIWeek/RifleLancer",
                Some("/Lotus/Types/Enemies/Grineer/GrineerMarine"),
                Some("MaxLevel=15\n"),
            ),
            kind(
                "/Lotus/Types/Enemies/Grineer/AIWeek/Avatars/RifleLancerAvatar",
                None,
                Some("LocTag=/Lotus/Language/Game/Lancer\n"),
            ),
        ])
    }

    #[test]
    fn an_enemy_is_named_by_the_avatar_its_agent_inherits() {
        let en = Phrases::from([("/Lotus/Language/Game/Lancer".into(), "Lancer".into())]);

        let spawns = read(&lineage(), &en, &Phrases::new());

        assert_eq!(spawns.tileset[0].mission[0].kind, "MT_DEFENSE");
        let roster = &spawns.spec[0].enemy;
        assert_eq!(roster[0].name.as_deref(), Some("Lancer"));
        assert_eq!(roster[1].name, None);
        assert_eq!(
            (roster[1].probability, roster[1].tier, roster[1].most),
            (0.3, 1, 1)
        );
    }

    #[test]
    fn a_share_is_a_weight_against_the_roster() {
        let en = Phrases::from([("/Lotus/Language/Game/Lancer".into(), "Lancer".into())]);
        let spawns = read(&lineage(), &en, &Phrases::new());

        let shares = shares(
            &spawns,
            "/Lotus/Types/Game/RegionTilesets/GrineerAsteroidTileset",
            "MT_DEFENSE",
        );

        assert_eq!(shares["Lancer"], (0.7692, 0));
    }
}
