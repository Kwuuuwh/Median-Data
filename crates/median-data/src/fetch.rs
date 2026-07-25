use anyhow::{Result, anyhow};
use sources::de::{self, IndexEntry};
use sources::{drops, wfm, wiki};
use vault::{Entry, Snapshot, Vault};

use crate::spec;

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

/// Pin the wiki data modules we read. They are Lua source, pinned verbatim like any other
/// input, so a build never depends on what the wiki says today.
fn fetch_wiki(vault: &Vault, now_ms: i64) -> Result<()> {
    let agent = wiki::agent();
    let wanted = [
        (spec::WIKI_MISSIONS, wiki::MISSIONS),
        (spec::WIKI_DROPS, wiki::DROP_TABLES),
        (spec::WIKI_BARO, wiki::BARO),
        (spec::WIKI_RESEARCH, wiki::RESEARCH),
        (spec::WIKI_VENDORS, wiki::VENDORS),
    ];
    let mut fetched = Vec::new();
    for (logical, module) in wanted {
        let bytes = wiki::fetch_module(&agent, module)?;
        let blob = vault.put(&bytes)?;
        fetched.push((logical, blob, bytes.len() as u64));
    }

    let joined: Vec<String> = fetched
        .iter()
        .map(|(_, blob, _)| blob.to_string())
        .collect();
    let id = format!(
        "wiki-{}",
        &blake3::hash(joined.join(".").as_bytes()).to_hex()[..16]
    );
    let mut snap = Snapshot::new(id, spec::WIKI, now_ms);
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
    let bytes = wfm::fetch_items(agent)?;
    let blob = vault.put(&bytes)?;
    let id = format!("wfm-{}", &blob.to_string()[..16]);
    let mut snap = Snapshot::new(id, spec::WFM, now_ms);
    snap.entries.push(Entry {
        logical: spec::WFM_ITEMS.to_string(),
        blob: blob.to_string(),
        len: bytes.len() as u64,
    });
    vault.save(&snap)?;
    eprintln!("WFM snapshot {} ({} bytes)", snap.id, bytes.len());
    Ok(())
}

fn fetch_drops(vault: &Vault, agent: &ureq::Agent, now_ms: i64) -> Result<()> {
    let bytes = drops::fetch(agent)?;
    let blob = vault.put(&bytes)?;
    let id = format!("drops-{}", &blob.to_string()[..16]);
    let mut snap = Snapshot::new(id, spec::DROPS, now_ms);
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
