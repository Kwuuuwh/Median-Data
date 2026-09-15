use anyhow::{Context, Result};

use crate::net::{agent_as, get};

const RAW: &str = "https://wiki.warframe.com/index.php?action=raw&title=Module:";
const API: &str = "https://wiki.warframe.com/api.php";

/// How many titles one API call may ask about.
pub const BATCH: usize = 50;

/// The wiki asks automated readers to say who they are rather than pose as a browser.
const USER_AGENT: &str = "median-data/0.1 (Warframe catalog builder; one request per module)";

/// An agent that identifies itself to the wiki.
pub fn agent() -> ureq::Agent {
    agent_as(USER_AGENT)
}

/// Data modules we read. The wiki is written by people, so it never outranks DE — it is a
/// second witness and it fills what DE does not export at all.
pub const MISSIONS: &str = "Missions/data";
pub const DROP_TABLES: &str = "DropTables/data";
pub const BARO: &str = "Baro/data";
pub const RESEARCH: &str = "Research/data";
pub const VENDORS: &str = "Vendors/data";
pub const VOID: &str = "Void/data";
pub const BLUEPRINTS: &str = "Blueprints/data";

/// Fetch one data module's Lua source.
pub fn fetch_module(agent: &ureq::Agent, module: &str) -> Result<Vec<u8>> {
    get(agent, &format!("{RAW}{}", encode(module)))
}

/// One file the wiki holds.
#[derive(Debug, Clone)]
pub struct File {
    /// Title without the `File:` prefix, as the wiki spells it.
    pub name: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/// Which of these file titles exist, with where to download them. Titles are asked about in
/// batches; a title the wiki does not hold is simply absent from the answer.
pub fn files(agent: &ureq::Agent, titles: &[String]) -> Result<Vec<File>> {
    let mut out = Vec::new();
    for batch in titles.chunks(BATCH) {
        let query = format!(
            "{API}?action=query&format=json&prop=imageinfo&iiprop=url|size&titles={}",
            encode(&batch.join("|"))
        );
        out.extend(read(&get(agent, &query)?)?);
    }
    Ok(out)
}

/// Every file used on one page.
pub fn page_files(agent: &ureq::Agent, page: &str) -> Result<Vec<File>> {
    let query = format!(
        "{API}?action=query&format=json&generator=images&gimlimit=max&prop=imageinfo\
         &iiprop=url|size&titles={}",
        encode(page)
    );
    read(&get(agent, &query)?)
}

/// Download a file the wiki serves.
pub fn fetch_file(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>> {
    get(agent, url)
}

/// Read the `pages` map of an API answer, keeping the entries that exist.
fn read(raw: &[u8]) -> Result<Vec<File>> {
    let json: serde_json::Value = serde_json::from_slice(raw).context("wiki api answer")?;
    let Some(pages) = json.pointer("/query/pages").and_then(|p| p.as_object()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for page in pages.values() {
        if page.get("missing").is_some() {
            continue;
        }
        let title = page
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or_default();
        let info = page.pointer("/imageinfo/0");
        let Some(url) = info.and_then(|i| i.get("url")).and_then(|u| u.as_str()) else {
            continue;
        };
        out.push(File {
            name: title.trim_start_matches("File:").to_string(),
            url: url.to_string(),
            width: info
                .and_then(|i| i.get("width"))
                .and_then(|w| w.as_u64())
                .unwrap_or(0) as u32,
            height: info
                .and_then(|i| i.get("height"))
                .and_then(|h| h.as_u64())
                .unwrap_or(0) as u32,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Percent-encode a module title for a query string, keeping the `/` the wiki expects.
fn encode(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for b in title.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            b' ' => out.push('_'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_module_title_becomes_a_query_value() {
        assert_eq!(encode("Missions/data"), "Missions/data");
        assert_eq!(encode("Baro/data/visits"), "Baro/data/visits");
        assert_eq!(encode("Void Storm/data"), "Void_Storm/data");
    }
}
