use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sources::cache::Archive;
use sources::{language, notation};

use crate::offers::{self, Stall};

/// Languages median reads, and the package each one's string table sits in.
const STRING_TABLES: [(&str, &str); 2] = [("en", "H.Misc_en"), ("ru", "H.Misc_ru")];

/// The string table of one language.
const STRINGS: &str = "/Languages.bin";

/// Longest a phrase can be and still be something's name.
const NAME_LEN: usize = 48;

/// Most words a name runs to.
const NAME_WORDS: usize = 4;

/// Longest a word may be and still read as a joiner inside a name, as in `Sister of Parvos`.
const JOINER_LEN: usize = 3;

/// Package the vendor manifests sit in.
const PACKAGE: &str = "H.Misc";

/// Directories of the cache that hold what vendors sell.
const STALLS: [&str; 2] = ["/Lotus/Types/Game/VendorManifests/", "/Lotus/Syndicates/"];

const NAMES: &str = "names.toml";
const OFFERS: &str = "offers.toml";

/// Every stall of the game, under one key so the file reads as a list.
#[derive(Debug, Serialize)]
struct Stalls {
    vendor: Vec<Stall>,
}

/// What the client calls things: the Russian for an English name, and the English the client
/// gives more than one Russian for, which nothing may translate on its own.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
struct Names {
    name: BTreeMap<String, String>,
    ambiguous: BTreeMap<String, Vec<String>>,
}

/// The Russian the client shows for a name it prints in English.
pub fn spoken(dir: &Path) -> Result<BTreeMap<String, String>> {
    let path = dir.join(NAMES);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {} — run `extract` to write it", path.display()))?;
    let names: Names =
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(names.name)
}

/// Distil the game cache into the files a build reads.
pub fn run(cache: &Path, out_dir: &Path) -> Result<()> {
    if !cache.is_dir() {
        bail!("no game cache at {}", cache.display());
    }
    let mut tables = BTreeMap::new();
    for (lang, package) in STRING_TABLES {
        let archive = Archive::open(&cache.join(format!("{package}.toc")))?;
        let entry = archive
            .entries()
            .iter()
            .find(|entry| entry.path == STRINGS)
            .with_context(|| format!("{package} holds no {STRINGS}"))?
            .clone();
        let phrases = language::strings(&archive.read(&entry)?)
            .with_context(|| format!("read the {lang} strings"))?;
        eprintln!("cache    {lang} strings — {} keys", phrases.len());
        tables.insert(lang, phrases);
    }

    let names = names(&tables["en"], &tables["ru"]);
    eprintln!(
        "cache    {} names, {} the client translates two ways",
        names.name.len(),
        names.ambiguous.len()
    );
    write(
        &out_dir.join(NAMES),
        "# Written by `median-data extract` from the game cache: the Russian the client\n\
         # shows for a name it prints in English.\n\n",
        &names,
    )?;

    let stalls = stalls(&Archive::open(&cache.join(format!("{PACKAGE}.toc")))?)?;
    eprintln!(
        "cache    {} stalls, {} offers",
        stalls.vendor.len(),
        stalls
            .vendor
            .iter()
            .map(|stall| stall.offer.len())
            .sum::<usize>()
    );
    write(
        &out_dir.join(OFFERS),
        "# Written by `median-data extract` from the game cache: what each vendor and\n\
         # syndicate manifest sells, in the game's own paths.\n\n",
        &stalls,
    )
}

/// Every stall the cache describes, in the order its table of contents lists them.
fn stalls(archive: &Archive) -> Result<Stalls> {
    let mut vendor = Vec::new();
    for entry in archive.entries() {
        if !STALLS.iter().any(|dir| entry.path.starts_with(dir)) {
            continue;
        }
        let raw = archive.read(entry)?;
        // Binary files share these directories; they are not manifests.
        let Ok(written) = notation::parse(&raw) else {
            eprintln!("cache    skipped {} — not a manifest", entry.path);
            continue;
        };
        let stall = offers::stall(&entry.path, &written);
        if !stall.offer.is_empty() {
            vendor.push(stall);
        }
    }
    vendor.sort_by(|one, other| one.manifest.cmp(&other.manifest));
    Ok(Stalls { vendor })
}

fn names(en: &BTreeMap<String, String>, ru: &BTreeMap<String, String>) -> Names {
    let mut said: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (key, english) in en {
        let Some(russian) = ru.get(key) else { continue };
        if russian == english || !is_name(english) {
            continue;
        }
        let spoken = said.entry(english).or_default();
        if !spoken.contains(&russian.as_str()) {
            spoken.push(russian);
        }
    }

    let mut names = Names::default();
    for (english, russian) in said {
        match russian.as_slice() {
            [only] => {
                names.name.insert(english.to_string(), only.to_string());
            }
            many => {
                let mut many: Vec<String> = many.iter().map(|text| text.to_string()).collect();
                many.sort();
                names.ambiguous.insert(english.to_string(), many);
            }
        }
    }
    names
}

/// Whether a phrase reads as something's name rather than a line of interface or dialogue.
fn is_name(phrase: &str) -> bool {
    if phrase.is_empty() || phrase.chars().count() > NAME_LEN {
        return false;
    }
    if phrase.contains(['|', '<', '>', '\\', '/', '\n']) {
        return false;
    }
    if phrase.ends_with(['.', '!', '?', ':', ',']) {
        return false;
    }
    let mut words = phrase.split_whitespace();
    let Some(first) = words.next() else {
        return false;
    };
    if !capitalised(first) {
        return false;
    }
    let rest: Vec<&str> = words.collect();
    rest.len() < NAME_WORDS
        && rest
            .iter()
            .all(|word| capitalised(word) || word.chars().count() <= JOINER_LEN)
}

/// Whether a word opens the way a name's words do.
fn capitalised(word: &str) -> bool {
    word.chars()
        .next()
        .is_some_and(|first| !first.is_alphabetic() || first.is_uppercase())
}

fn write<T: Serialize>(path: &Path, header: &str, body: &T) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = format!("{header}{}", toml::to_string_pretty(body)?);
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))
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
    fn a_name_carries_its_russian() {
        let en = table(&[("/Lotus/Language/Enemies/TeralystName", "Eidolon Teralyst")]);
        let ru = table(&[("/Lotus/Language/Enemies/TeralystName", "Эйдолон Тералист")]);

        let names = names(&en, &ru);

        assert_eq!(names.name["Eidolon Teralyst"], "Эйдолон Тералист");
        assert!(names.ambiguous.is_empty());
    }

    #[test]
    fn two_russians_for_one_english_translate_neither() {
        let en = table(&[
            ("/Lotus/Language/Menu/RELOAD", "Reload"),
            ("/Lotus/Language/Game/RELOAD_HINT", "Reload"),
        ]);
        let ru = table(&[
            ("/Lotus/Language/Menu/RELOAD", "Перезарядка"),
            ("/Lotus/Language/Game/RELOAD_HINT", "Перезарядить"),
        ]);

        let names = names(&en, &ru);

        assert!(names.name.is_empty());
        assert_eq!(names.ambiguous["Reload"], ["Перезарядить", "Перезарядка"]);
    }

    #[test]
    fn a_phrase_the_client_leaves_in_english_is_not_carried() {
        let en = table(&[("/Lotus/Language/Locations/Adaro", "Adaro")]);
        let ru = table(&[("/Lotus/Language/Locations/Adaro", "Adaro")]);

        assert!(names(&en, &ru).name.is_empty());
    }

    #[test]
    fn interface_lines_are_not_names() {
        assert!(is_name("Eidolon Teralyst"));
        assert!(is_name("002-Er"));
        assert!(is_name("Sister of Parvos"));
        assert!(!is_name("|PLAYER| respawned"));
        assert!(!is_name(
            "An ancient brute possessed of a weighty arm-flail"
        ));
        assert!(!is_name("Complete Chapter 1."));
        assert!(!is_name("Crit / 100 Damage"));
        assert!(!is_name(""));
    }
}
