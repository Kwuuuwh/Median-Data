use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use sources::cache::{Archive, TocEntry};
use sources::{language, notation};

/// Languages median reads, and the package each one's string table sits in.
const STRING_TABLES: [(&str, &str); 2] = [("en", "H.Misc_en"), ("ru", "H.Misc_ru")];

/// Package the manifests sit in.
const PACKAGE: &str = "H.Misc";

/// Directories of the cache worth taking whole.
const MANIFEST_DIRS: [&str; 2] = ["/Lotus/Types/Game/VendorManifests/", "/Lotus/Syndicates/"];

/// The string table of one language.
const STRINGS: &str = "/Languages.bin";

/// How hard the artifact is compressed. The file is written once and downloaded on every
/// build, so the slower level pays for itself.
const ZSTD_LEVEL: i32 = 19;

/// What the repository keeps about an extract: enough to download it again and to tell one
/// extract from the next.
#[derive(Debug, Serialize, PartialEq)]
pub struct Pointer {
    pub id: String,
    /// The day the game last wrote to the cache, as `YYYY.MM.DD`.
    pub build: String,
    pub tag: String,
    pub file: String,
    pub blake3: String,
    pub size: u64,
}

/// Read the game cache and write the artifact a build downloads, with the pointer that names it.
pub fn run(cache: &Path, out_dir: &Path, pointer_file: &Path) -> Result<()> {
    if !cache.is_dir() {
        bail!("no game cache at {}", cache.display());
    }
    let mut files = Vec::new();
    for (lang, package) in STRING_TABLES {
        let archive = open(cache, package)?;
        let entry = archive
            .entries()
            .iter()
            .find(|entry| entry.path == STRINGS)
            .with_context(|| format!("{package} holds no {STRINGS}"))?
            .clone();
        let phrases = language::strings(&archive.read(&entry)?)
            .with_context(|| format!("read the {lang} strings"))?;
        eprintln!("cache    {lang} strings — {} keys", phrases.len());
        files.push((
            format!("strings/{lang}.json"),
            serde_json::to_vec(&phrases)?,
        ));
    }

    let archive = open(cache, PACKAGE)?;
    let mut taken = 0;
    for entry in archive.entries() {
        if !MANIFEST_DIRS.iter().any(|dir| entry.path.starts_with(dir)) {
            continue;
        }
        let raw = archive.read(entry)?;
        // Binary files share these directories; they are not manifests.
        if notation::parse(&raw).is_err() {
            eprintln!("cache    skipped {} — not a manifest", entry.path);
            continue;
        }
        files.push((format!("manifests{}", entry.path), raw));
        taken += 1;
    }
    eprintln!("cache    {taken} manifests");

    let build = day(newest_write_ms(archive.entries()));
    let artifact = bundle(&mut files)?;
    let blake3 = blake3::hash(&artifact).to_hex().to_string();
    let id = format!("game-{}", &blake3[..16]);
    let pointer = Pointer {
        file: format!("{id}.tar.zst"),
        tag: format!("cache-{build}"),
        size: artifact.len() as u64,
        id,
        build,
        blake3,
    };

    std::fs::create_dir_all(out_dir)?;
    let written = out_dir.join(&pointer.file);
    std::fs::write(&written, &artifact).with_context(|| format!("write {}", written.display()))?;
    write_pointer(pointer_file, &pointer)?;
    eprintln!(
        "cache    {} — {} MB in {}",
        pointer.id,
        pointer.size / 1_000_000,
        written.display()
    );
    Ok(())
}

fn open(cache: &Path, package: &str) -> Result<Archive> {
    Archive::open(&cache.join(format!("{package}.toc")))
}

/// Pack the files in name order, so the same cache always produces the same artifact.
fn bundle(files: &mut [(String, Vec<u8>)]) -> Result<Vec<u8>> {
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut archive = tar::Builder::new(Vec::new());
    archive.mode(tar::HeaderMode::Deterministic);
    for (name, bytes) in files.iter() {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_cksum();
        archive.append_data(&mut header, name, bytes.as_slice())?;
    }
    let packed = zstd::stream::encode_all(archive.into_inner()?.as_slice(), ZSTD_LEVEL)?;
    Ok(packed)
}

fn write_pointer(path: &Path, pointer: &Pointer) -> Result<()> {
    const HEADER: &str = "# Written by `median-data extract`. Names the artifact a build\n\
                          # downloads the game cache from.\n\n";
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let staging: PathBuf = path.with_extension("toml.writing");
    std::fs::write(
        &staging,
        format!("{HEADER}{}", toml::to_string_pretty(pointer)?),
    )
    .with_context(|| format!("write {}", staging.display()))?;
    std::fs::rename(&staging, path).with_context(|| format!("replace {}", path.display()))
}

fn newest_write_ms(entries: &[TocEntry]) -> i64 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_becomes_the_day_it_falls_on() {
        assert_eq!(day(1_787_173_743_000), "2026.08.19");
        assert_eq!(day(0), "1970.01.01");
        // A leap day, and the day after it.
        assert_eq!(day(1_709_164_800_000), "2024.02.29");
        assert_eq!(day(1_709_251_200_000), "2024.03.01");
    }

    #[test]
    fn the_same_files_pack_to_the_same_bytes() {
        let mut one = vec![
            ("strings/ru.json".to_string(), b"{}".to_vec()),
            (
                "manifests/Lotus/Syndicates/CetusManifest".to_string(),
                b"Favors={\n}\n".to_vec(),
            ),
        ];
        let mut other = vec![
            (
                "manifests/Lotus/Syndicates/CetusManifest".to_string(),
                b"Favors={\n}\n".to_vec(),
            ),
            ("strings/ru.json".to_string(), b"{}".to_vec()),
        ];

        assert_eq!(bundle(&mut one).unwrap(), bundle(&mut other).unwrap());
    }

    #[test]
    fn the_pointer_names_the_artifact_and_its_tag() {
        let pointer = Pointer {
            id: "game-0123456789abcdef".to_string(),
            build: "2026.09.22".to_string(),
            tag: "cache-2026.09.22".to_string(),
            file: "game-0123456789abcdef.tar.zst".to_string(),
            blake3: "0123456789abcdef".to_string(),
            size: 7,
        };

        let written = toml::to_string_pretty(&pointer).unwrap();

        assert!(written.contains("tag = \"cache-2026.09.22\""));
        assert!(written.contains("file = \"game-0123456789abcdef.tar.zst\""));
    }
}
