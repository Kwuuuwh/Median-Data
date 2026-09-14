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

        let bases: Vec<&str> = items
            .iter()
            .filter(|w| w.has_tag("set"))
            .filter_map(|w| w.slug.strip_suffix("_set"))
            .collect();

        let mut sets = Vec::new();
        for item in items.iter().filter(|w| w.has_tag("set")) {
            let Some(base) = item.slug.strip_suffix("_set") else {
                continue;
            };
            let members = items
                .iter()
                .filter(|m| !m.has_tag("set") && owner(&bases, &m.slug) == Some(base))
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

/// The set a listing belongs to: the **longest** base its slug carries, and nothing when it
/// carries none.
///
/// A prime weapon shares the start of its plain namesake's slug — `perigale_prime_barrel`
/// begins with both `perigale_` and `perigale_prime_` — so a set that took every slug
/// starting with its own base would take its prime's parts as well, and the part would end up
/// in two sets at once. Only the longest base names a set, because the market builds the
/// slug by appending to the item's name and the longer base is the more specific item.
fn owner<'a>(bases: &[&'a str], slug: &str) -> Option<&'a str> {
    bases
        .iter()
        .filter(|base| {
            slug.len() > base.len()
                && slug.starts_with(**base)
                && slug.as_bytes()[base.len()] == b'_'
        })
        .max_by_key(|base| base.len())
        .copied()
}

fn resolve(w: &WfmItem, paths: &Paths, curated: &BTreeMap<&str, &str>) -> Option<(String, How)> {
    if let Some(path) = curated.get(w.slug.as_str()) {
        return Some(((*path).to_string(), How::Curated));
    }
    if let Some(game_ref) = w.game_ref.as_deref() {
        if let Some(blueprint) = traded_blueprint(w, game_ref, paths) {
            return Some((blueprint, How::Reference));
        }
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

/// The blueprint a `…_blueprint` listing really trades, where its reference points at the
/// component that blueprint builds. The game trades the blueprint, never what it builds, and
/// the market keeps the distinction in the slug while its `gameRef` names the component.
fn traded_blueprint(w: &WfmItem, game_ref: &str, paths: &Paths) -> Option<String> {
    if !w.slug.ends_with("_blueprint") {
        return None;
    }
    let blueprint = format!("{}Blueprint", game_ref.strip_suffix("Component")?);
    paths.has(&blueprint).then_some(blueprint)
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
            vaulted: None,
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
            mastery_req: None,
            max_level_cap: None,
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

    /// Measured on the real market 30.08.2026: `perigale_set` held seven members, four of
    /// them the prime's, and every prime part sat in two sets at once. Eight weapon families
    /// and the damaged Necramech were in the same state.
    #[test]
    fn a_prime_part_belongs_to_the_primes_set_and_not_to_its_namesakes() {
        let catalog = [
            de("/R/PerigaleBarrel", "Perigale Barrel"),
            de("/R/PerigalePrimeBarrel", "Perigale Prime Barrel"),
        ];
        let paths = Paths::build(&catalog, &[]);
        let items = vec![
            item("perigale_set", "", "Perigale Set", &["set"]),
            item(
                "perigale_prime_set",
                "",
                "Perigale Prime Set",
                &["set", "prime"],
            ),
            item(
                "perigale_barrel",
                "/R/PerigaleBarrel",
                "Perigale Barrel",
                &["component"],
            ),
            item(
                "perigale_prime_barrel",
                "/R/PerigalePrimeBarrel",
                "Perigale Prime Barrel",
                &["component"],
            ),
        ];
        let bridge = Bridge::new(&items, &paths, &BTreeMap::new());

        let members = |slug: &str| -> Vec<String> {
            bridge
                .sets()
                .iter()
                .find(|set| set.item.slug == slug)
                .expect("the set")
                .members
                .iter()
                .map(|m| m.slug.clone())
                .collect()
        };

        assert_eq!(members("perigale_set"), ["perigale_barrel"]);
        assert_eq!(members("perigale_prime_set"), ["perigale_prime_barrel"]);
    }

    /// A listing whose slug no set base claims belongs to no set at all.
    #[test]
    fn a_listing_no_set_claims_stands_alone() {
        assert_eq!(owner(&["perigale", "perigale_prime"], "cedo_barrel"), None);
        assert_eq!(owner(&[], "perigale_barrel"), None);
        // a base is not its own member, and a longer word is not a longer base
        assert_eq!(owner(&["perigale"], "perigale"), None);
        assert_eq!(owner(&["mag"], "magnus_prime_barrel"), None);
    }

    #[test]
    fn a_blueprint_listing_lands_on_the_blueprint_not_on_what_it_builds() {
        let catalog = [
            de(
                "/R/Archwing/SupportWingsBlueprint",
                "Amesha Wings Blueprint",
            ),
            de("/R/Archwing/SupportWingsComponent", "Amesha Wings"),
        ];
        let paths = Paths::build(&catalog, &[]);
        let items = vec![item(
            "amesha_wings_blueprint",
            "/R/Archwing/SupportWingsComponent",
            "Amesha Wings",
            &["archwing", "component"],
        )];
        let bridge = Bridge::new(&items, &paths, &BTreeMap::new());
        assert_eq!(
            bridge.matched()["amesha_wings_blueprint"].0,
            "/R/Archwing/SupportWingsBlueprint"
        );
        assert!(bridge.get("/R/Archwing/SupportWingsComponent").is_none());
    }

    #[test]
    fn a_component_listing_keeps_the_component_the_market_names() {
        let catalog = [de("/R/Weapons/BratonPrimeBarrel", "Braton Prime Barrel")];
        let paths = Paths::build(&catalog, &[]);
        let items = vec![item(
            "braton_prime_barrel",
            "/R/Weapons/BratonPrimeBarrel",
            "Braton Prime Barrel",
            &["component"],
        )];
        let bridge = Bridge::new(&items, &paths, &BTreeMap::new());
        assert_eq!(
            bridge.matched()["braton_prime_barrel"].0,
            "/R/Weapons/BratonPrimeBarrel"
        );
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
