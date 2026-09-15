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

/// DE manifest carrying relics and their rewards.
pub const RELICS: &str = "ExportRelicArcane";

/// DE manifest mapping items to their icon textures.
pub const TEXTURES: &str = "ExportManifest";

/// Logical name of a manifest's Russian variant in a snapshot.
pub fn ru(manifest: &str) -> String {
    format!("{manifest}.ru")
}
