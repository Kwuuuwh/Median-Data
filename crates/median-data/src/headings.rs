//! Russian for the headings the drop tables print. A heading is not a name someone gave a
//! place: it is built out of parts — a level range, an activity, a mode in brackets — and
//! every part is already translated, by the client or by hand. So the Russian is composed
//! from the parts rather than written out one heading at a time, and a heading whose parts
//! are not all known stays English and waits for a person.

use graph::Bounty;

use crate::curation::Terms;

/// What a heading ends with once its brackets are off, and the Russian that reads naturally
/// in front of the thing it belongs to. The client shows these words on their own
/// (`REWARDS`, `Убийство`) but never in the phrase the tables glue, so the phrasing is here.
const ROLES: [(&str, &str); 12] = [
    ("Common Rewards", "обычные награды"),
    ("Uncommon Rewards", "необычные награды"),
    ("Rare Rewards", "редкие награды"),
    ("Gold Rewards", "золотые награды"),
    ("Silver Rewards", "серебряные награды"),
    ("Legendary Rewards", "легендарные награды"),
    ("Rewards", "награды"),
    ("Mission Caches", "тайники миссии"),
    ("Endurance Caches", "тайники на выносливость"),
    ("Caches", "тайники"),
    ("Assassination", "убийство"),
    ("Vault", "хранилище"),
];

/// What the tables put in brackets after a heading to say which run it is.
const MODES: [(&str, &str); 3] = [
    ("Steel Path", "Стальной путь"),
    ("Steel Path Winner", "Стальной путь, победитель"),
    ("Winner", "победитель"),
];

/// The Russian for a heading, or nothing when a part of it is still untranslated. A bounty
/// heading is its own shape — the level range belongs in front of nothing and behind the
/// activity — so it is read off the bounty the tables already gave us.
pub fn russian(printed: &str, bounty: Option<&Bounty>, terms: &Terms) -> Option<String> {
    if let Some(bounty) = bounty {
        return of_bounty(bounty);
    }
    let (core, modes) = split_modes(printed.trim());
    let modes: Option<Vec<String>> = modes.iter().map(|m| mode(m, terms)).collect();
    let core = phrase(core, terms)?;
    Some(match modes?.as_slice() {
        [] => core,
        said => format!("{core} ({})", said.join(", ")),
    })
}

/// A bounty reads as its activity and the levels it is offered at.
fn of_bounty(bounty: &Bounty) -> Option<String> {
    let activity = bounty.activity_ru.as_deref()?;
    match (bounty.min_level, bounty.max_level) {
        (0, 0) => Some(activity.to_string()),
        (min, max) => Some(format!("{activity}, ур. {min}–{max}")),
    }
}

/// A heading without its brackets: what the client shows for the whole phrase, or a phrase
/// the tables built — a thing they name after a place, and a word for what the table holds.
fn phrase(core: &str, terms: &Terms) -> Option<String> {
    if let Some(ru) = terms.of("place", core, core) {
        return Some(ru);
    }
    if let Some((stem, role)) = split_role(core)
        && let Some(stem) = phrase(stem.trim_end_matches([':', ' ']), terms)
    {
        // A stem that already reads as two parts takes a dash, so one heading never carries
        // two colons.
        return Some(match stem.contains(':') {
            true => format!("{stem} — {role}"),
            false => format!("{stem}: {role}"),
        });
    }
    let (head, tail) = core.split_once(": ")?;
    let (head, tail) = (phrase(head, terms)?, phrase(tail.trim(), terms)?);
    Some(format!("{head}: {tail}"))
}

/// What one bracket says, in Russian: a mode the tables name, or anything the client names.
fn mode(printed: &str, terms: &Terms) -> Option<String> {
    MODES
        .iter()
        .find(|(en, _)| en.eq_ignore_ascii_case(printed))
        .map(|(_, ru)| (*ru).to_string())
        .or_else(|| terms.of("place", printed, printed))
}

/// A heading split into what it is about and the bracketed runs it was printed for.
fn split_modes(printed: &str) -> (&str, Vec<&str>) {
    let mut core = printed;
    let mut modes = Vec::new();
    while let Some(open) = core.strip_suffix(')').and_then(|c| c.rfind('(')) {
        let bracket = &core[open + 1..core.len() - 1];
        modes.push(bracket.trim());
        core = core[..open].trim_end();
    }
    modes.reverse();
    (core, modes)
}

/// A heading split into the thing it names and the word for what the table holds.
fn split_role(core: &str) -> Option<(&str, &'static str)> {
    ROLES
        .iter()
        .filter_map(|(en, ru)| {
            let stem = core.strip_suffix(en)?.trim_end();
            (!stem.is_empty()).then_some((stem, *ru))
        })
        .max_by_key(|(stem, _)| core.len() - stem.len())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::curation::Curation;

    fn spoken() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("VOID STORM".to_string(), "БУРЯ БЕЗДНЫ".to_string()),
            ("Earth".to_string(), "Земля".to_string()),
            (
                "Deep Archimedea".to_string(),
                "Глубинная Архимедия".to_string(),
            ),
            ("The Descendia".to_string(), "Спуск".to_string()),
            ("Phorid".to_string(), "Форид".to_string()),
        ])
    }

    fn bounty(activity: &str, min: i64, max: i64) -> Bounty {
        Bounty {
            settlement: "Cetus".to_string(),
            settlement_ru: Some("Цетус".to_string()),
            giver: None,
            giver_ru: None,
            min_level: min,
            max_level: max,
            activity: activity.to_string(),
            activity_ru: Some("Заказы Цетуса".to_string()),
        }
    }

    #[test]
    fn a_bounty_reads_as_its_activity_and_levels() {
        let curated = Curation::default();
        let spoken = spoken();
        let terms = curated.terms(&spoken);
        let b = bounty("Cetus Bounty", 5, 15);

        assert_eq!(
            russian("Level 5 - 15 Cetus Bounty", Some(&b), &terms).as_deref(),
            Some("Заказы Цетуса, ур. 5–15")
        );
    }

    #[test]
    fn a_table_printed_for_one_planet_keeps_the_planet() {
        let curated = Curation::default();
        let spoken = spoken();
        let terms = curated.terms(&spoken);

        assert_eq!(
            russian("Void Storm (Earth)", None, &terms).as_deref(),
            Some("Буря Бездны (Земля)")
        );
    }

    #[test]
    fn a_heading_names_what_it_holds() {
        let curated = Curation::default();
        let spoken = spoken();
        let terms = curated.terms(&spoken);

        assert_eq!(
            russian("Deep Archimedea Gold Rewards", None, &terms).as_deref(),
            Some("Глубинная Архимедия: золотые награды")
        );
        assert_eq!(
            russian("The Descendia: Rare Rewards (Steel Path)", None, &terms).as_deref(),
            Some("Спуск: редкие награды (Стальной путь)")
        );
        assert_eq!(
            russian("Phorid Assassination", None, &terms).as_deref(),
            Some("Форид: убийство")
        );
    }

    #[test]
    fn a_heading_with_an_unknown_part_waits_for_a_person() {
        let curated = Curation::default();
        let spoken = spoken();
        let terms = curated.terms(&spoken);

        assert_eq!(russian("Derelict Vault", None, &terms), None);
        assert_eq!(russian("Deep Archimedea (Infernum 6)", None, &terms), None);
    }
}
