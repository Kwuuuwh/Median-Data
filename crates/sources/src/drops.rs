use anyhow::{Result, anyhow};
use scraper::{ElementRef, Html, Selector};

use crate::net::get;

/// Landing URL that redirects to the current drop-table document.
const URL: &str = "https://www.warframe.com/droptables";

/// One printed drop line: an item, where it drops from, and how often.
#[derive(Debug, Clone, PartialEq)]
pub struct Drop {
    /// Section id the row came from, e.g. `missionRewards`.
    pub section: String,
    /// Item name as printed.
    pub item: String,
    /// Place or enemy name as printed.
    pub place: String,
    /// Rotation letter, when the place rotates.
    pub rotation: Option<String>,
    /// Bounty stage label, when the place is staged.
    pub stage: Option<String>,
    /// Rarity label as printed.
    pub rarity: String,
    /// Drop probability within the table, in `[0, 1]`.
    pub chance: f64,
    /// An enemy's chance to roll this table at all.
    pub table_chance: Option<f64>,
}

/// One printed relic reward line. The catalog takes relic rewards from the DE export;
/// these are kept as an independent witness to check that against.
#[derive(Debug, Clone, PartialEq)]
pub struct RelicRow {
    /// Relic name as printed, without the refinement.
    pub relic: String,
    /// Refinement printed in the header, lowercased.
    pub refinement: String,
    pub reward: String,
    pub rarity: String,
    pub chance: f64,
}

/// Everything parsed out of the drop-table document.
#[derive(Debug, Default)]
pub struct Tables {
    pub drops: Vec<Drop>,
    pub relics: Vec<RelicRow>,
    /// Sections carrying a table this parser does not read.
    pub skipped: Vec<String>,
}

/// How a section's table is laid out.
#[derive(Clone, Copy)]
enum Layout {
    /// Place header, optional rotation, then `item | rarity`.
    Place,
    /// Place header, rotation, stage, then `_ | item | rarity`.
    Bounty,
    /// Enemy header with its table chance, then `_ | item | rarity`.
    ByAvatar,
    /// Item header, then `enemy | table chance | rarity`.
    ByDrop,
}

/// Layout of each known section. Relic rewards are read from the DE export instead.
fn layout(section: &str) -> Option<Layout> {
    Some(match section {
        "missionRewards" | "keyRewards" | "transientRewards" | "sortieRewards" => Layout::Place,
        "cetusRewards" | "solarisRewards" | "deimosRewards" | "zarimanRewards"
        | "entratiLabRewards" | "hexRewards" => Layout::Bounty,
        "modByAvatar"
        | "blueprintByAvatar"
        | "resourceByAvatar"
        | "sigilByAvatar"
        | "additionalItemByAvatar"
        | "relicByAvatar" => Layout::ByAvatar,
        "modByDrop" | "blueprintByDrop" | "resourceByDrop" => Layout::ByDrop,
        _ => return None,
    })
}

/// Fetch the drop-table document, following the landing redirect.
pub fn fetch(agent: &ureq::Agent) -> Result<Vec<u8>> {
    get(agent, URL)
}

/// Parse the whole drop-table document.
pub fn parse(raw: &[u8]) -> Result<Tables> {
    let html = String::from_utf8_lossy(raw);
    let doc = Html::parse_document(&html);
    let sel = Selectors::new()?;

    let mut out = Tables::default();
    for h3 in doc.select(&sel.h3) {
        let Some(section) = h3.value().attr("id") else {
            continue;
        };
        let Some(table) = next_table(h3) else {
            continue;
        };
        match (section, layout(section)) {
            ("relicRewards", _) => read_relics(&sel, table, &mut out.relics),
            (_, Some(kind)) => read(&sel, table, section, kind, &mut out.drops),
            (_, None) => out.skipped.push(section.to_string()),
        }
    }
    Ok(out)
}

struct Selectors {
    h3: Selector,
    tr: Selector,
    cell: Selector,
}

impl Selectors {
    fn new() -> Result<Self> {
        let parse = |s: &str| Selector::parse(s).map_err(|e| anyhow!("selector {s}: {e:?}"));
        Ok(Self {
            h3: parse("h3[id]")?,
            tr: parse("tr")?,
            cell: parse("th, td")?,
        })
    }
}

/// One table cell, with what the layouts need to tell rows apart.
struct Cell {
    header: bool,
    pad: bool,
    text: String,
}

/// The first element after `h3`, when it is a table.
fn next_table(h3: ElementRef<'_>) -> Option<ElementRef<'_>> {
    let next = h3.next_siblings().filter_map(ElementRef::wrap).next()?;
    (next.value().name() == "table").then_some(next)
}

fn cells(sel: &Selectors, tr: ElementRef<'_>) -> Vec<Cell> {
    tr.select(&sel.cell)
        .map(|c| Cell {
            header: c.value().name() == "th",
            pad: c
                .value()
                .attr("class")
                .is_some_and(|v| v.contains("pad-cell")),
            text: c.text().collect::<String>().trim().to_string(),
        })
        .collect()
}

/// Walk one table, emitting a drop per item row.
fn read(sel: &Selectors, table: ElementRef<'_>, section: &str, kind: Layout, out: &mut Vec<Drop>) {
    let mut place = String::new();
    let mut rotation = None;
    let mut stage = None;
    let mut table_chance = None;

    for tr in table.select(&sel.tr) {
        let row = cells(sel, tr);
        if row.iter().all(|c| c.text.is_empty()) {
            continue;
        }

        match kind {
            Layout::Place | Layout::Bounty => {
                if row.len() == 1 && row[0].header {
                    match row[0].text.strip_prefix("Rotation ") {
                        Some(letter) => rotation = Some(letter.trim().to_string()),
                        None => {
                            place = row[0].text.clone();
                            rotation = None;
                            stage = None;
                        }
                    }
                    continue;
                }
                if row.len() == 2 && row[0].pad && row[1].header {
                    stage = Some(row[1].text.clone());
                    continue;
                }
            }
            Layout::ByAvatar => {
                if row.len() == 2 && row[0].header && row[1].header {
                    place = row[0].text.clone();
                    table_chance = percent(&row[1].text);
                    continue;
                }
            }
            Layout::ByDrop => {
                if row.len() == 1 && row[0].header {
                    place = row[0].text.clone();
                    continue;
                }
                if row.iter().all(|c| c.header) {
                    continue;
                }
            }
        }

        let (item, rate, chance) = match kind {
            // `item | rarity`
            Layout::Place if row.len() == 2 => (&row[0].text, &row[1].text, table_chance),
            // `_ | item | rarity`
            Layout::Bounty | Layout::ByAvatar if row.len() == 3 => {
                (&row[1].text, &row[2].text, table_chance)
            }
            // `enemy | table chance | rarity`, with the item named by the block header
            Layout::ByDrop if row.len() == 3 => (&row[0].text, &row[2].text, percent(&row[1].text)),
            _ => continue,
        };
        let Some((rarity, drop_chance)) = rate_of(rate) else {
            continue;
        };

        // ByDrop inverts the table: the row names the place, the header names the item.
        let (item, source) = match kind {
            Layout::ByDrop => (place.clone(), item.clone()),
            _ => (item.clone(), place.clone()),
        };
        if item.is_empty() || source.is_empty() {
            continue;
        }

        out.push(Drop {
            section: section.to_string(),
            item,
            place: source,
            rotation: rotation.clone(),
            stage: stage.clone(),
            rarity,
            chance: drop_chance,
            table_chance: chance,
        });
    }
}

/// Walk the relic table, whose headers read `Axi A1 Relic (Intact)`.
fn read_relics(sel: &Selectors, table: ElementRef<'_>, out: &mut Vec<RelicRow>) {
    let mut header: Option<(String, String)> = None;
    for tr in table.select(&sel.tr) {
        let row = cells(sel, tr);
        if row.iter().all(|c| c.text.is_empty()) {
            continue;
        }
        if row.len() == 1 && row[0].header {
            header = split_grade(&row[0].text);
            continue;
        }
        let (Some((relic, refinement)), 2) = (&header, row.len()) else {
            continue;
        };
        let Some((rarity, chance)) = rate_of(&row[1].text) else {
            continue;
        };
        out.push(RelicRow {
            relic: relic.clone(),
            refinement: refinement.clone(),
            reward: row[0].text.clone(),
            rarity,
            chance,
        });
    }
}

/// `Axi A1 Relic (Intact)` -> (`Axi A1 Relic`, `intact`).
fn split_grade(text: &str) -> Option<(String, String)> {
    let (name, rest) = text.rsplit_once(" (")?;
    let grade = rest.strip_suffix(')')?;
    Some((name.to_string(), grade.to_lowercase()))
}

/// `Uncommon (12.50%)` -> (`Uncommon`, 0.125).
fn rate_of(text: &str) -> Option<(String, f64)> {
    let (rarity, rest) = text.split_once('(')?;
    let chance = percent(rest)?;
    Some((rarity.trim().to_string(), chance))
}

/// The first percentage in a string, as a fraction.
fn percent(text: &str) -> Option<f64> {
    let end = text.find('%')?;
    let start = text[..end]
        .rfind(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map_or(0, |i| i + 1);
    text[start..end].parse::<f64>().ok().map(|v| v / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(section: &str, table: &str) -> Vec<Drop> {
        let html = format!("<h3 id=\"{section}\">x</h3><table>{table}</table>");
        parse(html.as_bytes()).unwrap().drops
    }

    #[test]
    fn reads_mission_rotations() {
        let drops = one(
            "missionRewards",
            "<tr><th colspan=2>Mercury/Apollodorus (Survival)</th></tr>\
             <tr><th colspan=2>Rotation A</th></tr>\
             <tr><td>100 Endo</td><td>Common (50.00%)</td></tr>\
             <tr><th colspan=2>Rotation B</th></tr>\
             <tr><td>Lith Q3 Relic</td><td>Rare (7.69%)</td></tr>",
        );
        assert_eq!(drops.len(), 2);
        assert_eq!(drops[0].place, "Mercury/Apollodorus (Survival)");
        assert_eq!(drops[0].rotation.as_deref(), Some("A"));
        assert!((drops[0].chance - 0.5).abs() < 1e-9);
        assert_eq!(drops[1].rotation.as_deref(), Some("B"));
        assert_eq!(drops[1].item, "Lith Q3 Relic");
    }

    #[test]
    fn reads_bounty_stages() {
        let drops = one(
            "cetusRewards",
            "<tr><th colspan=3>Level 5 - 15 Cetus Bounty</th></tr>\
             <tr><th colspan=3>Rotation A</th></tr>\
             <tr><td class=\"pad-cell\"></td><th colspan=2>Stage 1</th></tr>\
             <tr><td></td><td>Redirection</td><td>Uncommon (20.00%)</td></tr>",
        );
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].place, "Level 5 - 15 Cetus Bounty");
        assert_eq!(drops[0].rotation.as_deref(), Some("A"));
        assert_eq!(drops[0].stage.as_deref(), Some("Stage 1"));
        assert_eq!(drops[0].item, "Redirection");
    }

    #[test]
    fn reads_enemy_table_chance() {
        let drops = one(
            "modByAvatar",
            "<tr><th>Scaldra Ti-92</th><th colspan=2>Mod Drop Chance: 3.00%</th></tr>\
             <tr><td></td><td>Vitality</td><td>Common (37.94%)</td></tr>",
        );
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].place, "Scaldra Ti-92");
        assert_eq!(drops[0].item, "Vitality");
        assert!((drops[0].table_chance.unwrap() - 0.03).abs() < 1e-9);
    }

    #[test]
    fn by_drop_inverts_item_and_place() {
        let drops = one(
            "modByDrop",
            "<tr><th colspan=3>Target Acquired</th></tr>\
             <tr><th>Source</th><th>Mod Drop Chance</th><th>Chance</th></tr>\
             <tr><td>Tusk Thumper Bull</td><td>15.00%</td><td>Uncommon (12.50%)</td></tr>",
        );
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].item, "Target Acquired");
        assert_eq!(drops[0].place, "Tusk Thumper Bull");
        assert!((drops[0].table_chance.unwrap() - 0.15).abs() < 1e-9);
    }

    #[test]
    fn relic_rewards_go_to_their_own_list() {
        let html = "<h3 id=\"relicRewards\">x</h3><table>\
             <tr><th colspan=2>Axi A1 Relic (Intact)</th></tr>\
             <tr><td>Nikana Prime Blueprint</td><td>Rare (2.00%)</td></tr>\
             <tr class=\"blank-row\"><td class=\"blank-row\" colspan=2></td></tr>\
             <tr><th colspan=2>Axi A1 Relic (Radiant)</th></tr>\
             <tr><td>Nikana Prime Blueprint</td><td>Rare (10.00%)</td></tr></table>";
        let tables = parse(html.as_bytes()).unwrap();
        assert!(tables.drops.is_empty());
        assert!(tables.skipped.is_empty());
        assert_eq!(tables.relics.len(), 2);
        assert_eq!(tables.relics[0].relic, "Axi A1 Relic");
        assert_eq!(tables.relics[0].refinement, "intact");
        assert_eq!(tables.relics[0].reward, "Nikana Prime Blueprint");
        assert_eq!(tables.relics[1].refinement, "radiant");
    }
}
