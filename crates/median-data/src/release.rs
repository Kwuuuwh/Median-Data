use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use flate2::Compression;
use flate2::write::GzEncoder;
use funnel::State;
use serde::Serialize;
use vault::BlobId;

/// What the build stamped the catalog with, repeated in the manifest so a client can read it
/// before downloading the database itself.
pub struct Stamp {
    pub schema: u32,
    pub fetched_ms: i64,
    /// The pinned snapshot each source was at, so the next run can tell whether anything moved
    /// without holding a vault of its own.
    pub sources: BTreeMap<String, String>,
}

/// Where the publishable artifacts are written.
const DIST: &str = "dist";
/// Names an application asks for by. They never change, so a client can be written once.
const DB: &str = "catalog.sqlite.gz";
const PACK: &str = "pack.tar";
const INDEX: &str = "pack.index";
const MANIFEST: &str = "manifest.json";

/// What a client needs to know before it downloads anything: whether it can read this schema
/// at all, what the version is, and how many bytes each part costs.
#[derive(Serialize)]
struct Manifest {
    schema: u32,
    version: String,
    /// When the game export this was built from was published.
    fetched_ms: i64,
    sources: BTreeMap<String, String>,
    db: Part,
    pack: Pack,
    /// Everything a fresh install downloads, so a progress bar can start at zero.
    total: u64,
}

#[derive(Serialize)]
struct Part {
    file: String,
    blake3: String,
    size: u64,
    /// Size after decompression, where the file is compressed.
    #[serde(skip_serializing_if = "Option::is_none")]
    unpacked: Option<u64>,
}

#[derive(Serialize)]
struct Pack {
    #[serde(flatten)]
    part: Part,
    /// How many pictures the pack holds.
    count: usize,
    /// The same pack minus everything the named release already shipped. Pictures are named
    /// by their content, so a client that has that release needs only this.
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<Delta>,
}

#[derive(Serialize)]
struct Delta {
    from: String,
    #[serde(flatten)]
    part: Part,
    count: usize,
}

/// Package what the last build produced. Nothing is fetched, nothing is rebuilt and nothing is
/// uploaded: this only turns the artifacts on disk into the files a release is made of, so the
/// same command produces the same release from the same build.
pub fn run(
    db: &Path,
    pack: &Path,
    state: &Path,
    stamp: Stamp,
    previous: Option<&Path>,
) -> Result<()> {
    let state: State = serde_json::from_slice(&fs::read(state).with_context(|| {
        format!(
            "read {} — run `build` first, the version comes from it",
            state.display()
        )
    })?)?;
    let version = state
        .version
        .clone()
        .context("the build wrote no version into its state")?;
    let Stamp {
        schema,
        fetched_ms,
        sources,
    } = stamp;

    if let Some(dir) = previous {
        let was = released(dir)?;
        if was.as_deref() == Some(version.as_str()) {
            bail!("version {version} is already released — nothing changed since it was built");
        }
        if let Some(was) = was {
            let (major, _) = split(&was);
            if major != schema {
                eprintln!(
                    "release  schema {major} -> {schema}: an application built for {was} cannot \
                     read this one"
                );
            }
        }
    }

    let dist = PathBuf::from(DIST);
    fs::create_dir_all(&dist)?;
    let db_part = gzip(db, &dist.join(DB))?;
    let ids = pack_ids(pack)?;
    let pack_part = tar(pack, &dist.join(PACK), &ids)?;

    let delta = match previous.map(shipped).transpose()?.flatten() {
        Some((from, had)) => {
            let fresh: BTreeSet<String> = ids.difference(&had).cloned().collect();
            let name = format!("pack-{from}.tar");
            Some(Delta {
                part: tar(pack, &dist.join(&name), &fresh)?,
                count: fresh.len(),
                from,
            })
        }
        None => None,
    };

    let manifest = Manifest {
        schema,
        version: version.clone(),
        fetched_ms,
        sources,
        total: db_part.size + pack_part.size,
        db: db_part,
        pack: Pack {
            part: pack_part,
            count: ids.len(),
            delta,
        },
    };
    fs::write(dist.join(MANIFEST), serde_json::to_vec_pretty(&manifest)?)?;
    fs::write(dist.join(INDEX), index(&ids))?;
    fs::copy("catalog.state.json", dist.join("catalog.state.json"))?;

    eprintln!(
        "release  {version} — {} ({} MB) + {} pictures ({} MB)",
        DB,
        mb(manifest.db.size),
        manifest.pack.count,
        mb(manifest.pack.part.size)
    );
    match &manifest.pack.delta {
        Some(d) => eprintln!(
            "release  delta from {} — {} pictures ({} MB)",
            d.from,
            d.count,
            mb(d.part.size)
        ),
        None => eprintln!("release  no previous release given, no delta written"),
    }
    eprintln!("release  files are in {DIST}/, upload them yourself");
    Ok(())
}

/// The version of the release a directory holds, from the state file published with it.
fn released(dir: &Path) -> Result<Option<String>> {
    let path = dir.join("catalog.state.json");
    if !path.exists() {
        return Ok(None);
    }
    let state: State = serde_json::from_slice(&fs::read(path)?)?;
    Ok(state.version)
}

/// The pictures a previous release shipped, by the index published with it.
fn shipped(dir: &Path) -> Result<Option<(String, BTreeSet<String>)>> {
    let index = dir.join(INDEX);
    let Some(version) = released(dir)? else {
        return Ok(None);
    };
    if !index.exists() {
        return Ok(None);
    }
    let ids = fs::read_to_string(index)?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    Ok(Some((version, ids)))
}

/// Every picture in the pack, by file name. Names are content hashes, so the set alone says
/// what a client already holds.
fn pack_ids(pack: &Path) -> Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for entry in fs::read_dir(pack).with_context(|| format!("read {}", pack.display()))? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "webp")
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            out.insert(stem.to_string());
        }
    }
    Ok(out)
}

fn index(ids: &BTreeSet<String>) -> String {
    let mut out = String::with_capacity(ids.len() * 17);
    for id in ids {
        out.push_str(id);
        out.push('\n');
    }
    out
}

/// Compress one file, reporting what came out.
fn gzip(from: &Path, to: &Path) -> Result<Part> {
    let raw = fs::read(from).with_context(|| format!("read {}", from.display()))?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&raw)?;
    let packed = encoder.finish()?;
    fs::write(to, &packed)?;
    Ok(part(to, &packed, Some(raw.len() as u64)))
}

/// Archive the named pictures, in name order so the same set always produces the same file.
fn tar(pack: &Path, to: &Path, ids: &BTreeSet<String>) -> Result<Part> {
    let file = BufWriter::new(File::create(to)?);
    let mut archive = tar::Builder::new(file);
    archive.mode(tar::HeaderMode::Deterministic);
    for id in ids {
        let name = format!("{id}.webp");
        archive.append_path_with_name(pack.join(&name), &name)?;
    }
    archive.into_inner()?.flush()?;
    let bytes = fs::read(to)?;
    Ok(part(to, &bytes, None))
}

fn part(path: &Path, bytes: &[u8], unpacked: Option<u64>) -> Part {
    Part {
        file: path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string(),
        blake3: BlobId::of(bytes).to_string(),
        size: bytes.len() as u64,
        unpacked,
    }
}

fn split(version: &str) -> (u32, String) {
    match version.split_once('.') {
        Some((major, rest)) => (major.parse().unwrap_or(0), rest.to_string()),
        None => (0, version.to_string()),
    }
}

fn mb(bytes: u64) -> u64 {
    bytes / 1_000_000
}
