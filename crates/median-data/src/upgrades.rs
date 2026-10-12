use graph::{ArcaneInfo, Extra, Graph, Line, ModInfo, Node, RankText};

use crate::effects;
use crate::extract::{DeArcane, DeMod};

/// `compatName`s that name a class of weapon rather than one weapon.
const CLASSES: [&str; 8] = [
    "Rifle",
    "Shotgun",
    "Pistol",
    "Assault Rifle",
    "Sniper",
    "Bow",
    "PRIMARY",
    "SECONDARY",
];

/// Arcanes for guns open their name with the slot they fit.
const GUN_ARCANES: [&str; 2] = ["Primary ", "Secondary "];

/// What hanging mods and arcanes on their items came to.
#[derive(Default)]
pub struct Read {
    pub mods: usize,
    pub arcanes: usize,
    /// Lines of a mod or arcane a gun build could take that no rule reads, by item.
    pub unread: Vec<(String, String)>,
}

/// Give every gun mod and every arcane its top rank, read into effects.
pub fn link(graph: &mut Graph, mods: Vec<DeMod>, arcanes: Vec<DeArcane>) -> Read {
    let mut out = Read::default();
    for m in mods {
        let augment = !CLASSES.contains(&m.compat.as_str());
        let pvp = m.unique_name.contains("/PvPMods/");
        let top_rank = rank(&m.top_en, &m.top_ru);
        if !augment && !pvp {
            out.unread
                .extend(unread(&m.top_en).map(|line| (m.unique_name.clone(), line)));
        }
        let info = ModInfo {
            primed: m.name.starts_with("Primed "),
            umbral: m.polarity == "AP_UMBRA",
            augment,
            pvp,
            exilus: m.exilus,
            compat: m.compat,
            rarity: m.rarity,
            polarity: m.polarity,
            base_drain: m.base_drain,
            max_rank: m.max_rank,
            mod_set: m.mod_set,
            top_rank,
        };
        if let Some(Node::Item(item)) = graph.get_mut(&m.unique_name) {
            item.extra = Extra::Mod(Box::new(info));
            out.mods += 1;
        }
    }
    for a in arcanes {
        let Some(Node::Item(item)) = graph.get_mut(&a.unique_name) else {
            continue;
        };
        if GUN_ARCANES
            .iter()
            .any(|slot| item.names.en.value.starts_with(slot))
        {
            out.unread
                .extend(unread(&a.top_en).map(|line| (a.unique_name.clone(), line)));
        }
        item.extra = Extra::Arcane(ArcaneInfo {
            rarity: a.rarity,
            max_rank: a.max_rank,
            top_rank: rank(&a.top_en, &a.top_ru),
        });
        out.arcanes += 1;
    }
    out
}

/// One rank: its lines paired across the two languages, and the effects the English holds.
fn rank(en: &[String], ru: &[String]) -> RankText {
    let lines = en
        .iter()
        .enumerate()
        .map(|(at, english)| Line {
            en: english.replace("\r\n", "\n"),
            ru: ru.get(at).map(|russian| russian.replace("\r\n", "\n")),
        })
        .collect();
    RankText {
        lines,
        effects: effects::read(en).effects,
    }
}

/// Lines no rule reads, each once.
fn unread(en: &[String]) -> impl Iterator<Item = String> {
    let mut lines = effects::read(en).unread;
    lines.sort();
    lines.dedup();
    lines.into_iter()
}
