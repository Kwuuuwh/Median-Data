use std::collections::BTreeMap;

use anyhow::{Context, Result};
use sources::lua::{self, Table};

/// The root the missions module assigns its data to.
const ROOT: &str = "MissionData";

/// What the wiki says about the star chart.
pub struct Chart {
    /// DE's `missionIndex` and every name the wiki gives it in `MissionTypes` — it files more
    /// than one type under some indices (`Defense` and `Stage Defense` share 8).
    pub mission_names: BTreeMap<i64, Vec<String>>,
    /// One record per node of `MissionDetails`, keyed by DE's own node key.
    pub nodes: Vec<Node>,
    /// Records the wiki writes with an empty `InternalName`. A node with no key cannot be
    /// joined to anything, so it is left out and named here instead.
    pub keyless: Vec<String>,
}

/// A star-chart node as the wiki describes it.
pub struct Node {
    /// `InternalName`: `SolNode94`, `CrewBattleNode502`.
    pub key: String,
    pub name: String,
    pub planet: String,
    /// Mission type by name; the wiki has names for types DE gives no index (`Skirmish`).
    pub mission: Option<String>,
    /// The faction that holds the node, which the wiki calls the enemy.
    pub faction: Option<String>,
    pub min_level: i64,
    pub max_level: i64,
    pub railjack: bool,
    /// Not shown on the star chart: onslaught, free flight, event-only nodes.
    pub hidden: bool,
    /// Keys into the drop-table module: the node's own reward table, its caches, and the
    /// extra table the drop tables print under a `… Extra` heading.
    pub alias: Option<String>,
    pub cache_alias: Option<String>,
    pub extra_alias: Option<String>,
}

/// One reward row of a drop table: what drops, what kind of thing it is, and how often.
#[derive(Debug, Clone)]
pub struct Row {
    pub name: String,
    /// The wiki's own label: `Resource`, `Blueprint`, `Relic`, `Mod`.
    pub kind: String,
    /// Per cent, as the wiki prints it.
    pub chance: f64,
    pub rotation: Option<String>,
}

/// Mission reward tables from `Module:DropTables/data`, keyed by the alias the star chart
/// module refers to them by.
pub fn tables(raw: &[u8]) -> Result<BTreeMap<String, Vec<Row>>> {
    let src = String::from_utf8_lossy(raw);
    let root = lua::table_of(&src, "DropData").context("read Module:DropTables/data")?;
    let mut out = BTreeMap::new();
    let Some(missions) = root.table("Missions") else {
        return Ok(out);
    };
    for (alias, entry) in &missions.fields {
        let Some(table) = entry.table() else { continue };
        let Some(rewards) = table.table("Rewards") else {
            continue;
        };
        let mut rows = Vec::new();
        for (rotation, list) in &rewards.fields {
            let Some(list) = list.table() else { continue };
            rows.extend(read_rows(list, Some(rotation.as_str())));
        }
        rows.extend(read_rows(rewards, None));
        out.insert(alias.clone(), rows);
    }
    Ok(out)
}

/// A vendor and their stock, as the wiki records it.
pub struct Store {
    pub name: String,
    /// The wiki page the entry belongs to, and with it who the vendor actually is: one person
    /// can keep several counters, each filed under its own name.
    pub link: Option<String>,
    /// What they charge in.
    pub currency: Option<String>,
    /// How the wiki files them: `Store`, `Syndicate`, an event.
    pub kind: Option<String>,
    pub offers: Vec<StoreOffer>,
}

/// One line of a vendor's stock.
pub struct StoreOffer {
    pub name: String,
    /// The wiki's own label for what the thing is.
    pub kind: String,
    pub cost: i64,
    pub count: i64,
    /// Standing rank required.
    pub rank: Option<i64>,
    /// Seconds the offer stays up, where it rotates.
    pub timer: Option<i64>,
}

/// Every vendor of `Module:Vendors/data`: the syndicates, the hub NPCs, the event stores and
/// the in-game market. Baro is not among them — he has his own module with a visit history.
pub fn vendors(raw: &[u8]) -> Result<Vec<Store>> {
    let src = String::from_utf8_lossy(raw);
    let root = lua::returned(&src).context("read Module:Vendors/data")?;
    let mut out = Vec::new();
    let Some(vendors) = root.table("Vendors") else {
        return Ok(out);
    };

    for (key, entry) in &vendors.fields {
        let Some(t) = entry.table() else { continue };
        let offers = t
            .table("Offerings")
            .map(|list| {
                list.items
                    .iter()
                    .filter_map(|row| {
                        let row = row.table()?;
                        Some(StoreOffer {
                            name: row.items.first()?.str()?.to_string(),
                            kind: row
                                .items
                                .get(1)
                                .and_then(|v| v.str())
                                .unwrap_or_default()
                                .to_string(),
                            cost: row.items.get(2).and_then(|v| v.num()).unwrap_or(0.0) as i64,
                            count: row.items.get(3).and_then(|v| v.num()).unwrap_or(1.0) as i64,
                            rank: row.int("Prereq"),
                            timer: row.int("Timer"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        out.push(Store {
            name: t.str("Name").unwrap_or(key).to_string(),
            link: text(t, "Link"),
            currency: text(t, "Currency"),
            kind: text(t, "Type"),
            offers,
        });
    }
    Ok(out)
}

/// A dojo lab and everything researched in it.
pub struct Dojo {
    pub labs: Vec<Lab>,
    pub research: Vec<Researched>,
}

/// One research room.
pub struct Lab {
    pub key: String,
    pub name: String,
    pub faction: String,
}

/// One thing a clan can research, by the name the wiki prints for it.
pub struct Researched {
    pub name: String,
    /// Key into the lab list.
    pub lab: String,
    pub credits: i64,
    pub time: i64,
    pub affinity: i64,
    pub prereq: Option<String>,
    pub resources: Vec<(String, i64)>,
}

/// Read the dojo labs and their research from `Module:Research/data`.
pub fn dojo(raw: &[u8]) -> Result<Dojo> {
    let src = String::from_utf8_lossy(raw);
    let root = lua::table_of(&src, "Data").context("read Module:Research/data")?;

    let mut labs = Vec::new();
    if let Some(rooms) = root.table("Labs") {
        for (key, entry) in &rooms.fields {
            let Some(t) = entry.table() else { continue };
            labs.push(Lab {
                key: key.clone(),
                name: t.str("Name").unwrap_or(key).to_string(),
                faction: t.str("Faction").unwrap_or_default().to_string(),
            });
        }
    }

    let mut research = Vec::new();
    if let Some(entries) = root.table("Research") {
        for (name, entry) in &entries.fields {
            let Some(t) = entry.table() else { continue };
            let Some(lab) = t.str("Lab") else { continue };
            research.push(Researched {
                name: name.clone(),
                lab: lab.to_string(),
                credits: t.int("Credits").unwrap_or(0),
                time: t.int("Time").unwrap_or(0),
                affinity: t.int("Affinity").unwrap_or(0),
                prereq: text(t, "Prereq"),
                resources: t
                    .table("Resources")
                    .map(|list| {
                        list.items
                            .iter()
                            .filter_map(|row| {
                                let row = row.table()?;
                                Some((row.str("Name")?.to_string(), row.int("Count").unwrap_or(1)))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
    }

    Ok(Dojo { labs, research })
}

/// One thing a vendor has offered, as the wiki records it.
#[derive(Debug, Clone)]
pub struct Offered {
    pub name: String,
    pub ducats: Option<i64>,
    pub credits: Option<i64>,
    /// Visits it has appeared on. The wiki keeps the dates; the count is what survives as a
    /// catalog fact, because which visit is current is live world state.
    pub times: usize,
    pub always: bool,
    pub gone: bool,
}

/// Everything Baro Ki'Teer has ever brought, from `Module:Baro/data`. The module returns its
/// table directly instead of naming it.
pub fn baro(raw: &[u8]) -> Result<Vec<Offered>> {
    let src = String::from_utf8_lossy(raw);
    let root = lua::returned(&src).context("read Module:Baro/data")?;
    let mut out = Vec::new();
    for group in ["Items", "ExtraItems"] {
        let Some(items) = root.table(group) else {
            continue;
        };
        for (key, entry) in &items.fields {
            let Some(t) = entry.table() else { continue };
            out.push(Offered {
                name: t.str("Name").unwrap_or(key).to_string(),
                ducats: t.int("DucatCost"),
                credits: t.int("CreditCost"),
                times: dates(t),
                always: t.bool("IsAlways").unwrap_or(false),
                gone: t.bool("IsDiscont").unwrap_or(false),
            });
        }
    }
    Ok(out)
}

/// How many visits a thing was offered on, over every platform's list.
fn dates(t: &Table) -> usize {
    ["OfferingDates", "PcOfferingDates", "TennoConOfferingDates"]
        .iter()
        .filter_map(|key| t.table(key))
        .map(|list| list.items.len())
        .max()
        .unwrap_or(0)
}

/// Rows are positional: name, kind, chance, and sometimes a stack size.
fn read_rows(list: &Table, rotation: Option<&str>) -> Vec<Row> {
    list.items
        .iter()
        .filter_map(|item| {
            let row = item.table()?;
            let name = row.items.first()?.str()?;
            Some(Row {
                name: name.to_string(),
                kind: row.items.get(1).and_then(|v| v.str()).unwrap_or_default().to_string(),
                chance: row.items.get(2).and_then(|v| v.num()).unwrap_or(0.0),
                rotation: rotation.map(str::to_string),
            })
        })
        .collect()
}

/// Read the star chart out of `Module:Missions/data`.
pub fn chart(raw: &[u8]) -> Result<Chart> {
    let src = String::from_utf8_lossy(raw);
    let root = lua::table_of(&src, ROOT).context("read Module:Missions/data")?;

    let mut mission_names = BTreeMap::new();
    if let Some(types) = root.table("MissionTypes") {
        for entry in types.fields.values() {
            let Some(t) = entry.table() else { continue };
            if let (Some(index), Some(name)) = (t.int("Index"), t.str("Name")) {
                let names: &mut Vec<String> = mission_names.entry(index).or_default();
                if !names.iter().any(|n| n == name) {
                    names.push(name.to_string());
                }
            }
        }
    }

    let mut nodes = Vec::new();
    let mut keyless = Vec::new();
    if let Some(details) = root.table("MissionDetails") {
        for item in &details.items {
            let Some(t) = item.table() else { continue };
            let (Some(key), Some(name)) = (t.str("InternalName"), t.str("Name")) else {
                continue;
            };
            if key.is_empty() {
                keyless.push(name.to_string());
                continue;
            }
            nodes.push(Node {
                key: key.to_string(),
                name: name.to_string(),
                planet: t.str("Planet").unwrap_or_default().to_string(),
                mission: text(t, "Type"),
                faction: text(t, "Enemy"),
                min_level: t.int("MinLevel").unwrap_or(0),
                max_level: t.int("MaxLevel").unwrap_or(0),
                railjack: t.bool("IsRailjack").unwrap_or(false),
                hidden: t.bool("IsHidden").unwrap_or(false),
                alias: text(t, "DropTableAlias"),
                cache_alias: text(t, "CacheDropTableAlias"),
                extra_alias: text(t, "ExtraDropTableAlias"),
            });
        }
    }

    Ok(Chart {
        mission_names,
        nodes,
        keyless,
    })
}

/// A field that is present but empty says nothing.
fn text(t: &Table, key: &str) -> Option<String> {
    t.str(key).filter(|v| !v.is_empty()).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped exactly like the real module, ragged whitespace and all: the node list sits
    /// under a key whose line starts with a space before the tab.
    const SRC: &str = "local MissionData = {\n\
        \t[\"MissionTypes\"] = {\n\
        \t\tSurvival = { Name = \"Survival\", Index = 2, IsEndless = true },\n\
        \t\t[\"Conclave\"] = { Name = \"Conclave\", Index = 12 },\n\
        \t},\n\
        \t[\"MissionModifiers\"] = { [\"Void Storm\"] = { Name = \"Void Storm\" } },\n\
        \x20\t[\"MissionDetails\"] = {\n\
        \t\t{ Name = \"Apollodorus\", Planet = \"Mercury\", Type = \"Survival\", Tileset = \"\", \
             Enemy = \"Infested\", MinLevel = 6, MaxLevel = 11, InternalName = \"SolNode94\", \
             PreviousNodes = { \"Boethius\" }, IsTracked = true },\n\
        \t\t{ Name = \"Sover Strait\", Planet = \"Earth Proxima\", Type = \"Skirmish\", \
             Enemy = \"Grineer\", MinLevel = 15, MaxLevel = 20, \
             DropTableAlias = \"EarthProximaSkirmish\", InternalName = \"CrewBattleNode502\", \
             IsRailjack = true, NextNodes = { \"Iota Temple\" } },\n\
        \t\t{ Name = \"Elite Sanctuary Onslaught\", Planet = \"Sanctuary Onslaught\", \
             Type = \"Sanctuary Onslaught\", Tileset = \"\", Enemy = \"\", \
             InternalName = \"SolNode802\", IsHidden = true },\n\
        \t},\n\
        }\n\
        for k in pairs(MissionData.by) do table.insert(MissionData.vars, k) end\n\
        return MissionData\n";

    #[test]
    fn reads_mission_indices() {
        let c = chart(SRC.as_bytes()).unwrap();
        assert_eq!(c.mission_names[&2], vec!["Survival".to_string()]);
        assert_eq!(c.mission_names[&12], vec!["Conclave".to_string()]);
    }

    #[test]
    fn keeps_every_name_an_index_is_given() {
        let src = "local MissionData = { [\"MissionTypes\"] = {\n\
            Defense = { Name = \"Defense\", Index = 8 },\n\
            [\"Stage Defense\"] = { Name = \"Stage Defense\", Index = 8 },\n\
            } }";
        let c = chart(src.as_bytes()).unwrap();
        assert_eq!(c.mission_names[&8].len(), 2);
        assert!(c.mission_names[&8].contains(&"Defense".to_string()));
    }

    #[test]
    fn reads_nodes_including_railjack() {
        let c = chart(SRC.as_bytes()).unwrap();
        assert_eq!(c.nodes.len(), 3);
        let sol = &c.nodes[0];
        assert_eq!(sol.key, "SolNode94");
        assert_eq!(sol.planet, "Mercury");
        assert_eq!(sol.mission.as_deref(), Some("Survival"));
        assert_eq!(sol.faction.as_deref(), Some("Infested"));
        assert_eq!((sol.min_level, sol.max_level), (6, 11));
        assert!(!sol.railjack && !sol.hidden);

        let rail = &c.nodes[1];
        assert_eq!(rail.key, "CrewBattleNode502");
        assert_eq!(rail.planet, "Earth Proxima");
        assert!(rail.railjack);
    }

    #[test]
    fn an_empty_field_says_nothing_and_hidden_nodes_are_marked() {
        let c = chart(SRC.as_bytes()).unwrap();
        let onslaught = &c.nodes[2];
        assert_eq!(onslaught.faction, None);
        assert!(onslaught.hidden);
    }
}
