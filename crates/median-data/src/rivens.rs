use std::collections::BTreeMap;

use consensus::{Claim, Conflict, Source, claims, resolve};
use graph::{Extra, Graph, Item, Mount, Node, Riven, RivenClass, Weapon};
use sources::wfm::RivenWeapon;

/// The wiki outranks the market: the market files every archgun under `rifle`.
const CLASS: &[Source] = &[Source::Wiki, Source::Wfm];

/// What working out the rivens weapons take came to.
#[derive(Default)]
pub struct Rivened {
    pub weapons: usize,
    /// Market riven lines whose weapon is not in the catalog, by slug, with that weapon.
    pub unknown: Vec<(String, String)>,
    /// What a weapon's wiki entries say about its riven that cannot be read, by weapon.
    pub unread: Vec<(String, String)>,
}

/// Give every weapon a riven fits its riven class, weapon line and market slug.
pub fn judge(graph: &mut Graph, market: &[RivenWeapon], conflicts: &mut Vec<Conflict>) -> Rivened {
    let mut out = Rivened::default();
    let by_ref: BTreeMap<&str, &RivenWeapon> =
        market.iter().map(|w| (w.game_ref.as_str(), w)).collect();
    for w in market {
        let known = matches!(graph.get(&w.game_ref), Some(Node::Item(i)) if matches!(i.extra, Extra::Weapon(_)));
        if !known {
            out.unknown.push((w.slug.clone(), w.game_ref.clone()));
        }
    }

    let mut judged = Vec::new();
    for item in graph.items() {
        let Extra::Weapon(weapon) = &item.extra else {
            continue;
        };
        let listed = by_ref.get(item.unique_name.as_str()).copied();
        let mut fits = Vec::new();
        for mount in &weapon.mounts {
            match fit(item.kind.value.as_str(), mount) {
                Fit::Riven(class) if !fits.contains(&class) => fits.push(class),
                Fit::Unknown => out.unread.push((
                    item.unique_name.clone(),
                    format!(
                        "entry '{}' is a {} {}, a riven class nothing names",
                        mount.name, mount.slot, mount.class
                    ),
                )),
                _ => {}
            }
        }
        if fits.len() > 1 {
            let named: Vec<&str> = fits.iter().map(|c| c.as_str()).collect();
            out.unread.push((
                item.unique_name.clone(),
                format!("entries fit several riven classes: {}", named.join(", ")),
            ));
        }
        let mut all = Vec::new();
        if let Some(&class) = fits.first() {
            all.push(Claim {
                source: Source::Wiki,
                value: class,
            });
        }
        if let Some(w) = listed {
            match market_class(w) {
                Some(class) => all.push(Claim {
                    source: Source::Wfm,
                    value: class,
                }),
                None => out.unread.push((
                    item.unique_name.clone(),
                    format!(
                        "the market files it as a {} riven, a class nothing names",
                        w.riven_type
                    ),
                )),
            }
        }
        let Some(class) = resolve(&all, CLASS) else {
            continue;
        };
        if class.status == consensus::Status::Conflict {
            conflicts.push(Conflict::new(
                &item.unique_name,
                "riven_class",
                claims(&[
                    (Source::Wiki, fits.first().copied()),
                    (Source::Wfm, listed.and_then(market_class)),
                ]),
                class.value.to_string(),
            ));
        }
        let family = family(graph, &item.unique_name, weapon);
        judged.push((
            item.unique_name.clone(),
            Riven {
                class,
                family,
                slug: None,
                market_disposition: listed.map(|w| w.disposition),
            },
        ));
    }

    // A weapon line shares one riven, and the market lists the line under one of its weapons.
    let mut lines: BTreeMap<(String, RivenClass), String> = BTreeMap::new();
    for (path, riven) in &judged {
        if let (Some(w), Some(family)) = (by_ref.get(path.as_str()), &riven.family) {
            lines.insert((family.clone(), riven.class.value), w.slug.clone());
        }
    }
    for (path, riven) in &mut judged {
        riven.slug = match by_ref.get(path.as_str()) {
            Some(w) => Some(w.slug.clone()),
            None => riven
                .family
                .as_ref()
                .and_then(|family| lines.get(&(family.clone(), riven.class.value)).cloned()),
        };
    }

    for (path, riven) in judged {
        if let Some(Node::Item(item)) = graph.get_mut(&path)
            && let Extra::Weapon(weapon) = &mut item.extra
        {
            weapon.riven = Some(riven);
            out.weapons += 1;
        }
    }
    out
}

/// The weapon line a weapon belongs to. DE's Conclave copy of a weapon has no wiki entry of
/// its own and belongs to the line of the weapon it copies.
fn family(graph: &Graph, path: &str, weapon: &Weapon) -> Option<String> {
    let own = |w: &Weapon| w.mounts.iter().find_map(|m| m.family.clone());
    own(weapon).or_else(|| match graph.get(&path.replace("PvPVariant", ""))? {
        Node::Item(Item {
            extra: Extra::Weapon(copied),
            ..
        }) => own(copied),
        _ => None,
    })
}

/// What one wiki entry says about the riven its weapon takes.
#[derive(Debug, PartialEq)]
enum Fit {
    Riven(RivenClass),
    /// The entry is of a kind no riven exists for: amps, arch-melee, exalted weapons, claws.
    NoRiven,
    /// A slot rivens exist for, with a weapon class no rule here names.
    Unknown,
}

/// `kind` is the weapon's place in our taxonomy, which knows kitgun and zaw parts.
fn fit(kind: &str, mount: &Mount) -> Fit {
    match kind {
        "kitgun-part" => return Fit::Riven(RivenClass::Kitgun),
        "zaw-part" => return Fit::Riven(RivenClass::Zaw),
        "exalted-weapon" => return Fit::NoRiven,
        _ => {}
    }
    let class = mount.class.as_str();
    if class == "Exalted Weapon" {
        return Fit::NoRiven;
    }
    let riven = match (mount.slot.as_str(), class) {
        ("Primary", "Shotgun") => RivenClass::Shotgun,
        ("Primary", "Rifle" | "Bow" | "Crossbow" | "Launcher" | "Sniper Rifle" | "Speargun") => {
            RivenClass::Rifle
        }
        ("Secondary", "Shotgun Sidearm" | "Dual Shotguns") => RivenClass::Shotgun,
        ("Secondary", "Pistol" | "Dual Pistols" | "Crossbow" | "Thrown" | "Tome") => {
            RivenClass::Pistol
        }
        ("Melee", _) => RivenClass::Melee,
        ("Archgun" | "Archgun (Atmosphere)", _) => RivenClass::Archgun,
        ("Robotic" | "Hound", "Rifle" | "Sniper Rifle") => RivenClass::Rifle,
        ("Robotic" | "Hound", "Shotgun") => RivenClass::Shotgun,
        ("Robotic" | "Hound", "Pistol") => RivenClass::Pistol,
        ("Robotic" | "Hound", "Melee" | "Glaive") => RivenClass::Melee,
        ("Amp" | "Archmelee" | "Beast", _) => return Fit::NoRiven,
        _ => return Fit::Unknown,
    };
    Fit::Riven(riven)
}

/// The riven class a market line names. The market files archguns as `rifle` under the
/// `archgun` group.
fn market_class(w: &RivenWeapon) -> Option<RivenClass> {
    if w.group == "archgun" {
        return Some(RivenClass::Archgun);
    }
    match w.riven_type.as_str() {
        "rifle" => Some(RivenClass::Rifle),
        "shotgun" => Some(RivenClass::Shotgun),
        "pistol" => Some(RivenClass::Pistol),
        "melee" => Some(RivenClass::Melee),
        "kitgun" => Some(RivenClass::Kitgun),
        "zaw" => Some(RivenClass::Zaw),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mount(slot: &str, class: &str) -> Mount {
        Mount {
            name: "X".to_string(),
            slot: slot.to_string(),
            class: class.to_string(),
            family: None,
            traits: Vec::new(),
            disposition: None,
            ammo_max: None,
            tooltip_attack: None,
            attacks: Vec::new(),
        }
    }

    #[test]
    fn a_wiki_entry_names_the_riven_its_weapon_takes() {
        let cases = [
            (
                "primary",
                "Primary",
                "Shotgun",
                Fit::Riven(RivenClass::Shotgun),
            ),
            ("primary", "Primary", "Bow", Fit::Riven(RivenClass::Rifle)),
            (
                "secondary",
                "Secondary",
                "Shotgun Sidearm",
                Fit::Riven(RivenClass::Shotgun),
            ),
            (
                "secondary",
                "Secondary",
                "Thrown",
                Fit::Riven(RivenClass::Pistol),
            ),
            (
                "kitgun-part",
                "Primary",
                "Shotgun",
                Fit::Riven(RivenClass::Kitgun),
            ),
            (
                "zaw-part",
                "Melee",
                "Zaw Sword / Polearm",
                Fit::Riven(RivenClass::Zaw),
            ),
            (
                "arch-gun",
                "Archgun (Atmosphere)",
                "Rifle",
                Fit::Riven(RivenClass::Archgun),
            ),
            (
                "companion-weapon",
                "Robotic",
                "Glaive",
                Fit::Riven(RivenClass::Melee),
            ),
            ("exalted-weapon", "Melee", "Exalted Weapon", Fit::NoRiven),
            ("amp", "Amp", "Amp", Fit::NoRiven),
            ("primary", "Primary", "Flamethrower", Fit::Unknown),
        ];
        for (kind, slot, class, want) in cases {
            assert_eq!(fit(kind, &mount(slot, class)), want, "{slot} {class}");
        }
    }

    #[test]
    fn the_market_files_archguns_under_their_own_group() {
        let line = RivenWeapon {
            slug: "imperator".to_string(),
            game_ref: "/x".to_string(),
            group: "archgun".to_string(),
            riven_type: "rifle".to_string(),
            disposition: 1.0,
        };
        assert_eq!(market_class(&line), Some(RivenClass::Archgun));
    }
}
