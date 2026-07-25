use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use consensus::Source;
use graph::Graph;
use sources::wfm::WfmItem;
use vault::{BlobId, Entry, Snapshot, Vault};

use crate::{build, spec};

/// DE puts a mod's card artwork here instead of an icon: the scene painted on the card,
/// without the frame, the name or the stats. The market renders the whole card, per
/// language, so anything DE answers from here is better taken from there.
const ARTWORK: &str = "/Cards/Images/";

/// Logical name of the entry saying which vendor each pinned picture belongs to.
const INDEX: &str = "index";

/// Pin the picture of every item the product ships. DE textures carry DE's own content hash
/// in their location and market assets carry theirs, so anything pinned under the same
/// location is still current and is reused instead of downloaded again.
pub fn run(vault: &Vault, scope_path: &Path, now_ms: i64) -> Result<()> {
    let built = build::graph(vault)?;
    let policy = projections::load(scope_path)?;
    let scope = projections::apply(&built.graph, &policy);
    let shipped = projections::kept(&built.graph, &scope);

    let de_snap = vault.latest(spec::DE)?;
    let textures = crate::extract::de_textures(&build::blob(vault, &de_snap, spec::TEXTURES)?)?;

    let wanted: BTreeSet<&str> = shipped
        .iter()
        .filter_map(|path| textures.get(path).map(String::as_str))
        .collect();
    pin(vault, spec::ICONS, &wanted, now_ms, |agent, location| {
        sources::de::fetch_texture(agent, location)
    })?;

    let cards = cards(&built.graph, &built.wfm, &textures);
    let wanted: BTreeSet<&str> = cards
        .iter()
        .filter(|(path, _)| shipped.contains(*path))
        .flat_map(|(_, langs)| langs.values().map(String::as_str))
        .collect();
    pin(vault, spec::WFM_ICONS, &wanted, now_ms, |agent, path| {
        sources::wfm::fetch_asset(agent, path)
    })?;

    portraits(vault, now_ms)?;
    Ok(())
}

/// Pin a picture for every vendor. They are people and places, not items, so DE has nothing
/// for them; the wiki illustrates its own pages and is the only source there is.
fn portraits(vault: &Vault, now_ms: i64) -> Result<()> {
    let snap = vault.latest(spec::WIKI)?;
    let stores = crate::wiki::vendors(&build::blob(vault, &snap, spec::WIKI_VENDORS)?)?;
    let mut wanted: BTreeMap<String, String> = BTreeMap::new();
    for store in &stores {
        let page = crate::vendors::person(store);
        wanted.insert(crate::vendors::slug(&page), page);
    }
    // Baro comes from his own module, so the stock list does not name him.
    let (key, page) = crate::vendors::BARO_PAGE;
    wanted.insert(key.to_string(), page.to_string());
    let pages: Vec<(String, String)> = wanted.into_iter().collect();

    let agent = sources::wiki::agent();
    let chosen = crate::portraits::choose(&agent, &pages)?;
    let known = pinned(vault, spec::PORTRAITS);
    let mut snap = Snapshot::new(
        format!("{}-{}", spec::PORTRAITS, chosen.len()),
        spec::PORTRAITS,
        now_ms,
    );
    let mut index: BTreeMap<&str, &str> = BTreeMap::new();
    let (mut fetched, mut reused) = (0, 0);

    for portrait in &chosen {
        index.insert(&portrait.vendor, &portrait.file.url);
        if let Some(blob) = known.get(&portrait.file.url) {
            snap.entries
                .push(entry(&portrait.file.url, blob.clone(), 0));
            reused += 1;
            continue;
        }
        match sources::wiki::fetch_file(&agent, &portrait.file.url) {
            Ok(bytes) => {
                let blob = vault.put(&bytes)?;
                snap.entries.push(entry(
                    &portrait.file.url,
                    blob.to_string(),
                    bytes.len() as u64,
                ));
                fetched += 1;
            }
            Err(e) => eprintln!("skip portrait {}: {e:#}", portrait.file.name),
        }
    }

    // Which vendor a picture belongs to is not in the file itself, so it is pinned beside it.
    let bytes = serde_json::to_vec(&index)?;
    let blob = vault.put(&bytes)?;
    snap.entries
        .push(entry(INDEX, blob.to_string(), bytes.len() as u64));
    vault.save(&snap)?;

    eprintln!(
        "{:<8} {} of {} vendors have a picture ({fetched} fetched, {reused} reused)",
        spec::PORTRAITS,
        chosen.len(),
        pages.len()
    );
    Ok(())
}

/// Fetch everything a source wants that the vault does not already hold, and record the lot
/// as that source's latest snapshot.
fn pin(
    vault: &Vault,
    source: &str,
    wanted: &BTreeSet<&str>,
    now_ms: i64,
    fetch: impl Fn(&ureq::Agent, &str) -> Result<Vec<u8>>,
) -> Result<()> {
    let known = pinned(vault, source);
    let agent = sources::agent();
    let mut snap = Snapshot::new(format!("{source}-{}", wanted.len()), source, now_ms);
    let (mut fetched, mut reused, mut failed) = (0, 0, 0);

    for location in wanted {
        if let Some(blob) = known.get(*location) {
            snap.entries.push(entry(location, blob.clone(), 0));
            reused += 1;
            continue;
        }
        match fetch(&agent, location) {
            Ok(bytes) => {
                let blob = vault.put(&bytes)?;
                snap.entries
                    .push(entry(location, blob.to_string(), bytes.len() as u64));
                fetched += 1;
                // Checkpoint, so a run cut short leaves its downloads reusable instead of
                // making the next run fetch them again.
                if fetched % 250 == 0 {
                    vault.save(&snap)?;
                    eprintln!("  fetched {fetched} of {}", wanted.len() - reused);
                }
            }
            Err(e) => {
                eprintln!("skip {location}: {e:#}");
                failed += 1;
            }
        }
    }

    vault.save(&snap)?;
    eprintln!(
        "{source:<8} {} pinned ({fetched} fetched, {reused} reused, {failed} failed)",
        snap.entries.len()
    );
    Ok(())
}

/// Market card assets per language, for the items whose DE texture is artwork rather than
/// an icon. Items DE illustrates properly keep their DE icon.
pub fn cards(
    graph: &Graph,
    wfm: &[WfmItem],
    textures: &BTreeMap<String, String>,
) -> BTreeMap<String, BTreeMap<String, String>> {
    let by_slug: BTreeMap<&str, &WfmItem> = wfm.iter().map(|w| (w.slug.as_str(), w)).collect();
    let mut out = BTreeMap::new();
    for item in graph.items() {
        if !textures
            .get(&item.unique_name)
            .is_some_and(|loc| loc.contains(ARTWORK))
        {
            continue;
        }
        let Some(slug) = &item.slug else { continue };
        let Some(listing) = by_slug.get(slug.value.as_str()) else {
            continue;
        };
        if !listing.icons.is_empty() {
            out.insert(item.unique_name.clone(), listing.icons.clone());
        }
    }
    out
}

/// Item path -> language -> the blob holding its picture, and where that picture came from.
pub type Pictures = BTreeMap<String, BTreeMap<String, (String, Source)>>;

/// Resolve every shipped item to the picture the vault holds for it, per language. The
/// market's card wins where there is one, because DE only offered artwork there.
pub fn pictures(
    vault: &Vault,
    cards: &BTreeMap<String, BTreeMap<String, String>>,
    textures: &BTreeMap<String, String>,
) -> Pictures {
    let de = pinned(vault, spec::ICONS);
    let assets = pinned(vault, spec::WFM_ICONS);
    let mut out: Pictures = BTreeMap::new();

    for (item, location) in textures {
        if let Some(blob) = de.get(location) {
            let langs = out.entry(item.clone()).or_default();
            for lang in graph::LANGS {
                langs.insert((*lang).to_string(), (blob.clone(), Source::De));
            }
        }
    }
    for (vendor, blob) in vendor_pictures(vault) {
        let langs = out.entry(graph::vendor_id(&vendor)).or_default();
        for lang in graph::LANGS {
            langs.insert((*lang).to_string(), (blob.clone(), Source::Wiki));
        }
    }
    for (item, langs) in cards {
        let held: BTreeMap<&str, &String> = langs
            .iter()
            .filter_map(|(lang, path)| assets.get(path).map(|b| (lang.as_str(), b)))
            .collect();
        if held.is_empty() {
            continue;
        }
        let slot = out.entry(item.clone()).or_default();
        for lang in graph::LANGS {
            let blob = held
                .get(lang)
                .copied()
                .or_else(|| held.values().next().copied());
            if let Some(blob) = blob {
                slot.insert((*lang).to_string(), (blob.clone(), Source::Wfm));
            }
        }
    }
    out
}

/// Pictures for an item, read from the vault.
pub struct Pinned<'a> {
    vault: &'a Vault,
    pictures: Pictures,
}

impl<'a> Pinned<'a> {
    /// Open the pictures pinned by the last icon run. `None` when none were.
    pub fn open(vault: &'a Vault, pictures: Pictures) -> Option<Self> {
        (!pictures.is_empty()).then_some(Self { vault, pictures })
    }

    /// The picture's bytes and its source.
    pub fn picture(&self, unique_name: &str, lang: &str) -> Option<(Vec<u8>, Source)> {
        let (blob, from) = self.pictures.get(unique_name)?.get(lang)?;
        let bytes = self.vault.get(&BlobId::from_hex(blob.clone())).ok()?;
        Some((bytes, *from))
    }
}

impl projections::IconSource for Pinned<'_> {
    fn bytes(&self, unique_name: &str, lang: &str) -> Option<Vec<u8>> {
        self.picture(unique_name, lang).map(|(bytes, _)| bytes)
    }
}

/// Vendor key -> the blob holding their picture, as the last icon run pinned it.
fn vendor_pictures(vault: &Vault) -> BTreeMap<String, String> {
    let held = pinned(vault, spec::PORTRAITS);
    let Some(blob) = held.get(INDEX) else {
        return BTreeMap::new();
    };
    let Ok(raw) = vault.get(&BlobId::from_hex(blob.clone())) else {
        return BTreeMap::new();
    };
    let index: BTreeMap<String, String> = serde_json::from_slice(&raw).unwrap_or_default();
    index
        .into_iter()
        .filter_map(|(vendor, url)| held.get(&url).map(|blob| (vendor, blob.clone())))
        .collect()
}

/// Locations a source already holds in the vault, from its last run.
fn pinned(vault: &Vault, source: &str) -> BTreeMap<String, String> {
    let Ok(snap) = vault.latest(source) else {
        return BTreeMap::new();
    };
    snap.entries
        .into_iter()
        .filter(|e| vault.get(&BlobId::from_hex(e.blob.clone())).is_ok())
        .map(|e| (e.logical, e.blob))
        .collect()
}

fn entry(location: &str, blob: String, len: u64) -> Entry {
    Entry {
        logical: location.to_string(),
        blob,
        len,
    }
}
