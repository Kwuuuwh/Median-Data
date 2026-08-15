use std::collections::{BTreeMap, BTreeSet};

use consensus::{Claim, Conflict, Resolved, Source, Status, claims, resolve};
use graph::{Extra, Item, Kind, Names, Taxonomy};
use sources::wfm::WfmItem;

use crate::bridge::Bridge;
use crate::curation::Curation;
use crate::extract::DeItem;
use crate::rules;
use crate::taxonomy::{self, Policy};

/// Source order for every property: a person outranks DE, which outranks the market, which
/// outranks a rule the build derived.
const NAMES: &[Source] = &[Source::Curated, Source::De, Source::Wfm];
const DERIVED: &[Source] = &[Source::Curated, Source::Rule, Source::Wfm];

/// What every item is judged against besides its own DE record: which things a recipe builds,
/// which live in the void economy, and the tree that classifies them.
pub struct Facts<'a> {
    pub built: &'a BTreeSet<String>,
    pub economy: &'a BTreeSet<String>,
    pub taxonomy: &'a Policy,
}

/// Build item nodes from DE facts, WFM evidence, curated decisions and rules, collecting
/// conflicts.
pub fn items(
    de: Vec<DeItem>,
    ru: &BTreeMap<String, String>,
    bridge: &Bridge<'_>,
    curated: &Curation,
    facts: &Facts<'_>,
    conflicts: &mut Vec<Conflict>,
) -> Vec<Item> {
    let mut unique: BTreeMap<String, DeItem> = BTreeMap::new();
    for it in de {
        unique.entry(it.unique_name.clone()).or_insert(it);
    }
    let names = curated.names();
    let picks = curated.picks();

    unique
        .into_values()
        .map(|it| {
            let path = it.unique_name.as_str();
            let hand = Hand {
                en: picks.get(&(path, "name_en")).copied(),
                ru: picks
                    .get(&(path, "name_ru"))
                    .copied()
                    .or_else(|| names.get(path).copied()),
                prime: flag(&picks, path, "prime"),
                tradable: flag(&picks, path, "tradable"),
                kind: picks.get(&(path, "kind")).copied(),
            };
            let wfm = bridge.get(&it.unique_name);
            one(&it, ru.get(&it.unique_name), wfm, &hand, facts, conflicts)
        })
        .collect()
}

/// What a person decided about one item. A decision both fixes the value and settles the
/// disagreement: the sources still differ, but nobody needs to look at it again.
struct Hand<'a> {
    en: Option<&'a str>,
    ru: Option<&'a str>,
    prime: Option<bool>,
    tradable: Option<bool>,
    kind: Option<&'a str>,
}

fn one(
    de: &DeItem,
    de_ru: Option<&String>,
    wfm: Option<&WfmItem>,
    hand: &Hand<'_>,
    facts: &Facts<'_>,
    conflicts: &mut Vec<Conflict>,
) -> Item {
    let wfm_en = wfm.and_then(|w| w.en_name.as_deref());
    let en = name(hand.en, Some(de.name.as_str()), wfm_en).expect("DE name present");
    let ru = name(
        hand.ru,
        de_ru.map(String::as_str),
        wfm.and_then(|w| w.ru_name.as_deref()),
    );

    let rule_prime = rules::prime(&de.name, &de.unique_name, facts.economy);
    let wfm_prime = wfm.map(|w| w.has_tag("prime"));
    let prime = flags(hand.prime, Some(rule_prime), wfm_prime).expect("rule claim present");
    let tradable = tradability(&de.unique_name, wfm, hand.tradable, facts.built);

    if unsettled(&en, hand.en.is_some()) {
        conflicts.push(Conflict::new(
            &de.unique_name,
            "name_en",
            claims(&[
                (Source::De, Some(de.name.clone())),
                (Source::Wfm, wfm_en.map(str::to_string)),
            ]),
            en.value.clone(),
        ));
    }
    if let Some(r) = ru.as_ref().filter(|r| unsettled(r, hand.ru.is_some())) {
        conflicts.push(Conflict::new(
            &de.unique_name,
            "name_ru",
            claims(&[
                (Source::De, de_ru.cloned()),
                (Source::Wfm, wfm.and_then(|w| w.ru_name.clone())),
            ]),
            r.value.clone(),
        ));
    }
    if unsettled(&prime, hand.prime.is_some()) {
        conflicts.push(Conflict::new(
            &de.unique_name,
            "prime",
            claims(&[(Source::Rule, Some(rule_prime)), (Source::Wfm, wfm_prime)]),
            prime.value.to_string(),
        ));
    }
    if let Some(t) = tradable
        .as_ref()
        .filter(|t| unsettled(t, hand.tradable.is_some()))
    {
        conflicts.push(Conflict::new(
            &de.unique_name,
            "tradable",
            claims(&[
                (
                    Source::Rule,
                    facts.built.contains(&de.unique_name).then_some(false),
                ),
                (Source::Wfm, wfm.map(|_| true)),
            ]),
            t.value.to_string(),
        ));
    }

    Item {
        unique_name: de.unique_name.clone(),
        names: Names { en, ru },
        category: single(Source::De, de.category.clone()),
        kind: kind(de, hand.kind, facts.taxonomy, conflicts),
        slug: wfm.map(|w| single(Source::Wfm, w.slug.clone())),
        tradable,
        vaulted: None,
        prime,
        ducats: wfm.and_then(|w| w.ducats),
        extra: rules::relic(&de.unique_name).map_or(Extra::None, Extra::Relic),
    }
}

/// Where the item sits in the taxonomy: a hand decision when it names a declared kind,
/// otherwise the rule table's verdict. A decision naming a kind no class declares is
/// recorded as a conflict rather than dropped, so a typo surfaces instead of vanishing.
fn kind(
    de: &DeItem,
    hand: Option<&str>,
    taxonomy: &Policy,
    conflicts: &mut Vec<Conflict>,
) -> Resolved<Kind> {
    let derived = taxonomy.classify(&taxonomy::facts(de));
    settle(&de.unique_name, hand, derived, &taxonomy.tree, conflicts)
}

/// Resolve a curated kind against a derived one.
pub fn settle(
    path: &str,
    hand: Option<&str>,
    derived: Kind,
    tree: &Taxonomy,
    conflicts: &mut Vec<Conflict>,
) -> Resolved<Kind> {
    if let Some(bad) = hand.filter(|k| !tree.has(k)) {
        conflicts.push(Conflict::new(
            path,
            "kind",
            claims(&[
                (Source::Curated, Some(bad.to_string())),
                (Source::Rule, Some(derived.as_str().to_string())),
            ]),
            derived.as_str().to_string(),
        ));
    }

    let mut all = Vec::new();
    if let Some(picked) = hand.filter(|k| tree.has(k)) {
        all.push(Claim {
            source: Source::Curated,
            value: Kind::new(picked),
        });
    }
    all.push(Claim {
        source: Source::Rule,
        value: derived,
    });
    resolve(&all, DERIVED).expect("rule claim present")
}

/// Presence on the market is evidence of tradability; the built-component rule overrides it,
/// and a person overrides both.
fn tradability(
    unique_name: &str,
    wfm: Option<&WfmItem>,
    hand: Option<bool>,
    built: &BTreeSet<String>,
) -> Option<Resolved<bool>> {
    flags(
        hand,
        built.contains(unique_name).then_some(false),
        wfm.map(|_| true),
    )
}

fn flags(curated: Option<bool>, rule: Option<bool>, wfm: Option<bool>) -> Option<Resolved<bool>> {
    let mut all = Vec::new();
    for (source, value) in [
        (Source::Curated, curated),
        (Source::Rule, rule),
        (Source::Wfm, wfm),
    ] {
        if let Some(value) = value {
            all.push(Claim { source, value });
        }
    }
    resolve(&all, DERIVED)
}

/// The market often spells the same name differently: title case where DE writes sentence
/// case, or DE's name plus a parenthetical the market adds to tell listings apart
/// (`Rifle Riven Mod (Veiled)`, `Nihil's Oubliette (Key)`). Either way it is one name, so the
/// two claims are reconciled to the spelling worth keeping before they are compared: DE's for
/// a pure case change, the market's fuller name when it adds a tag DE omits.
fn name(curated: Option<&str>, de: Option<&str>, wfm: Option<&str>) -> Option<Resolved<String>> {
    let (de, wfm) = reconcile(de, wfm);
    let mut all = Vec::new();
    for (source, value) in [
        (Source::Curated, curated),
        (Source::De, de),
        (Source::Wfm, wfm),
    ] {
        if let Some(value) = value {
            all.push(Claim {
                source,
                value: value.to_string(),
            });
        }
    }
    resolve(&all, NAMES)
}

/// When the market only decorates DE's name, rewrite both claims to the spelling we keep so
/// the resolver reads agreement rather than a conflict.
fn reconcile<'a>(de: Option<&'a str>, wfm: Option<&'a str>) -> (Option<&'a str>, Option<&'a str>) {
    match (de, wfm) {
        (Some(d), Some(w)) if d.to_lowercase() == w.to_lowercase() => (Some(d), Some(d)),
        (Some(d), Some(w)) if parenthetical(d, w) => (Some(w), Some(w)),
        _ => (de, wfm),
    }
}

/// Whether a property still needs a human. Sources that disagree stay recorded as a
/// conflict in the provenance, but once a person has judged it there is nothing left to do.
fn unsettled<T>(value: &Resolved<T>, decided: bool) -> bool {
    value.status == Status::Conflict && !decided
}

/// Whether the market name is DE's plus a trailing parenthetical tag, ignoring case (the
/// market also title-cases what DE writes in sentence case).
fn parenthetical(de: &str, wfm: &str) -> bool {
    let (de, wfm) = (de.to_lowercase(), wfm.to_lowercase());
    match wfm.strip_prefix(&de) {
        Some(rest) => {
            let rest = rest.trim();
            rest.starts_with('(') && rest.ends_with(')')
        }
        None => false,
    }
}

/// A curated boolean, where a person wrote one.
fn flag(picks: &BTreeMap<(&str, &str), &str>, item: &str, prop: &str) -> Option<bool> {
    match *picks.get(&(item, prop))? {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn single<T: Clone + PartialEq>(source: Source, value: T) -> Resolved<T> {
    resolve(&[Claim { source, value }], &[source]).expect("one claim resolves")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_case_difference_is_agreement_on_des_spelling() {
        let r = name(None, Some("Валентный ряд"), Some("Валентный Ряд")).unwrap();
        assert_eq!(r.status, Status::Confirmed);
        assert_eq!(r.value, "Валентный ряд");
    }

    #[test]
    fn a_parenthetical_tag_is_kept_as_the_fuller_name() {
        // the tag (Veiled) is information DE omits, so the market's name wins, no conflict
        let r = name(
            None,
            Some("Rifle Riven Mod"),
            Some("Rifle Riven Mod (Veiled)"),
        )
        .unwrap();
        assert_eq!(r.status, Status::Confirmed);
        assert_eq!(r.value, "Rifle Riven Mod (Veiled)");
    }

    #[test]
    fn a_parenthetical_tag_is_kept_through_a_case_change_too() {
        let r = name(
            None,
            Some("Мод Разлома для Винтовок"),
            Some("Мод Разлома Для Винтовок (Закрытый)"),
        )
        .unwrap();
        assert_eq!(r.status, Status::Confirmed);
        assert_eq!(r.value, "Мод Разлома Для Винтовок (Закрытый)");
    }

    #[test]
    fn a_genuinely_reworded_name_still_conflicts() {
        let r = name(
            None,
            Some("Муталист Алад V: Навигационные координаты"),
            Some("Муталист-навигационная координата"),
        )
        .unwrap();
        assert_eq!(r.status, Status::Conflict);
    }

    #[test]
    fn a_trailing_word_without_parentheses_still_conflicts() {
        let r = name(None, Some("Smeeta Kavat"), Some("Smeeta Kavat Imprint")).unwrap();
        assert_eq!(r.status, Status::Conflict);
        assert_eq!(r.value, "Smeeta Kavat");
    }

    #[test]
    fn a_hand_written_name_outranks_every_source() {
        let r = name(Some("Вольт Прайм"), Some("Volt Prime"), Some("Volt Prime")).unwrap();
        assert_eq!(r.value, "Вольт Прайм");
    }
}
