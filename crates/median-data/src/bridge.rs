use std::collections::BTreeMap;

use consensus::Source;
use sources::wfm::WfmItem;

use crate::paths::Paths;

/// A market trade set with the member parts sharing its slug base.
pub struct SetInfo<'a> {
    pub item: &'a WfmItem,
    pub members: Vec<&'a WfmItem>,
}

/// How a listing was tied to a catalog path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// A person said so.
    Curated,
    /// The market's own `gameRef`, which the catalog holds.
    Reference,
    /// One catalog item, and only one, answers to the listing's name.
    Name,
}

impl How {
    pub fn source(self) -> Source {
        match self {
            How::Curated => Source::Curated,
            How::Reference => Source::Wfm,
            How::Name => Source::Rule,
        }
    }
}

/// WFM items indexed for lookup by DE path, plus the trade sets.
pub struct Bridge<'a> {
    by_ref: BTreeMap<String, &'a WfmItem>,
    matched: BTreeMap<String, (String, How)>,
    sets: Vec<SetInfo<'a>>,
}

impl<'a> Bridge<'a> {
    /// Tie every listing to the catalog item it prices. A curated decision wins; otherwise
    /// the market's own reference is used when the catalog holds it, and failing that the
    /// listing's name, but only where exactly one catalog item answers to it.
    pub fn new(items: &'a [WfmItem], paths: &Paths, curated: &BTreeMap<&str, &str>) -> Self {
        let mut by_ref = BTreeMap::new();
        let mut matched = BTreeMap::new();

        // A set or an imprint is a market item distinct from the DE entity its gameRef names,
        // so it never overwrites that entity; each gets its own node instead.
        for w in items
            .iter()
            .filter(|w| !w.has_tag("set") && !w.has_tag("imprint"))
        {
            let Some((path, how)) = resolve(w, paths, curated) else {
                continue;
            };
            matched.insert(w.slug.clone(), (path.clone(), how));
            by_ref.entry(path).or_insert(w);
        }

        let mut sets = Vec::new();
        for item in items.iter().filter(|w| w.has_tag("set")) {
            let Some(base) = item.slug.strip_suffix("_set") else {
                continue;
            };
            let prefix = format!("{base}_");
            let members = items
                .iter()
                .filter(|m| !m.has_tag("set") && m.slug.starts_with(&prefix))
                .collect();
            sets.push(SetInfo { item, members });
        }
        Self {
            by_ref,
            matched,
            sets,
        }
    }

    /// The WFM item bridged to a DE path.
    pub fn get(&self, unique_name: &str) -> Option<&'a WfmItem> {
        self.by_ref.get(unique_name).copied()
    }

    /// Every listing that found its item, and how.
    pub fn matched(&self) -> &BTreeMap<String, (String, How)> {
        &self.matched
    }

    /// How many listings each kind of match accounts for.
    pub fn tally(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for (_, how) in self.matched.values() {
            *counts.entry(how.source().as_str()).or_default() += 1;
        }
        counts
    }

    pub fn sets(&self) -> &[SetInfo<'a>] {
        &self.sets
    }
}

fn resolve(w: &WfmItem, paths: &Paths, curated: &BTreeMap<&str, &str>) -> Option<(String, How)> {
    if let Some(path) = curated.get(w.slug.as_str()) {
        return Some(((*path).to_string(), How::Curated));
    }
    if let Some(game_ref) = w.game_ref.as_deref() {
        if paths.has(game_ref) {
            return Some((game_ref.to_string(), How::Reference));
        }
    }
    let name = w.en_name.as_deref()?;
    // A veiled riven or a key the market names `X (Veiled)` / `X (Key)` is DE's `X`; try the
    // name without its trailing parenthetical when the full name matches nothing.
    let matched = paths.one(name).or_else(|| paths.one(base_name(name)));
    matched.map(|path| (path.to_string(), How::Name))
}

/// A market name with its trailing ` (…)` disambiguator removed.
fn base_name(name: &str) -> &str {
    match name.rfind(" (") {
        Some(cut) if name.ends_with(')') => &name[..cut],
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::DeItem;

    fn item(slug: &str, game_ref: &str, name: &str, tags: &[&str]) -> WfmItem {
        WfmItem {
            slug: slug.into(),
            game_ref: (!game_ref.is_empty()).then(|| game_ref.into()),
            en_name: (!name.is_empty()).then(|| name.into()),
            ru_name: None,
            ducats: None,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            icons: BTreeMap::new(),
        }
    }

    fn de(unique_name: &str, name: &str) -> DeItem {
        DeItem {
            unique_name: unique_name.into(),
            name: name.into(),
            category: "Test".into(),
            array: "Test".into(),
            parent: None,
            mod_type: None,
        }
    }

    #[test]
    fn bridges_by_game_ref_and_groups_set_members() {
        let catalog = [
            de("/Lotus/Powersuits/Volt/VoltPrime", "Volt Prime"),
            de("/R/VoltPrimeBlueprint", "Volt Prime Blueprint"),
            de(
                "/R/VoltPrimeChassisBlueprint",
                "Volt Prime Chassis Blueprint",
            ),
            de("/R/BratonPrimeBarrel", "Braton Prime Barrel"),
        ];
        let paths = Paths::build(&catalog, &[]);
        let items = vec![
            item(
                "volt_prime_set",
                "/Lotus/Powersuits/Volt/VoltPrime",
                "Volt Prime Set",
                &["set", "prime"],
            ),
            item(
                "volt_prime_blueprint",
                "/R/VoltPrimeBlueprint",
                "Volt Prime Blueprint",
                &["blueprint"],
            ),
            item(
                "volt_prime_chassis_blueprint",
                "/R/VoltPrimeChassisBlueprint",
                "Volt Prime Chassis Blueprint",
                &["component"],
            ),
            item(
                "braton_prime_barrel",
                "/R/BratonPrimeBarrel",
                "Braton Prime Barrel",
                &["component"],
            ),
        ];
        let bridge = Bridge::new(&items, &paths, &BTreeMap::new());

        assert_eq!(
            bridge.get("/R/VoltPrimeBlueprint").unwrap().slug,
            "volt_prime_blueprint"
        );
        // the set's own gameRef points at the assembled warframe, which is not tradable
        assert!(bridge.get("/Lotus/Powersuits/Volt/VoltPrime").is_none());

        assert_eq!(bridge.sets().len(), 1);
        let slugs: Vec<&str> = bridge.sets()[0]
            .members
            .iter()
            .map(|m| m.slug.as_str())
            .collect();
        assert!(slugs.contains(&"volt_prime_blueprint"));
        assert!(slugs.contains(&"volt_prime_chassis_blueprint"));
        assert!(!slugs.contains(&"braton_prime_barrel"));
    }

    #[test]
    fn a_stale_reference_falls_back_to_the_name() {
        let catalog = [de("/Mods/QuickThinking", "Quick Thinking")];
        let paths = Paths::build(&catalog, &[]);
        let items = vec![item(
            "quick_thinking",
            "/Moved/Away",
            "Quick Thinking",
            &["mod"],
        )];
        let bridge = Bridge::new(&items, &paths, &BTreeMap::new());
        assert_eq!(
            bridge.get("/Mods/QuickThinking").unwrap().slug,
            "quick_thinking"
        );
        assert_eq!(bridge.matched()["quick_thinking"].1, How::Name);
    }

    #[test]
    fn a_shared_name_stays_unmatched() {
        let catalog = [de("/A/Sunder", "Sunder"), de("/B/Sunder", "Sunder")];
        let paths = Paths::build(&catalog, &[]);
        let items = vec![item("sunder", "", "Sunder", &["mod"])];
        let bridge = Bridge::new(&items, &paths, &BTreeMap::new());
        assert!(bridge.matched().is_empty());
    }

    #[test]
    fn a_veiled_listing_matches_the_base_de_entity() {
        let catalog = [de("/Mods/CompanionRiven", "Companion Weapon Riven Mod")];
        let paths = Paths::build(&catalog, &[]);
        let items = vec![item(
            "companion_weapon_riven_mod_(veiled)",
            "",
            "Companion Weapon Riven Mod (Veiled)",
            &["mod", "riven"],
        )];
        let bridge = Bridge::new(&items, &paths, &BTreeMap::new());
        assert_eq!(
            bridge.get("/Mods/CompanionRiven").unwrap().slug,
            "companion_weapon_riven_mod_(veiled)"
        );
    }

    #[test]
    fn a_curated_decision_outranks_the_market() {
        let catalog = [de("/A/Real", "Real"), de("/B/Other", "Other")];
        let paths = Paths::build(&catalog, &[]);
        let items = vec![item("thing", "/B/Other", "Other", &["mod"])];
        let curated = BTreeMap::from([("thing", "/A/Real")]);
        let bridge = Bridge::new(&items, &paths, &curated);
        assert_eq!(bridge.matched()["thing"].0, "/A/Real");
        assert_eq!(bridge.matched()["thing"].1, How::Curated);
    }
}
