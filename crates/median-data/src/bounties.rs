use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use graph::Bounty;
use serde::Deserialize;

/// Settlements and the Russian labels for special bounties.
#[derive(Debug, Default, Deserialize)]
pub struct Settlements {
    #[serde(default)]
    settlements: BTreeMap<String, Settlement>,
    #[serde(default)]
    activity: Vec<Activity>,
}

/// Where a family of bounties is handed out, and by whom.
#[derive(Debug, Deserialize)]
struct Settlement {
    name_en: String,
    name_ru: Option<String>,
    giver_en: Option<String>,
    giver_ru: Option<String>,
}

/// Russian for one printed bounty label.
#[derive(Debug, Deserialize)]
struct Activity {
    label: String,
    ru: String,
}

/// Read the settlements. A missing file leaves bounty places as bare printed names.
pub fn load(path: &Path) -> Result<Settlements> {
    if !path.exists() {
        return Ok(Settlements::default());
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

impl Settlements {
    /// What a bounty table says about itself: its level range and label come from the printed
    /// name, its settlement and giver from the section the drop tables filed it under.
    pub fn read(&self, section: &str, printed: &str) -> Option<Bounty> {
        let place = self.settlements.get(section)?;
        let (min_level, max_level, activity) = levels(printed);
        Some(Bounty {
            settlement: place.name_en.clone(),
            settlement_ru: place.name_ru.clone(),
            giver: place.giver_en.clone().filter(|g| !g.is_empty()),
            giver_ru: place.giver_ru.clone().filter(|g| !g.is_empty()),
            min_level,
            max_level,
            activity_ru: self
                .activity
                .iter()
                .find(|a| a.label.eq_ignore_ascii_case(&activity))
                .map(|a| a.ru.clone()),
            activity,
        })
    }
}

/// Split `Level 5 - 15 Cetus Bounty` into its range and the label after it. A table without a
/// printed range keeps zeroes and the whole name as its label.
fn levels(printed: &str) -> (i64, i64, String) {
    let rest = match printed.trim().strip_prefix("Level") {
        Some(rest) => rest.trim_start(),
        None => return (0, 0, printed.trim().to_string()),
    };
    let mut parts = rest.splitn(3, char::is_whitespace).map(str::trim);
    let (Some(min), Some(dash), Some(tail)) = (parts.next(), parts.next(), parts.next()) else {
        return (0, 0, printed.trim().to_string());
    };
    let (Ok(min), "-", Some((max, label))) = (
        min.parse::<i64>(),
        dash,
        tail.trim_start().split_once(char::is_whitespace),
    ) else {
        return (0, 0, printed.trim().to_string());
    };
    match max.parse::<i64>() {
        Ok(max) => (min, max, label.trim().to_string()),
        Err(_) => (0, 0, printed.trim().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settlements() -> Settlements {
        toml::from_str(
            r#"
[settlements.cetusRewards]
name_en = "Cetus"
name_ru = "Цетус"
giver_en = "Konzu"
giver_ru = "Конзу"

[settlements.hexRewards]
name_en = "Höllvania"

[[activity]]
label = "Ghoul Bounty"
ru = "Гули"
"#,
        )
        .unwrap()
    }

    #[test]
    fn reads_range_and_label() {
        assert_eq!(
            levels("Level 5 - 15 Cetus Bounty"),
            (5, 15, "Cetus Bounty".to_string())
        );
        // the tables print a double space before the range on some rows
        assert_eq!(
            levels("Level  105 - 110 WF1999 Bounty"),
            (105, 110, "WF1999 Bounty".to_string())
        );
    }

    #[test]
    fn a_table_without_a_range_keeps_its_whole_name() {
        assert_eq!(
            levels("First Completion"),
            (0, 0, "First Completion".to_string())
        );
    }

    #[test]
    fn fills_settlement_giver_and_russian_label() {
        let s = settlements();
        let b = s
            .read("cetusRewards", "Level 15 - 25 Ghoul Bounty")
            .unwrap();
        assert_eq!(b.settlement, "Cetus");
        assert_eq!(b.giver.as_deref(), Some("Konzu"));
        assert_eq!((b.min_level, b.max_level), (15, 25));
        assert_eq!(b.activity_ru.as_deref(), Some("Гули"));
    }

    #[test]
    fn a_settlement_with_no_giver_says_so_rather_than_inventing_one() {
        let s = settlements();
        let b = s.read("hexRewards", "Level 55 - 60 WF1999 Bounty").unwrap();
        assert_eq!(b.settlement, "Höllvania");
        assert_eq!(b.giver, None);
        assert_eq!(b.activity_ru, None);
    }

    #[test]
    fn a_section_nobody_described_yields_nothing() {
        assert!(settlements().read("sortieRewards", "Sortie").is_none());
    }
}
