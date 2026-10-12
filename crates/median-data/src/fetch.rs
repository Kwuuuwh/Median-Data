use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow};
use sources::de::{self, IndexEntry};
use sources::{drops, wfm, wiki};
use vault::{Entry, Snapshot, Vault};

use crate::spec;

/// What a source's latest snapshot would be called if it were pinned right now, beside what
/// the vault already holds. Equal ids mean the source published nothing new.
pub struct Change {
    pub source: &'static str,
    pub pinned: Option<String>,
    pub current: String,
}

impl Change {
    pub fn moved(&self) -> bool {
        self.pinned.as_deref() != Some(self.current.as_str())
    }
}

/// Name the recipe is reported under.
pub const RECIPE: &str = "recipe";

/// Whether any source or the recipe moved since the last build, reported line by line.
pub fn changed(
    vault: &Vault,
    released: Option<&std::path::Path>,
    state_file: &std::path::Path,
) -> Result<bool> {
    Ok(changes(vault, released, state_file)?
        .iter()
        .any(Change::moved))
}

/// Every source and the recipe beside what the last build was made of, reported line by line.
pub fn changes(
    vault: &Vault,
    released: Option<&std::path::Path>,
    state_file: &std::path::Path,
) -> Result<Vec<Change>> {
    let was: BTreeMap<String, String> = match released {
        Some(path) => {
            let raw = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
            let manifest: serde_json::Value = serde_json::from_slice(&raw)?;
            manifest
                .get("sources")
                .and_then(|s| s.as_object())
                .map(|map| {
                    map.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                        .collect()
                })
                .unwrap_or_default()
        }
        None => BTreeMap::new(),
    };

    let mut changes = check(vault)?;
    if released.is_some() {
        for change in &mut changes {
            change.pinned = was.get(change.source).cloned();
        }
    }
    changes.push(Change {
        source: RECIPE,
        pinned: std::fs::read(state_file)
            .ok()
            .and_then(|raw| serde_json::from_slice::<funnel::State>(&raw).ok())
            .and_then(|last| last.recipe),
        current: crate::recipe::fingerprint(std::path::Path::new("."))?,
    });

    for change in &changes {
        let state = match (change.moved(), &change.pinned) {
            (false, _) => "unchanged".to_string(),
            (true, None) => format!("new — {}", change.current),
            (true, Some(was)) => format!("moved — {was} -> {}", change.current),
        };
        eprintln!("{:<8} {state}", change.source);
    }
    Ok(changes)
}

/// Ask every source whether it has anything new, as cheaply as each can be asked. DE answers
/// from its own index — a few kilobytes naming the content hash of every manifest — so a game
/// patch is detected without downloading a single manifest. The other three are small enough
/// to read whole.
pub fn check(vault: &Vault) -> Result<Vec<Change>> {
    let agent = sources::agent();
    let mut out = Vec::new();

    let mut wanted: Vec<(String, IndexEntry)> = Vec::new();
    for lang in spec::LANGS {
        let index = de::fetch_index(&agent, lang)?;
        for name in manifests(lang) {
            let entry = index
                .iter()
                .find(|e| e.manifest == name)
                .ok_or_else(|| anyhow!("DE index ({lang}) missing manifest {name}"))?;
            wanted.push((name.to_string(), entry.clone()));
        }
    }
    out.push(change(vault, spec::DE, snapshot_id(&wanted)));

    let blobs: Vec<String> = wfm_lists(&agent)?
        .iter()
        .map(|(_, bytes)| vault_hash(bytes))
        .collect();
    out.push(change(vault, spec::WFM, wfm_id(&blobs)));

    let tables = drops::fetch(&agent)?;
    out.push(change(vault, spec::DROPS, drops_id(&vault_hash(&tables))));

    let blobs: Vec<String> = wiki_sources()?
        .iter()
        .map(|(_, bytes)| vault_hash(bytes))
        .collect();
    out.push(change(vault, spec::WIKI, wiki_id(&blobs)));

    Ok(out)
}

fn change(vault: &Vault, source: &'static str, current: String) -> Change {
    Change {
        source,
        pinned: vault.latest(source).ok().map(|s| s.id),
        current,
    }
}

fn vault_hash(bytes: &[u8]) -> String {
    vault::BlobId::of(bytes).to_string()
}

/// The wiki modules pinned as one snapshot, with the logical name each is stored under.
fn wiki_modules() -> Vec<(&'static str, &'static str)> {
    let mut modules = vec![
        (spec::WIKI_MISSIONS, wiki::MISSIONS),
        (spec::WIKI_DROPS, wiki::DROP_TABLES),
        (spec::WIKI_BARO, wiki::BARO),
        (spec::WIKI_RESEARCH, wiki::RESEARCH),
        (spec::WIKI_VENDORS, wiki::VENDORS),
        (spec::WIKI_VOID, wiki::VOID),
        (spec::WIKI_BLUEPRINTS, wiki::BLUEPRINTS),
    ];
    modules.extend(spec::WIKI_WEAPONS.into_iter().zip(wiki::WEAPONS));
    modules
}

/// Every wiki module and article pinned, with the logical name each is stored under.
fn wiki_sources() -> Result<Vec<(&'static str, Vec<u8>)>> {
    let agent = wiki::agent();
    let mut out = Vec::new();
    for (logical, module) in wiki_modules() {
        out.push((logical, wiki::fetch_module(&agent, module)?));
    }
    out.push((
        spec::WIKI_RIVEN_MODS,
        wiki::fetch_page(&agent, wiki::RIVEN_MODS)?,
    ));
    Ok(out)
}

fn wiki_id(blobs: &[String]) -> String {
    format!(
        "wiki-{}",
        &blake3::hash(blobs.join(".").as_bytes()).to_hex()[..16]
    )
}

fn wfm_id(blobs: &[String]) -> String {
    format!(
        "wfm-{}",
        &blake3::hash(blobs.join(".").as_bytes()).to_hex()[..16]
    )
}

/// The market lists pinned as one snapshot, with the logical name each is stored under.
fn wfm_lists(agent: &ureq::Agent) -> Result<Vec<(&'static str, Vec<u8>)>> {
    Ok(vec![
        (spec::WFM_ITEMS, wfm::fetch_items(agent)?),
        (spec::WFM_RIVEN_WEAPONS, wfm::fetch_riven_weapons(agent)?),
        (
            spec::WFM_RIVEN_ATTRIBUTES,
            wfm::fetch_riven_attributes(agent)?,
        ),
    ])
}

fn drops_id(blob: &str) -> String {
    format!("drops-{}", &blob[..16])
}

/// Pin the DE manifests (English and Russian), the WFM item list and the official drop
/// tables into the vault.
pub fn run(vault: &Vault, now_ms: i64) -> Result<()> {
    let agent = sources::agent();
    fetch_de(vault, &agent, now_ms)?;
    fetch_wfm(vault, &agent, now_ms)?;
    fetch_drops(vault, &agent, now_ms)?;
    fetch_wiki(vault, now_ms)?;
    Ok(())
}

/// Pin one source, for when only it moved.
pub fn one(vault: &Vault, source: &str, now_ms: i64) -> Result<()> {
    let agent = sources::agent();
    match source {
        spec::DE => fetch_de(vault, &agent, now_ms),
        spec::WFM => fetch_wfm(vault, &agent, now_ms),
        spec::DROPS => fetch_drops(vault, &agent, now_ms),
        spec::WIKI => fetch_wiki(vault, now_ms),
        other => anyhow::bail!("unknown source: {other}"),
    }
}

/// Pin the wiki data modules we read. They are Lua source, pinned verbatim like any other
/// input, so a build never depends on what the wiki says today.
fn fetch_wiki(vault: &Vault, now_ms: i64) -> Result<()> {
    let mut fetched = Vec::new();
    for (logical, bytes) in wiki_sources()? {
        let blob = vault.put(&bytes)?;
        fetched.push((logical, blob, bytes.len() as u64));
    }

    let joined: Vec<String> = fetched
        .iter()
        .map(|(_, blob, _)| blob.to_string())
        .collect();
    let mut snap = Snapshot::new(wiki_id(&joined), spec::WIKI, now_ms);
    for (logical, blob, len) in fetched {
        snap.entries.push(Entry {
            logical: logical.to_string(),
            blob: blob.to_string(),
            len,
        });
    }
    vault.save(&snap)?;
    eprintln!("wiki snapshot {} — {} modules", snap.id, snap.entries.len());
    Ok(())
}

fn fetch_de(vault: &Vault, agent: &ureq::Agent, now_ms: i64) -> Result<()> {
    let mut wanted: Vec<(String, IndexEntry)> = Vec::new();
    for lang in spec::LANGS {
        let index = de::fetch_index(agent, lang)?;
        for name in manifests(lang) {
            let entry = index
                .iter()
                .find(|e| e.manifest == name)
                .ok_or_else(|| anyhow!("DE index ({lang}) missing manifest {name}"))?;
            let logical = if *lang == "en" {
                name.to_string()
            } else {
                spec::ru(name)
            };
            wanted.push((logical, entry.clone()));
        }
    }

    let mut snap = Snapshot::new(snapshot_id(&wanted), spec::DE, now_ms);
    for (logical, entry) in &wanted {
        let bytes = de::fetch_manifest(agent, entry)?;
        let blob = vault.put(&bytes)?;
        snap.entries.push(Entry {
            logical: logical.clone(),
            blob: blob.to_string(),
            len: bytes.len() as u64,
        });
        eprintln!("pinned {logical} ({} bytes)", bytes.len());
    }
    vault.save(&snap)?;
    eprintln!("DE snapshot {} — {} manifests", snap.id, snap.entries.len());
    Ok(())
}

/// Recipes and texture locations carry no display names, so those are pinned once. The item
/// manifests and the star chart are pinned per language.
fn manifests(lang: &str) -> Vec<&'static str> {
    let mut names = spec::ITEM_MANIFESTS.to_vec();
    names.push(spec::REGIONS);
    if lang == "en" {
        names.push(spec::RECIPES);
        names.push(spec::TEXTURES);
    }
    names
}

fn fetch_wfm(vault: &Vault, agent: &ureq::Agent, now_ms: i64) -> Result<()> {
    let mut entries = Vec::new();
    for (logical, bytes) in wfm_lists(agent)? {
        let blob = vault.put(&bytes)?;
        entries.push(Entry {
            logical: logical.to_string(),
            blob: blob.to_string(),
            len: bytes.len() as u64,
        });
    }
    let blobs: Vec<String> = entries.iter().map(|e| e.blob.clone()).collect();
    let mut snap = Snapshot::new(wfm_id(&blobs), spec::WFM, now_ms);
    snap.entries = entries;
    vault.save(&snap)?;
    eprintln!("WFM snapshot {} — {} lists", snap.id, snap.entries.len());
    Ok(())
}

fn fetch_drops(vault: &Vault, agent: &ureq::Agent, now_ms: i64) -> Result<()> {
    let bytes = drops::fetch(agent)?;
    let blob = vault.put(&bytes)?;
    let mut snap = Snapshot::new(drops_id(&blob.to_string()), spec::DROPS, now_ms);
    snap.entries.push(Entry {
        logical: spec::DROP_TABLES.to_string(),
        blob: blob.to_string(),
        len: bytes.len() as u64,
    });
    vault.save(&snap)?;
    eprintln!("drops snapshot {} ({} bytes)", snap.id, bytes.len());
    Ok(())
}

/// DE snapshot id from the content hashes of everything pinned.
fn snapshot_id(wanted: &[(String, IndexEntry)]) -> String {
    let joined = wanted
        .iter()
        .map(|(_, e)| e.hash.as_str())
        .collect::<Vec<_>>()
        .join(".");
    format!("de-{}", &blake3::hash(joined.as_bytes()).to_hex()[..16])
}
