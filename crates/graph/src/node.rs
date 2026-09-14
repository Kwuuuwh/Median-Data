use consensus::{Resolved, Source};
use serde::Deserialize;

use crate::kind::Kind;

/// Display names per language.
#[derive(Debug, Clone, PartialEq)]
pub struct Names {
    pub en: Resolved<String>,
    pub ru: Option<Resolved<String>>,
}

/// A void relic. DE models every refinement as its own entity, all sharing a display name.
#[derive(Debug, Clone, PartialEq)]
pub struct RelicInfo {
    /// The logical relic behind all four refinements.
    pub base: String,
    pub refinement: String,
    /// Game version that put the relic in the vault, where a source names one.
    pub vaulted_in: Option<String>,
}

/// Payload carried only by items of a given kind.
#[derive(Debug, Clone, PartialEq)]
pub enum Extra {
    None,
    Relic(RelicInfo),
}

/// Which of the game's two mastery tables an item's ranks count on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mastery {
    /// A hundred per rank: weapons, amps, zaws, kitguns.
    Wielded,
    /// Two hundred per rank: warframes, companions, archwings, K-drives.
    Carried,
}

impl Mastery {
    pub fn as_str(self) -> &'static str {
        match self {
            Mastery::Wielded => "wielded",
            Mastery::Carried => "carried",
        }
    }
}

/// A catalog item.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub unique_name: String,
    pub names: Names,
    /// What DE files the item as: its `productCategory`, or the manifest when it gives none.
    pub category: Resolved<String>,
    /// Where the item sits in our own taxonomy.
    pub kind: Resolved<Kind>,
    pub slug: Option<Resolved<String>>,
    pub tradable: Option<Resolved<bool>>,
    /// Whether the item is in the vault — out of the game's drop tables, obtainable only by
    /// trade. Unset where nothing says either way.
    pub vaulted: Option<Resolved<bool>>,
    pub prime: Resolved<bool>,
    pub ducats: Option<i64>,
    /// Which mastery table the item's ranks count on; none when it gives no mastery.
    pub mastery: Option<Resolved<Mastery>>,
    /// Mastery rank the game asks before the item is built or traded.
    pub mastery_req: Option<i64>,
    /// Highest rank the item levels to, where it gives mastery.
    pub max_level_cap: Option<Resolved<i64>>,
    pub extra: Extra,
}

/// A foundry recipe, keyed by the blueprint that starts it.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub blueprint: String,
    pub build_price: Option<i64>,
    pub build_time: Option<i64>,
    /// The blueprint is spent when built.
    pub consumed: bool,
    pub rush_price: Option<i64>,
}

/// A warframe.market trade set.
#[derive(Debug, Clone, PartialEq)]
pub struct Set {
    pub slug: String,
    pub names: Names,
    pub ducats: Option<i64>,
    /// Whether every part of the set is in the vault.
    pub vaulted: Option<Resolved<bool>>,
}

/// A warframe.market imprint: a tradable breeding token for a pet. DE has no such entity —
/// only the animal — so the imprint carries the market's own name and slug and points at
/// the animal it yields.
#[derive(Debug, Clone, PartialEq)]
pub struct Imprint {
    pub slug: String,
    pub names: Names,
    /// `unique_name` of the animal this imprint breeds.
    pub animal: String,
}

/// What kind of place an item drops in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceKind {
    Node,
    Key,
    Sortie,
    Bounty,
    Transient,
}

impl PlaceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PlaceKind::Node => "node",
            PlaceKind::Key => "key",
            PlaceKind::Sortie => "sortie",
            PlaceKind::Bounty => "bounty",
            PlaceKind::Transient => "transient",
        }
    }
}

/// A bounty's settlement and who hands it out. Which bounty is on offer rotates, so this says
/// where to go and at what level, never what is available now.
#[derive(Debug, Clone, PartialEq)]
pub struct Bounty {
    pub settlement: String,
    pub settlement_ru: Option<String>,
    pub giver: Option<String>,
    pub giver_ru: Option<String>,
    pub min_level: i64,
    pub max_level: i64,
    /// What the drop tables print after the level range.
    pub activity: String,
    pub activity_ru: Option<String>,
}

/// Somewhere items drop: a mission node, a key, a sortie, a bounty. The drop tables print
/// their names in English only, so the Russian side can only be written by hand.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// The heading the drop tables print, kept verbatim: it is what a drop row names its
    /// place by, so it stays the key whatever else is read out of it.
    pub name: String,
    pub name_ru: Option<String>,
    pub kind: PlaceKind,
    /// Set for bounty tables, where the printed name carries a level range and a label.
    pub bounty: Option<Bounty>,
    /// Set for a node's reward table, where the heading names a location, a node and which
    /// table of that node it is.
    pub table: Option<Table>,
}

/// What a node's reward table heading says. The node it names is a separate entity, so the
/// location and the mission type here are only what the drop tables printed — the star-chart
/// node behind them carries the same facts as data.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub location: String,
    pub node: String,
    /// Printed beside the node: the mission type, or the table's own name (`Caches`).
    pub label: String,
    /// The node's second table, printed under a trailing `Extra`.
    pub extra: bool,
    /// A past event's table, which the drop tables keep long after its node is gone.
    pub event: bool,
}

/// Someone the player kills for what they carry. The drop tables split an enemy's table by
/// level range; the ranges belong to the tables, so the enemy is one node whatever its level.
#[derive(Debug, Clone, PartialEq)]
pub struct Enemy {
    pub name: String,
    pub name_ru: Option<String>,
}

/// A grouping the star chart files its nodes under. DE calls it the system name, and it is
/// not always a planet: the Proxima regions, the Void, a city, and a mode reached from a relay
/// are all filed the same way.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub name: String,
    pub name_ru: Option<String>,
    /// What the grouping is — `planet`, `moon`, `proxima`, `place`, `mode` — where the
    /// reference table says. Nothing is guessed for one it does not name.
    pub kind: Option<String>,
}

/// What one of DE's numbered enums is called, where the reference table names it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Label {
    pub en: Option<String>,
    pub ru: Option<String>,
    /// Item whose picture the game uses as this label's emblem, where the reference table
    /// names one.
    pub icon: Option<String>,
}

/// A node of the star chart, as DE describes it. Mission type, faction and node type are
/// DE's own numbers; an index the reference table does not name keeps its number and no
/// label, so nothing is guessed.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// DE's key for the node, e.g. `SolNode94`.
    pub node: String,
    pub name: String,
    /// Only where a translation exists: DE ships most node names untranslated.
    pub name_ru: Option<String>,
    /// DE shipped this node's name in Russian and it reads the same as the English one — the
    /// game keeps it as it is, so no translation is owed.
    pub verbatim: bool,
    /// The location the node sits in, by its English name.
    pub location: String,
    pub mission: i64,
    pub mission_label: Label,
    pub faction: i64,
    pub faction_label: Label,
    pub node_type: i64,
    pub type_label: Label,
    /// Mastery rank the node asks before it can be played.
    pub mastery_req: i64,
    /// Mastery the node's first completion gives, and as much again on the Steel Path.
    pub mastery_xp: i64,
    pub min_level: i64,
    pub max_level: i64,
    /// The map the node is played on. DE exports no such field; the wiki names one per node,
    /// in English only.
    pub tileset: Label,
    /// Who says this node exists. DE does not export Railjack at all, so those come from the
    /// wiki.
    pub origin: Source,
    /// Flown in a Railjack rather than walked.
    pub railjack: bool,
    /// Not shown on the star chart: onslaught rooms, free flight, event-only nodes.
    pub hidden: bool,
}

/// Someone who hands items over for currency. Whether the stock is fixed matters: Baro
/// brings a different set every visit, so an edge to him means the item has been offered at
/// some point, not that it is on sale now. What is on sale today is live world state and has
/// no place in a pinned catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct Vendor {
    pub key: String,
    pub name: String,
    pub name_ru: Option<String>,
    /// What they charge in: standing, ducats, platinum, or one of the game's many tokens.
    pub currency: Option<String>,
    /// How the source files them: a store, a syndicate, an event that has ended.
    pub kind: Option<String>,
    /// The whole stock changes between visits, as Baro's does.
    pub rotates: bool,
}

/// A research room of a clan dojo. What is researched there unlocks a blueprint.
#[derive(Debug, Clone, PartialEq)]
pub struct Lab {
    pub key: String,
    pub name: String,
    pub name_ru: Option<String>,
    pub faction: String,
}

/// A typed graph node.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Item(Item),
    Recipe(Recipe),
    Set(Set),
    Imprint(Imprint),
    Place(Place),
    Enemy(Enemy),
    Location(Location),
    Region(Region),
    Vendor(Vendor),
    Lab(Lab),
}

/// Node id of a recipe, by its blueprint path.
pub fn recipe_id(blueprint: &str) -> String {
    format!("recipe:{blueprint}")
}

/// Node id of a set, by its market slug.
pub fn set_id(slug: &str) -> String {
    format!("set:{slug}")
}

/// Node id of an imprint, by its market slug.
pub fn imprint_id(slug: &str) -> String {
    format!("imprint:{slug}")
}

/// Node id of a place, by its printed name.
pub fn place_id(name: &str) -> String {
    format!("place:{name}")
}

/// Node id of an enemy, by its printed name.
pub fn enemy_id(name: &str) -> String {
    format!("enemy:{name}")
}

/// Node id of a star-chart node, by DE's key for it.
pub fn region_id(node: &str) -> String {
    format!("region:{node}")
}

/// Node id of a location, by its English name.
pub fn location_id(name: &str) -> String {
    format!("location:{name}")
}

/// Node id of a vendor, by our own key for them.
pub fn vendor_id(key: &str) -> String {
    format!("vendor:{key}")
}

/// Node id of a dojo lab, by the wiki's key for it.
pub fn lab_id(key: &str) -> String {
    format!("lab:{key}")
}

impl Node {
    /// The node's stable key.
    pub fn id(&self) -> String {
        match self {
            Node::Item(i) => i.unique_name.clone(),
            Node::Recipe(r) => recipe_id(&r.blueprint),
            Node::Set(s) => set_id(&s.slug),
            Node::Imprint(i) => imprint_id(&i.slug),
            Node::Place(p) => place_id(&p.name),
            Node::Enemy(e) => enemy_id(&e.name),
            Node::Location(l) => location_id(&l.name),
            Node::Region(r) => region_id(&r.node),
            Node::Vendor(v) => vendor_id(&v.key),
            Node::Lab(l) => lab_id(&l.key),
        }
    }

    /// The node's English display name.
    pub fn label(&self) -> &str {
        match self {
            Node::Item(i) => &i.names.en.value,
            Node::Recipe(r) => &r.blueprint,
            Node::Set(s) => &s.names.en.value,
            Node::Imprint(i) => &i.names.en.value,
            Node::Place(p) => &p.name,
            Node::Enemy(e) => &e.name,
            Node::Location(l) => &l.name,
            Node::Region(r) => &r.name,
            Node::Vendor(v) => &v.name,
            Node::Lab(l) => &l.name,
        }
    }

    /// The node's Russian display name, where anything carries one.
    pub fn label_ru(&self) -> Option<&str> {
        match self {
            Node::Item(i) => i.names.ru.as_ref().map(|r| r.value.as_str()),
            Node::Recipe(_) => None,
            Node::Set(s) => s.names.ru.as_ref().map(|r| r.value.as_str()),
            Node::Imprint(i) => i.names.ru.as_ref().map(|r| r.value.as_str()),
            Node::Place(p) => p.name_ru.as_deref(),
            Node::Enemy(e) => e.name_ru.as_deref(),
            Node::Location(l) => l.name_ru.as_deref(),
            Node::Region(r) => r.name_ru.as_deref(),
            Node::Vendor(v) => v.name_ru.as_deref(),
            Node::Lab(l) => l.name_ru.as_deref(),
        }
    }
}
