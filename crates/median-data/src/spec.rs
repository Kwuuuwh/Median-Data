/// Vault source names.
pub const DE: &str = "de";
pub const WFM: &str = "wfm";
pub const DROPS: &str = "drops";
pub const WIKI: &str = "wiki";
pub const ICONS: &str = "icons";
pub const WFM_ICONS: &str = "wfm-icons";
pub const PORTRAITS: &str = "wiki-portraits";

/// Logical name of the drop-table blob in its snapshot.
pub const DROP_TABLES: &str = "droptables";

/// Languages pinned from the DE export.
pub use graph::LANGS;

/// Logical name of the WFM items blob in its snapshot.
pub const WFM_ITEMS: &str = "items";
/// Logical name of the WFM riven weapon list in its snapshot.
pub const WFM_RIVEN_WEAPONS: &str = "riven-weapons";
/// Logical name of the WFM riven attribute list in its snapshot.
pub const WFM_RIVEN_ATTRIBUTES: &str = "riven-attributes";

/// DE manifests that carry catalog items.
pub const ITEM_MANIFESTS: &[&str] = &[
    "ExportResources",
    "ExportWeapons",
    "ExportWarframes",
    "ExportSentinels",
    "ExportUpgrades",
    "ExportRelicArcane",
    "ExportGear",
    "ExportCustoms",
    "ExportKeys",
    "ExportDrones",
    "ExportFlavour",
];

/// DE manifest carrying foundry recipes.
pub const RECIPES: &str = "ExportRecipes";

/// DE manifest carrying the star chart. Pinned per language: DE translates planet names.
pub const REGIONS: &str = "ExportRegions";

/// Logical names of the wiki modules in its snapshot.
pub const WIKI_MISSIONS: &str = "missions";
pub const WIKI_DROPS: &str = "droptables";
pub const WIKI_BARO: &str = "baro";
pub const WIKI_RESEARCH: &str = "research";
pub const WIKI_VENDORS: &str = "vendors";
pub const WIKI_VOID: &str = "void";
pub const WIKI_BLUEPRINTS: &str = "blueprints";
/// Logical name of the wiki's riven article in its snapshot.
pub const WIKI_RIVEN_MODS: &str = "riven-mods";
/// Logical names of the wiki weapon modules, in the order `sources::wiki::WEAPONS` lists them.
pub const WIKI_WEAPONS: [&str; 6] = [
    "weapons-primary",
    "weapons-secondary",
    "weapons-melee",
    "weapons-archwing",
    "weapons-companion",
    "weapons-modular",
];

/// DE manifest carrying weapon stats and riven dispositions.
pub const WEAPONS: &str = "ExportWeapons";

/// DE manifest carrying mods, riven stat pools among them.
pub const UPGRADES: &str = "ExportUpgrades";

/// DE manifest carrying relics and their rewards.
pub const RELICS: &str = "ExportRelicArcane";

/// DE manifest mapping items to their icon textures.
pub const TEXTURES: &str = "ExportManifest";

/// Logical name of a manifest's Russian variant in a snapshot.
pub fn ru(manifest: &str) -> String {
    format!("{manifest}.ru")
}
