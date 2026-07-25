use anyhow::Result;

use crate::net::get;

const INDEX_BASE: &str = "https://content.warframe.com/PublicExport/index_";
const MANIFEST_BASE: &str = "https://content.warframe.com/PublicExport/Manifest/";
/// A `textureLocation` is appended to this verbatim, hash suffix and all.
const TEXTURE_BASE: &str = "https://content.warframe.com/PublicExport";

/// A parsed line of the DE Public Export index.
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// Manifest base name, e.g. `ExportResources`.
    pub manifest: String,
    /// Full index token `Export…_en.json!00_<hash>`.
    pub file: String,
    /// DE content-hash token, e.g. `00_<hash>`.
    pub hash: String,
}

/// Fetch and LZMA-decode `index_<lang>.txt.lzma` into index entries.
pub fn fetch_index(agent: &ureq::Agent, lang: &str) -> Result<Vec<IndexEntry>> {
    let url = format!("{INDEX_BASE}{lang}.txt.lzma");
    let compressed = get(agent, &url)?;
    let mut raw = Vec::new();
    lzma_rs::lzma_decompress(&mut std::io::Cursor::new(&compressed), &mut raw)?;
    let text = String::from_utf8_lossy(&raw);
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(parse_line)
        .collect())
}

/// Fetch one manifest's raw bytes by index entry.
pub fn fetch_manifest(agent: &ureq::Agent, entry: &IndexEntry) -> Result<Vec<u8>> {
    get(agent, &format!("{MANIFEST_BASE}{}", entry.file))
}

/// Fetch an icon by the `textureLocation` a manifest gives for it.
pub fn fetch_texture(agent: &ureq::Agent, location: &str) -> Result<Vec<u8>> {
    get(agent, &format!("{TEXTURE_BASE}{location}"))
}

/// Replace control bytes with spaces so `serde_json` accepts DE's strings.
pub fn sanitize(raw: &[u8]) -> String {
    let cleaned: Vec<u8> = raw
        .iter()
        .map(|&b| if b < 0x20 { b' ' } else { b })
        .collect();
    String::from_utf8_lossy(&cleaned).into_owned()
}

fn parse_line(line: &str) -> IndexEntry {
    let (left, hash) = line.split_once('!').unwrap_or((line, ""));
    let stem = left.strip_suffix(".json").unwrap_or(left);
    let manifest = stem.rsplit_once('_').map(|(base, _)| base).unwrap_or(stem);
    IndexEntry {
        manifest: manifest.to_string(),
        file: line.to_string(),
        hash: hash.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_line() {
        let e = parse_line("ExportResources_en.json!00_abc");
        assert_eq!(e.manifest, "ExportResources");
        assert_eq!(e.hash, "00_abc");
    }

    #[test]
    fn sanitize_strips_control_chars() {
        let v: serde_json::Value = serde_json::from_str(&sanitize(b"{\"n\":\"a\x01b\"}")).unwrap();
        assert_eq!(v["n"], "a b");
    }
}
