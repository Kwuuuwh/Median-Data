use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use graph::{
    Complication, Extra, Graph, MarketStat, NamePart, Node, RivenChallenge, RivenClass, RivenPool,
    RivenStat, Shown, StatForm,
};
use serde::Deserialize;
use sources::wfm::RivenAttribute;

use crate::extract::{DeRivenPool, DeRivenStat};
use crate::phrases::Phrases;

/// Where the client keeps the lines it prints stats with.
const STAT_LINES: &str = "/Lotus/Language/Upgrades/";

/// Where the client keeps the wording of challenges.
const CHALLENGES: &str = "/Lotus/Language/Challenges/Challenge_";

/// Where the client keeps riven name parts.
const NAME_PARTS: &str = "/Lotus/Language/Omega/";

/// Melee stats DE keys apart and the market files under its ranged attribute.
const MARKET_TAGS: [(&str, &str); 4] = [
    ("WeaponMeleeDamageMod", "WeaponDamageAmountMod"),
    (
        "WeaponMeleeFactionDamageCorpus",
        "WeaponFactionDamageCorpus",
    ),
    (
        "WeaponMeleeFactionDamageGrineer",
        "WeaponFactionDamageGrineer",
    ),
    (
        "WeaponMeleeFactionDamageInfested",
        "WeaponFactionDamageInfested",
    ),
];

/// Which client line a riven stat is shown with, where several lines print its English
/// differently in Russian. Written by hand from what the riven screen shows.
#[derive(Debug, Default, Deserialize)]
pub struct Lines {
    /// Stat tag -> the client's key for the line.
    #[serde(default)]
    pub line: BTreeMap<String, String>,
}

/// Read the chosen lines. A missing file means none are chosen yet.
pub fn load(path: &Path) -> Result<Lines> {
    if !path.exists() {
        return Ok(Lines::default());
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// What the client and the market say about the riven stats, besides DE.
pub struct Witnesses<'a> {
    pub phrases: &'a Phrases,
    pub lines: &'a Lines,
    pub market: &'a [RivenAttribute],
}

/// What putting the riven pools into the graph came to.
#[derive(Default)]
pub struct Pooled {
    pub pools: usize,
    /// What a pool holds that could not be read or named, by pool.
    pub unread: Vec<(String, String)>,
}

/// Hang each riven pool on the riven item it belongs to.
pub fn link(graph: &mut Graph, pools: Vec<DeRivenPool>, witnesses: &Witnesses<'_>) -> Pooled {
    let mut out = Pooled::default();
    for pool in pools {
        let path = pool.unique_name.clone();
        let mut note = |text: String| out.unread.push((path.clone(), text));
        let Some(class) = class_of(&pool.unique_name) else {
            note("a riven pool of no class this build knows".to_string());
            continue;
        };
        let stats = pool
            .stats
            .into_iter()
            .map(|stat| self::stat(stat, witnesses, &mut note))
            .collect();
        let challenges = pool
            .challenges
            .into_iter()
            .map(|(path, complications)| {
                challenge(path, complications, witnesses.phrases, &mut note)
            })
            .collect();
        if let Some(Node::Item(item)) = graph.get_mut(&pool.unique_name) {
            item.extra = Extra::RivenPool(RivenPool {
                class,
                stats,
                challenges,
                splices: Vec::new(),
            });
            out.pools += 1;
        }
    }
    out
}

/// The riven class a pool rolls for, by the record's name.
fn class_of(path: &str) -> Option<RivenClass> {
    let name = path.rsplit('/').next()?;
    let class = match name.strip_suffix("RandomModRare")? {
        "LotusRifle" => RivenClass::Rifle,
        "LotusShotgun" => RivenClass::Shotgun,
        "LotusPistol" => RivenClass::Pistol,
        "PlayerMeleeWeapon" => RivenClass::Melee,
        "LotusArchgun" => RivenClass::Archgun,
        "LotusModularPistol" => RivenClass::Kitgun,
        "LotusModularMelee" => RivenClass::Zaw,
        _ => return None,
    };
    Some(class)
}

fn stat(de: DeRivenStat, witnesses: &Witnesses<'_>, note: &mut impl FnMut(String)) -> RivenStat {
    let shown_en = de.shown.unwrap_or_default();
    let shown_ru = russian_line(&de.tag, &shown_en, witnesses, note);
    let market_tag = MARKET_TAGS
        .iter()
        .find(|(melee, _)| *melee == de.tag)
        .map_or(de.tag.as_str(), |(_, ranged)| ranged);
    let market = witnesses.market.iter().find(|a| a.game_ref == market_tag);
    if market.is_none() {
        note(format!("{}: the market names no attribute for it", de.tag));
    }
    RivenStat {
        prefix: name_part(&de.tag, &de.prefix, "Prefix", witnesses.phrases, note),
        suffix: name_part(&de.tag, &de.suffix, "Suffix", witnesses.phrases, note),
        form: form(&de.tag, &shown_en),
        market: market.map(|a| MarketStat {
            slug: a.slug.clone(),
            positive_only: a.positive_only,
            negative_only: a.negative_only,
            positive_is_negative: a.positive_is_negative,
            exclusive_to: a.exclusive_to.clone(),
        }),
        bases: de.bases,
        reverse_symbol: de.reverse_symbol,
        tag: de.tag,
        shown_en,
        shown_ru,
    }
}

/// The Russian the client prints a stat with. DE gives only the English, so the line is found
/// by the English the client prints for it; where several lines print that English with
/// different Russian, a hand choice decides.
fn russian_line(
    tag: &str,
    english: &str,
    witnesses: &Witnesses<'_>,
    note: &mut impl FnMut(String),
) -> Option<String> {
    let candidates: Vec<(&String, &Option<String>)> = witnesses
        .phrases
        .phrase
        .iter()
        .filter(|(key, spoken)| key.starts_with(STAT_LINES) && spoken.en == english)
        .map(|(key, spoken)| (key, &spoken.ru))
        .collect();
    let mut russian: Vec<&Option<String>> = candidates.iter().map(|(_, ru)| *ru).collect();
    russian.sort();
    russian.dedup();
    match (russian.as_slice(), witnesses.lines.line.get(tag)) {
        ([], _) => {
            note(format!("{tag}: no client line prints '{english}'"));
            None
        }
        ([only], _) => {
            if only.is_none() {
                note(format!(
                    "{tag}: the client prints '{english}' in English only"
                ));
            }
            (*only).clone()
        }
        (_, Some(chosen)) => match candidates.iter().find(|(key, _)| *key == chosen) {
            Some((_, ru)) => (*ru).clone(),
            None => {
                note(format!(
                    "{tag}: the chosen line {chosen} does not print '{english}'"
                ));
                None
            }
        },
        (_, None) => {
            let listed: Vec<String> = candidates
                .iter()
                .map(|(key, ru)| format!("{key} «{}»", ru.as_deref().unwrap_or("—")))
                .collect();
            note(format!(
                "{tag}: the client prints '{english}' several ways, none chosen — {}",
                listed.join(", ")
            ));
            None
        }
    }
}

/// A name part in both languages: the client keeps it under `<Stat>Prefix` or `<Stat>Suffix`
/// with the English DE prints.
fn name_part(
    tag: &str,
    english: &str,
    side: &str,
    phrases: &Phrases,
    note: &mut impl FnMut(String),
) -> NamePart {
    if english.is_empty() {
        return NamePart {
            en: String::new(),
            ru: Some(String::new()),
        };
    }
    let found = phrases.phrase.iter().find(|(key, spoken)| {
        key.starts_with(NAME_PARTS)
            && key.ends_with(side)
            && spoken.en.eq_ignore_ascii_case(english)
    });
    let ru = found.and_then(|(_, spoken)| spoken.ru.clone());
    if ru.is_none() {
        note(format!("{tag}: no Russian for the name part '{english}'"));
    }
    NamePart {
        en: english.to_string(),
        ru,
    }
}

/// A challenge in the client's words. The client keys them by the last part of DE's path:
/// `Challenge_<name>_Description`, `Challenge_<name>_Single_Description` for a count of one,
/// `Challenge_Complication_<name>`.
fn challenge(
    path: String,
    complications: Vec<String>,
    phrases: &Phrases,
    note: &mut impl FnMut(String),
) -> RivenChallenge {
    let name = last_part(&path);
    let mut line = |key: String, required: bool| {
        let spoken = phrases.phrase.get(&key);
        match spoken {
            Some(s) if s.ru.is_none() => {
                note(format!("{key}: the client prints it in English only"))
            }
            None if required => note(format!("{key}: the client has no such line")),
            _ => {}
        }
        spoken.map(|s| Shown {
            en: s.en.clone(),
            ru: s.ru.clone(),
        })
    };
    let shown = line(format!("{CHALLENGES}{name}_Description"), true).unwrap_or(Shown {
        en: String::new(),
        ru: None,
    });
    let shown_single = line(format!("{CHALLENGES}{name}_Single_Description"), false);
    let complications = complications
        .into_iter()
        .map(|path| Complication {
            shown: line(
                format!("{CHALLENGES}Complication_{}", last_part(&path)),
                true,
            )
            .unwrap_or(Shown {
                en: String::new(),
                ru: None,
            }),
            path,
        })
        .collect();
    RivenChallenge {
        path,
        shown,
        shown_single,
        complications,
    }
}

fn last_part(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// How the client prints a stat's number.
fn form(tag: &str, english: &str) -> StatForm {
    if tag.contains("FactionDamage") {
        StatForm::Multiplier
    } else if english.contains('%') {
        StatForm::Percent
    } else {
        StatForm::Plain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phrases::Spoken;

    fn phrases(rows: &[(&str, &str, Option<&str>)]) -> Phrases {
        Phrases {
            phrase: rows
                .iter()
                .map(|(key, en, ru)| {
                    (
                        key.to_string(),
                        Spoken {
                            en: en.to_string(),
                            ru: ru.map(str::to_string),
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn a_pool_takes_its_class_from_its_name() {
        assert_eq!(
            class_of("/Lotus/Upgrades/Mods/Randomized/LotusModularPistolRandomModRare"),
            Some(RivenClass::Kitgun)
        );
        assert_eq!(
            class_of("/Lotus/Upgrades/Mods/Randomized/RawRifleRandomMod"),
            None
        );
    }

    #[test]
    fn a_stat_line_is_found_by_its_english_and_chosen_where_it_forks() {
        let table = phrases(&[
            (
                "/Lotus/Language/Upgrades/WeaponCritDamage",
                "|val|% Critical Damage",
                Some("|val|% крит. урона"),
            ),
            (
                "/Lotus/Language/Upgrades/CriticalChance",
                "|val|% Critical Chance",
                Some("|val|% к шансу критического урона"),
            ),
            (
                "/Lotus/Language/Upgrades/WeaponCritChance",
                "|val|% Critical Chance",
                Some("|val|% шанс крит. урона"),
            ),
        ]);
        let mut lines = Lines::default();
        let witnesses = Witnesses {
            phrases: &table,
            lines: &lines,
            market: &[],
        };
        let mut notes = Vec::new();
        let mut note = |text: String| notes.push(text);
        assert_eq!(
            russian_line(
                "WeaponCritDamageMod",
                "|val|% Critical Damage",
                &witnesses,
                &mut note
            )
            .as_deref(),
            Some("|val|% крит. урона")
        );
        assert_eq!(
            russian_line(
                "WeaponCritChanceMod",
                "|val|% Critical Chance",
                &witnesses,
                &mut note
            ),
            None
        );
        assert_eq!(notes.len(), 1);

        lines.line.insert(
            "WeaponCritChanceMod".to_string(),
            "/Lotus/Language/Upgrades/WeaponCritChance".to_string(),
        );
        let witnesses = Witnesses {
            phrases: &table,
            lines: &lines,
            market: &[],
        };
        assert_eq!(
            russian_line(
                "WeaponCritChanceMod",
                "|val|% Critical Chance",
                &witnesses,
                &mut |_| {}
            )
            .as_deref(),
            Some("|val|% шанс крит. урона")
        );
    }

    #[test]
    fn a_challenge_is_worded_by_the_last_part_of_its_path() {
        let table = phrases(&[
            (
                "/Lotus/Language/Challenges/Challenge_RandomizedKill_Description",
                "Kill |COUNT| Enemies",
                Some("Убейте несколько (|COUNT|) врагов."),
            ),
            (
                "/Lotus/Language/Challenges/Challenge_RandomizedKill_Single_Description",
                "Kill an Enemy",
                Some("Убейте врага."),
            ),
            (
                "/Lotus/Language/Challenges/Challenge_Complication_Sliding",
                "while sliding",
                Some("в скольжении"),
            ),
        ]);
        let mut notes = Vec::new();
        let made = challenge(
            "/Lotus/Types/Challenges/RandomizedKill".to_string(),
            vec![
                "/Lotus/Types/Challenges/Complications/Sliding".to_string(),
                "/Lotus/Types/Challenges/Complications/Gone".to_string(),
            ],
            &table,
            &mut |text| notes.push(text),
        );
        assert_eq!(
            made.shown.ru.as_deref(),
            Some("Убейте несколько (|COUNT|) врагов.")
        );
        assert_eq!(
            made.shown_single.map(|s| s.en),
            Some("Kill an Enemy".to_string())
        );
        assert_eq!(made.complications[0].shown.en, "while sliding");
        assert_eq!(
            notes,
            vec![
                "/Lotus/Language/Challenges/Challenge_Complication_Gone: the client has no such line"
            ]
        );
    }

    #[test]
    fn faction_damage_prints_as_a_multiplier() {
        assert_eq!(
            form("WeaponFactionDamageCorpus", "|val| Damage to Corpus"),
            StatForm::Multiplier
        );
        assert_eq!(
            form("WeaponPunctureDepthMod", "|val| Punch Through"),
            StatForm::Plain
        );
        assert_eq!(
            form("WeaponCritDamageMod", "|val|% Critical Damage"),
            StatForm::Percent
        );
    }
}
