use std::collections::BTreeSet;

use graph::{Effect, Unit};

/// What one rank's lines came to.
#[derive(Debug, Default, PartialEq)]
pub struct Read {
    pub effects: Vec<Effect>,
    /// Lines no rule here reads, as printed.
    pub unread: Vec<String>,
}

/// Read the effects out of one rank of `levelStats`. A line ending in a colon is a condition
/// for the lines under it in the same entry. At the top rank DE repeats an unconditional
/// entry inside the conditional one above it, so a line that also stands as an entry of its
/// own is read once, as that entry.
pub fn read(stats: &[String]) -> Read {
    let standalone: BTreeSet<&str> = stats
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.contains('\n'))
        .collect();
    let mut out = Read::default();
    for entry in stats {
        let lines: Vec<&str> = entry.lines().collect();
        let mut condition: Option<String> = None;
        for raw in &lines {
            let line = raw.trim().trim_start_matches("<LOWER_IS_BETTER>");
            if line.is_empty() || line == "<LINE_SEPARATOR>" {
                continue;
            }
            if let Some(header) = line.strip_suffix(':') {
                condition = Some(header.trim().to_string());
                continue;
            }
            if condition.is_some() && lines.len() > 1 && standalone.contains(line) {
                continue;
            }
            match effects(line, condition.as_deref()) {
                Some(read) => out.effects.extend(read),
                None => out.unread.push(line.to_string()),
            }
        }
    }
    out
}

/// One line read as its effects. Two stats joined by `and` share what the second one says
/// about when and how long: `+3% Critical Damage and +2.25% Multishot for 12s`.
fn effects(line: &str, condition: Option<&str>) -> Option<Vec<Effect>> {
    let joined = [" and +", " and -"]
        .iter()
        .filter_map(|joint| line.find(joint))
        .min();
    let Some(at) = joined else {
        return effect(line, condition).map(|e| vec![e]);
    };
    let second = effect(&line[at + " and ".len()..], condition)?;
    let first = effect(&line[..at], condition)?;
    let first = Effect {
        condition: first.condition.or_else(|| second.condition.clone()),
        chance: first.chance.or(second.chance),
        duration_secs: first.duration_secs.or(second.duration_secs),
        max_stacks: first.max_stacks.or(second.max_stacks),
        ..first
    };
    Some(vec![first, second])
}

/// One line read as an effect, or nothing where the line says something else.
fn effect(line: &str, condition: Option<&str>) -> Option<Effect> {
    let mut rest = line;
    let mut chance = None;
    if let Some((amount, after)) = number(rest)
        && let Some(after) = after.strip_prefix("% chance for ")
    {
        chance = Some(amount);
        rest = after;
    }

    if let Some(after) = rest.strip_prefix('x') {
        let (amount, after) = number(after)?;
        let faction = after.strip_prefix(" Damage to ")?;
        return Some(Effect {
            stat: faction_stat(faction)?,
            amount,
            unit: Unit::Multiplier,
            condition: condition.map(str::to_string),
            chance,
            duration_secs: None,
            max_stacks: None,
        });
    }

    let (amount, after) = signed(rest)?;
    let (unit, after) = match after.chars().next()? {
        '%' => (Unit::Percent, &after[1..]),
        'm' => (Unit::Meters, &after[1..]),
        's' => (Unit::Seconds, &after[1..]),
        _ => (Unit::Plain, after),
    };
    let mut phrase = after.strip_prefix(' ')?;

    let mut max_stacks = None;
    if let Some((head, stacks)) = phrase.rsplit_once(". Stacks up to ") {
        max_stacks = Some(stacks.strip_suffix("x.")?.parse().ok()?);
        phrase = head;
    }
    let mut duration_secs = None;
    if let Some((head, secs)) = phrase.rsplit_once(" for ")
        && let Some(secs) = secs.strip_suffix('s')
        && let Some((secs, "")) = number(secs)
    {
        duration_secs = Some(secs);
        phrase = head;
    }
    if let Some((head, bows)) = phrase.rsplit_once(" (x")
        && bows.ends_with(" for Bows)")
    {
        phrase = head;
    }
    let mut condition = condition.map(str::to_string);
    if stat(phrase).is_none()
        && let Some((head, joint, state)) = [" when ", " while "]
            .iter()
            .find_map(|joint| phrase.rsplit_once(joint).map(|(h, s)| (h, joint, s)))
    {
        condition.get_or_insert_with(|| format!("{} {state}", capitalised(joint.trim())));
        phrase = head;
    }

    Some(Effect {
        stat: stat(phrase)?.to_string(),
        amount,
        unit,
        condition,
        chance,
        duration_secs,
        max_stacks,
    })
}

/// Our key for a stat the client names.
fn stat(phrase: &str) -> Option<&'static str> {
    if let Some(element) = phrase.strip_prefix("<DT_") {
        let (tag, _) = element.split_once("_COLOR>")?;
        return damage_type(tag);
    }
    let key = match phrase {
        "Damage" => "damage",
        "Multishot" => "multishot",
        "Critical Chance" => "crit_chance",
        "Critical Damage" => "crit_damage",
        "Status Chance" => "status_chance",
        "Status Duration" => "status_duration",
        "Status Damage" => "status_damage",
        "Fire Rate" => "fire_rate",
        "Reload Speed" => "reload_speed",
        "Magazine Capacity" => "magazine",
        "Ammo Maximum" => "ammo_max",
        "Ammo Efficiency" => "ammo_efficiency",
        "Weapon Recoil" => "recoil",
        "Punch Through" => "punch_through",
        "Projectile Speed" => "projectile_speed",
        "Accuracy" => "accuracy",
        "Zoom" => "zoom",
        "Weak Point Damage" => "weak_point_damage",
        "Weak Point Critical Chance" => "weak_point_crit_chance",
        "to Headshot Multiplier" => "headshot_multiplier",
        "Headshot Damage" => "headshot_damage",
        "Bodyshot Damage" => "bodyshot_damage",
        "Damage on first shot in Magazine" => "first_shot_damage",
        "Direct Damage per Status Type affecting the target" => "damage_per_status",
        "Blast Radius" | "Blast Range" => "blast_radius",
        "Energy" => "energy",
        "Beam Range" => "beam_range",
        "Magazine Reloaded/s when Holstered" => "holster_reload",
        "Combo Duration" => "combo_duration",
        "Movement Speed" => "movement_speed",
        _ => return None,
    };
    Some(key)
}

/// A word with its first letter raised: `when` -> `When`.
fn capitalised(state: &str) -> String {
    let mut words = state.chars();
    words.next().map_or_else(String::new, |first| {
        format!("{}{}", first.to_uppercase(), words.as_str())
    })
}

/// Our key for a damage type the client colours by its tag.
fn damage_type(tag: &str) -> Option<&'static str> {
    let key = match tag {
        "IMPACT" => "impact",
        "PUNCTURE" => "puncture",
        "SLASH" => "slash",
        "FIRE" => "heat",
        "FREEZE" => "cold",
        "ELECTRICITY" => "electricity",
        "POISON" => "toxin",
        "EXPLOSION" => "blast",
        "RADIATION" => "radiation",
        "GAS" => "gas",
        "MAGNETIC" => "magnetic",
        "VIRAL" => "viral",
        "CORROSIVE" => "corrosive",
        _ => return None,
    };
    Some(key)
}

/// Our key for damage to a faction.
fn faction_stat(faction: &str) -> Option<String> {
    matches!(
        faction,
        "Grineer" | "Corpus" | "Infested" | "Orokin" | "Murmur"
    )
    .then(|| format!("faction_{}", faction.to_lowercase()))
}

/// A number with its sign, and what follows it.
fn signed(text: &str) -> Option<(f64, &str)> {
    match text.strip_prefix('-') {
        Some(rest) => number(rest).map(|(n, after)| (-n, after)),
        None => number(text.strip_prefix('+').unwrap_or(text)),
    }
}

/// A number at the start of the text, thousands separated by commas, and what follows it.
fn number(text: &str) -> Option<(f64, &str)> {
    let end = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .unwrap_or(text.len());
    let digits = text[..end].trim_end_matches(['.', ',']);
    if digits.is_empty() {
        return None;
    }
    let value = digits.replace(',', "").parse().ok()?;
    Some((value, &text[digits.len()..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(stats: &[&str]) -> Vec<String> {
        stats.iter().map(|s| s.to_string()).collect()
    }

    fn plain(stat: &str, amount: f64, unit: Unit) -> Effect {
        Effect {
            stat: stat.to_string(),
            amount,
            unit,
            condition: None,
            chance: None,
            duration_secs: None,
            max_stacks: None,
        }
    }

    #[test]
    fn reads_the_plain_mods() {
        assert_eq!(
            read(&lines(&["+165% Damage"])).effects,
            vec![plain("damage", 165.0, Unit::Percent)]
        );
        assert_eq!(
            read(&lines(&["+150% Critical Chance"])).effects,
            vec![plain("crit_chance", 150.0, Unit::Percent)]
        );
        assert_eq!(
            read(&lines(&[
                "+55% Fire Rate (x2 for Bows)",
                "+2.2 Punch Through"
            ]))
            .effects,
            vec![
                plain("fire_rate", 55.0, Unit::Percent),
                plain("punch_through", 2.2, Unit::Plain)
            ]
        );
        assert_eq!(
            read(&lines(&["-60% Weapon Recoil", "x1.3 Damage to Grineer"])).effects,
            vec![
                plain("recoil", -60.0, Unit::Percent),
                plain("faction_grineer", 1.3, Unit::Multiplier)
            ]
        );
        assert_eq!(
            read(&lines(&["+90% <DT_POISON_COLOR>Toxin"])).effects,
            vec![plain("toxin", 90.0, Unit::Percent)]
        );
    }

    #[test]
    fn reads_a_condition_with_its_duration_and_stacks() {
        let read = read(&lines(&[
            "+80% Multishot",
            "On Kill:\r\n+30% Multishot for 20s. Stacks up to 5x.",
        ]));
        assert_eq!(read.effects[0], plain("multishot", 80.0, Unit::Percent));
        assert_eq!(
            read.effects[1],
            Effect {
                condition: Some("On Kill".to_string()),
                duration_secs: Some(20.0),
                max_stacks: Some(5),
                ..plain("multishot", 30.0, Unit::Percent)
            }
        );
    }

    #[test]
    fn an_entry_repeated_under_a_condition_counts_once() {
        let read = read(&lines(&[
            "On Kill:\r\n+30% Damage for 4s. Stacks up to 12x.\r\n+30% Reload Speed",
            "+30% Reload Speed",
        ]));
        assert_eq!(read.effects.len(), 2);
        assert_eq!(read.effects[1], plain("reload_speed", 30.0, Unit::Percent));
    }

    #[test]
    fn reads_a_chance_and_an_aim() {
        let read = read(&lines(&[
            "On Headshot:\r\n10% chance for +60% Damage for 16s",
            "+30% Critical Chance when Aiming for 3s",
        ]));
        assert_eq!(read.effects[0].chance, Some(10.0));
        assert_eq!(read.effects[0].condition.as_deref(), Some("On Headshot"));
        assert_eq!(read.effects[1].condition.as_deref(), Some("When Aiming"));
        assert_eq!(read.effects[1].duration_secs, Some(3.0));
    }

    #[test]
    fn two_stats_share_a_duration() {
        let read = read(&lines(&[
            "+3% Critical Damage and +2.25% Multishot for 12s. Stacks up to 40x.",
            "+330% Damage while Invisible",
        ]));
        assert_eq!(read.effects.len(), 3);
        assert_eq!(read.effects[0].stat, "crit_damage");
        assert_eq!(
            (read.effects[0].duration_secs, read.effects[0].max_stacks),
            (Some(12.0), Some(40))
        );
        assert_eq!(
            read.effects[2].condition.as_deref(),
            Some("While Invisible")
        );
    }

    #[test]
    fn prose_stays_unread() {
        let read = read(&lines(&["Grenades have 100% chance to stick to surfaces."]));
        assert!(read.effects.is_empty());
        assert_eq!(read.unread.len(), 1);
    }
}
