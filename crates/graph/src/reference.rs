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

/// What share of an opened relic each rarity accounts for, per refinement. The shares are the
/// rule; the chance of one reward is its share split between however many rewards of that
/// rarity the relic holds. A relic with the usual three commons gives 76 / 3 = 25.33 % each,
/// and one with eight of them gives 9.5 % — which is what the drop tables print.
pub const SHARES: &[(&str, &str, f64)] = &[
    ("COMMON", "intact", 0.76),
    ("COMMON", "exceptional", 0.70),
    ("COMMON", "flawless", 0.60),
    ("COMMON", "radiant", 0.50),
    ("UNCOMMON", "intact", 0.22),
    ("UNCOMMON", "exceptional", 0.26),
    ("UNCOMMON", "flawless", 0.34),
    ("UNCOMMON", "radiant", 0.40),
    ("RARE", "intact", 0.02),
    ("RARE", "exceptional", 0.04),
    ("RARE", "flawless", 0.06),
    ("RARE", "radiant", 0.10),
];

/// The share a rarity accounts for at a given refinement.
pub fn share(rarity: &str, refinement: &str) -> Option<f64> {
    SHARES
        .iter()
        .find(|(r, refined, _)| r.eq_ignore_ascii_case(rarity) && *refined == refinement)
        .map(|(_, _, share)| *share)
}

/// The chance of one reward, given how many rewards of its rarity the relic holds.
pub fn chance(rarity: &str, refinement: &str, in_rarity: usize) -> Option<f64> {
    match in_rarity {
        0 => None,
        n => share(rarity, refinement).map(|s| s / n as f64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rarity_has_four_refinements() {
        for rarity in ["COMMON", "UNCOMMON", "RARE"] {
            assert_eq!(SHARES.iter().filter(|(r, ..)| *r == rarity).count(), 4);
        }
    }

    #[test]
    fn every_refinement_shares_out_a_whole() {
        for refinement in ["intact", "exceptional", "flawless", "radiant"] {
            let total: f64 = SHARES
                .iter()
                .filter(|(_, r, _)| *r == refinement)
                .map(|(.., s)| s)
                .sum();
            assert!(
                (total - 1.0).abs() < 1e-9,
                "{refinement} shares out {total}"
            );
        }
    }

    #[test]
    fn a_share_splits_between_the_rewards_that_hold_it() {
        assert_eq!(chance("common", "intact", 3), Some(0.76 / 3.0));
        assert_eq!(chance("COMMON", "intact", 8), Some(0.095));
        assert_eq!(chance("common", "intact", 0), None);
        assert_eq!(chance("MYTHIC", "intact", 3), None);
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
