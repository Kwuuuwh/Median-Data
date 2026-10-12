use std::collections::BTreeMap;

use graph::{Graph, Node, Phrase};
use serde::{Deserialize, Serialize};

/// Parts of the client's string table the catalog reads: stat lines and riven name parts,
/// riven challenges, and the riven cycling dialog.
const KEPT: [&str; 4] = [
    "/Lotus/Language/Upgrades/",
    "/Lotus/Language/Omega/",
    "/Lotus/Language/Challenges/",
    "/Lotus/Language/Menu/REROLL_",
];

/// Lines of the client's interface median reads back out of its log or off its screen.
const SHOWN: [&str; 5] = [
    "/Lotus/Language/Menu/REROLL_Confirm",
    "/Lotus/Language/Omega/OmegaRerollSelectionConfirm",
    "/Lotus/Language/Omega/OmegaIdentifiedMod",
    "/Lotus/Language/Omega/GenericOmegaMod",
    "/Lotus/Language/Omega/OmegaUnidentified",
];

/// A slice of the client's string table, by key.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Phrases {
    pub phrase: BTreeMap<String, Spoken>,
}

/// One line of the client in both languages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spoken {
    pub en: String,
    /// None where the Russian table lacks the key.
    pub ru: Option<String>,
}

/// Keep the parts of the string tables the catalog reads.
pub fn read(en: &BTreeMap<String, String>, ru: &BTreeMap<String, String>) -> Phrases {
    let phrase = en
        .iter()
        .filter(|(key, _)| KEPT.iter().any(|kept| key.starts_with(kept)))
        .map(|(key, english)| {
            (
                key.clone(),
                Spoken {
                    en: english.clone(),
                    ru: ru.get(key).cloned(),
                },
            )
        })
        .collect();
    Phrases { phrase }
}

/// Put the interface lines median reads into the graph. Returns the keys the client lacks
/// or prints in English only.
pub fn link(graph: &mut Graph, phrases: &Phrases) -> Vec<String> {
    let mut missing = Vec::new();
    for key in SHOWN {
        match phrases.phrase.get(key) {
            Some(spoken) => {
                if spoken.ru.is_none() {
                    missing.push(key.to_string());
                }
                graph.insert(Node::Phrase(Phrase {
                    key: key.to_string(),
                    en: spoken.en.clone(),
                    ru: spoken.ru.clone(),
                }));
            }
            None => missing.push(key.to_string()),
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, text)| (key.to_string(), text.to_string()))
            .collect()
    }

    #[test]
    fn keeps_riven_lines_in_both_languages() {
        let en = table(&[
            (
                "/Lotus/Language/Upgrades/WeaponCritDamage",
                "|val|% Critical Damage",
            ),
            (
                "/Lotus/Language/Menu/REROLL_Confirm",
                "Are you sure you want to cycle |MOD|?",
            ),
            ("/Lotus/Language/Menu/Quit", "Quit"),
        ]);
        let ru = table(&[(
            "/Lotus/Language/Upgrades/WeaponCritDamage",
            "|val|% крит. урона",
        )]);
        let kept = read(&en, &ru);
        assert_eq!(kept.phrase.len(), 2);
        assert_eq!(
            kept.phrase["/Lotus/Language/Upgrades/WeaponCritDamage"]
                .ru
                .as_deref(),
            Some("|val|% крит. урона")
        );
        assert_eq!(kept.phrase["/Lotus/Language/Menu/REROLL_Confirm"].ru, None);
    }
}
