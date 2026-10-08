use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sources::cache::Archive;
use sources::{language, notation, packages};

use crate::lineage::Lineage;
use crate::offers::{self, Stall};
use crate::{grants, incubator, mining, spawns};

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

/// Every type of the game, with its parent and own properties.
const TYPES: &str = "/Packages.bin";

/// Directories of the cache that hold what vendors sell.
const STALLS: [&str; 2] = ["/Lotus/Types/Game/VendorManifests/", "/Lotus/Syndicates/"];

const NAMES: &str = "names.toml";
const OFFERS: &str = "offers.toml";
const CLIENT: &str = "client.toml";
const GRANTS: &str = "grants.toml";
const INCUBATOR: &str = "incubator.toml";
const MINING: &str = "mining.toml";
const SPAWNS: &str = "spawns.toml";

/// Which client the distillate was taken from.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Client {
    /// The day the game last wrote to its cache, as `YYYY.MM.DD`.
    pub build: String,
}

/// Every stall of the game, under one key so the file reads as a list.
#[derive(Debug, Serialize, Deserialize)]
struct Stalls {
    vendor: Vec<Stall>,
}

/// What the client calls things: the Russian for an English name, the English the client
/// gives more than one Russian for, and the names it leaves in English everywhere.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Names {
    pub name: BTreeMap<String, String>,
    pub ambiguous: BTreeMap<String, Vec<String>>,
    /// Names the client prints the same way in both languages, so no Russian is owed.
    pub verbatim: BTreeSet<String>,
}

/// The client the distillate was taken from, as the last extract left it.
pub fn client(dir: &Path) -> Result<Client> {
    let path = dir.join(CLIENT);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {} — run `extract` to write it", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn newest_write_ms(entries: &[sources::cache::TocEntry]) -> i64 {
    entries
        .iter()
        .map(|entry| entry.written_ms)
        .max()
        .unwrap_or(0)
}

/// The UTC day of a moment, as `YYYY.MM.DD`.
fn day(unix_ms: i64) -> String {
    // Days counted from 1970-03-01, which puts the leap day at the end of the cycle.
    let days = unix_ms.div_euclid(86_400_000) + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}.{month:02}.{day:02}")
}

/// The client a cache on disk belongs to.
pub fn installed(cache: &Path) -> Result<String> {
    let archive = Archive::open(&cache.join(format!("{PACKAGE}.toc")))?;
    Ok(day(newest_write_ms(archive.entries())))
}

/// What this extract is, so a catalog can say which cache it was built from.
pub fn stamp(dir: &Path) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    for name in [CLIENT, NAMES, OFFERS, GRANTS, INCUBATOR, MINING, SPAWNS] {
        let path = dir.join(name);
        let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        hasher.update(name.as_bytes());
        hasher.update(&[0]);
        hasher.update(&crate::recipe::unix(&bytes));
    }
    Ok(format!("game-{}", &hasher.finalize().to_hex()[..16]))
}

/// What each of the game's vendor manifests sells, as the last extract left it.
pub fn sold(dir: &Path) -> Result<Vec<Stall>> {
    let path = dir.join(OFFERS);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {} — run `extract` to write it", path.display()))?;
    let stalls: Stalls =
        toml::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(stalls.vendor)
}

/// What the game hands over along with an item, as the last extract left it.
pub fn granted(dir: &Path) -> Result<grants::Grants> {
    distilled(dir, GRANTS)
}

/// What the incubator takes and hatches, as the last extract left it.
pub fn hatched(dir: &Path) -> Result<incubator::Incubator> {
    distilled(dir, INCUBATOR)
}

/// Which enemies each tileset sends, as the last extract left it.
pub fn spawned(dir: &Path) -> Result<spawns::Spawns> {
    distilled(dir, SPAWNS)
}

/// What the open worlds' veins yield, as the last extract left it.
pub fn mined(dir: &Path) -> Result<mining::Mining> {
    distilled(dir, MINING)
}

fn distilled<T: serde::de::DeserializeOwned>(dir: &Path, name: &str) -> Result<T> {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {} — run `extract` to write it", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// What the client calls things, as the last extract left it.
pub fn spoken(dir: &Path) -> Result<Names> {
    let path = dir.join(NAMES);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {} — run `extract` to write it", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
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

    warn_if_behind(&tables["en"], &tables["ru"]);
    let names = names(&tables["en"], &tables["ru"]);
    eprintln!(
        "cache    {} names, {} translated two ways, {} left in English",
        names.name.len(),
        names.ambiguous.len(),
        names.verbatim.len()
    );
    write(
        &out_dir.join(NAMES),
        "# Written by `median-data extract` from the game cache: the Russian the client\n\
         # shows for a name it prints in English.\n\n",
        &names,
    )?;

    let archive = Archive::open(&cache.join(format!("{PACKAGE}.toc")))?;
    let client = Client {
        build: day(newest_write_ms(archive.entries())),
    };
    eprintln!("cache    client of {}", client.build);
    write(
        &out_dir.join(CLIENT),
        "# Written by `median-data extract`: which client the distillate was taken from.\n\n",
        &client,
    )?;

    let stalls = stalls(&archive)?;
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
    )?;

    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path == TYPES)
        .with_context(|| format!("{PACKAGE} holds no {TYPES}"))?;
    let types = packages::types(&archive.read(entry)?).context("read the game's types")?;
    eprintln!(
        "cache    {} types, {} with properties of their own",
        types.len(),
        types.iter().filter(|kind| kind.text.is_some()).count()
    );
    let lineage = Lineage::new(types);

    let grants = grants::read(&lineage);
    eprintln!("cache    {} grants", grants.grant.len());
    write(
        &out_dir.join(GRANTS),
        "# Written by `median-data extract` from the game cache: what the game hands over\n\
         # along with an item, in the game's own paths.\n\n",
        &grants,
    )?;

    let incubator = incubator::read(&lineage, &tables["en"], &tables["ru"]);
    eprintln!(
        "cache    {} eggs, {} incubator recipes",
        incubator.egg.len(),
        incubator.recipe.len()
    );
    write(
        &out_dir.join(INCUBATOR),
        "# Written by `median-data extract` from the game cache: what the incubator takes\n\
         # and what it can hatch, in the game's own paths.\n\n",
        &incubator,
    )?;

    let mining = mining::read(&lineage);
    eprintln!("cache    {} mining sites", mining.site.len());
    write(
        &out_dir.join(MINING),
        "# Written by `median-data extract` from the game cache: what the open worlds'\n\
         # veins yield and the tools that work them, in the game's own paths.\n\n",
        &mining,
    )?;

    let spawns = spawns::read(&lineage, &tables["en"], &tables["ru"]);
    eprintln!(
        "cache    {} tilesets, {} enemy rosters",
        spawns.tileset.len(),
        spawns.spec.len()
    );
    write(
        &out_dir.join(SPAWNS),
        "# Written by `median-data extract` from the game cache: which enemies each tileset\n\
         # sends at each mission type, in the game's own paths.\n\n",
        &spawns,
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

/// The launcher only downloads the language pack in use, so one table can be a patch behind
/// the other. A name whose English side is missing cannot be matched to anything, so the
/// count is said out loud rather than quietly lost.
fn warn_if_behind(en: &BTreeMap<String, String>, ru: &BTreeMap<String, String>) {
    let behind = ru.keys().filter(|key| !en.contains_key(*key)).count();
    if behind > 0 {
        eprintln!(
            "cache    the English table is behind: {behind} keys the Russian one has are \
             missing from it — switch the client to English once so the launcher fetches it"
        );
    }
}

fn names(en: &BTreeMap<String, String>, ru: &BTreeMap<String, String>) -> Names {
    let mut said: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut verbatim = BTreeSet::new();
    for (key, english) in en {
        let Some(russian) = ru.get(key) else { continue };
        if !is_name(english) {
            continue;
        }
        if russian == english {
            verbatim.insert(english.clone());
            continue;
        }
        let spoken = said.entry(english).or_default();
        if !spoken.contains(&russian.as_str()) {
            spoken.push(russian);
        }
    }

    let mut names = Names {
        verbatim,
        ..Names::default()
    };
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
    fn a_moment_becomes_the_day_it_falls_on() {
        assert_eq!(day(1_787_173_743_000), "2026.08.19");
        assert_eq!(day(0), "1970.01.01");
        // A leap day, and the day after it.
        assert_eq!(day(1_709_164_800_000), "2024.02.29");
        assert_eq!(day(1_709_251_200_000), "2024.03.01");
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
