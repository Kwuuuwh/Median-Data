/// Languages the catalog carries. Names have one per language, and so do the pictures of
/// mods, which draw their name and stats into the card.
pub const LANGS: &[&str] = &["en", "ru"];

/// DE keeps alternate copies of a mod under a tier folder. Some are training copies that
/// reuse the real mod's name; others are their own item (`/Expert/` holds the Primed and
/// Galvanized mods), so the folder alone never decides anything.
const TIERS: &[&str] = &["/Beginner/", "/Intermediate/", "/Expert/"];

/// Whether a path sits under a tier folder.
pub fn tiered(path: &str) -> bool {
    TIERS.iter().any(|tier| path.contains(tier))
}

/// Reward chance per relic rarity and refinement.
pub const CHANCES: &[(&str, &str, f64)] = &[
    ("COMMON", "intact", 0.2533),
    ("COMMON", "exceptional", 0.2333),
    ("COMMON", "flawless", 0.20),
    ("COMMON", "radiant", 0.1667),
    ("UNCOMMON", "intact", 0.11),
    ("UNCOMMON", "exceptional", 0.13),
    ("UNCOMMON", "flawless", 0.17),
    ("UNCOMMON", "radiant", 0.20),
    ("RARE", "intact", 0.02),
    ("RARE", "exceptional", 0.04),
    ("RARE", "flawless", 0.06),
    ("RARE", "radiant", 0.10),
];

/// The chance a rarity implies at a given refinement.
pub fn chance(rarity: &str, refinement: &str) -> Option<f64> {
    CHANCES
        .iter()
        .find(|(r, refined, _)| r.eq_ignore_ascii_case(rarity) && *refined == refinement)
        .map(|(_, _, chance)| *chance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rarity_has_four_refinements() {
        for rarity in ["COMMON", "UNCOMMON", "RARE"] {
            assert_eq!(CHANCES.iter().filter(|(r, ..)| *r == rarity).count(), 4);
        }
    }

    #[test]
    fn looks_up_case_insensitively() {
        assert_eq!(chance("common", "intact"), Some(0.2533));
        assert_eq!(chance("MYTHIC", "intact"), None);
    }

    #[test]
    fn tier_folders_are_recognised() {
        assert!(tiered(
            "/Lotus/Upgrades/Mods/Warframe/Beginner/AvatarHealthMaxModBeginner"
        ));
        // Primed mods live under /Expert/ and are real items, so the folder only marks a
        // candidate — the name decides.
        assert!(tiered(
            "/Lotus/Upgrades/Mods/Melee/Expert/WeaponMeleeDamageModExpert"
        ));
        assert!(!tiered("/Lotus/Upgrades/Mods/Warframe/AvatarHealthMaxMod"));
    }
}
