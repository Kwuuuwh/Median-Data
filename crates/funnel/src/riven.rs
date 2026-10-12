use std::collections::{BTreeMap, BTreeSet};

use graph::{Extra, Graph, RivenClass, RivenPool, StatForm, Weapon};

use crate::finding::{Finding, Layer};

/// How far two dispositions may sit apart before they disagree: both are printed to two
/// decimals, DE's in float32.
const DISPOSITION_SLACK: f64 = 0.01;

/// A rank-8 stat is its base × this × disposition × the layout's weight × the roll.
const RANK_EIGHT: f64 = 90.0;

/// The layout's weight for two positive stats and no negative one.
const TWO_POSITIVE: f64 = 0.99;

/// The best roll: 110% of the base.
const BEST_ROLL: f64 = 1.1;

/// Dispositions the game hands out run from one to five dots, 0.5 to 1.55.
const DISPOSITIONS: std::ops::RangeInclusive<f64> = 0.5..=1.55;

/// Check what the catalog says about the rivens weapons take.
pub fn check(graph: &Graph) -> Vec<Finding> {
    let mut out = Vec::new();
    let pooled: BTreeSet<RivenClass> = graph
        .items()
        .filter_map(|item| match &item.extra {
            Extra::RivenPool(pool) => Some(pool.class),
            _ => None,
        })
        .collect();
    let pools: BTreeMap<RivenClass, &RivenPool> = graph
        .items()
        .filter_map(|item| match &item.extra {
            Extra::RivenPool(pool) => Some((pool.class, pool)),
            _ => None,
        })
        .collect();
    for item in graph.items() {
        let Extra::Weapon(weapon) = &item.extra else {
            continue;
        };
        let Some(riven) = &weapon.riven else { continue };
        let path = item.unique_name.as_str();
        if !pooled.contains(&riven.class.value) {
            out.push(Finding::new(
                Layer::Coverage,
                "riven-class-without-pool",
                path,
                format!(
                    "takes a {} riven, and no pool of that class is in the catalog",
                    riven.class.value
                ),
            ));
        }

        for mount in &weapon.mounts {
            let Some(wiki) = mount.disposition else {
                continue;
            };
            let de = de_disposition(weapon, &mount.slot);
            if (wiki - de).abs() > DISPOSITION_SLACK {
                out.push(Finding::new(
                    Layer::Cross,
                    "riven-disposition-wiki",
                    path,
                    format!("DE gives {de}, the wiki's '{}' {wiki}", mount.name),
                ));
            }
        }
        if let Some(market) = riven.market_disposition
            && (market - weapon.disposition).abs() > DISPOSITION_SLACK
        {
            out.push(Finding::new(
                Layer::Cross,
                "riven-disposition-market",
                path,
                format!("DE gives {}, the market {market}", weapon.disposition),
            ));
        }
        if !DISPOSITIONS.contains(&weapon.disposition) {
            out.push(Finding::new(
                Layer::Outlier,
                "riven-disposition-range",
                path,
                format!(
                    "DE gives {}, outside the {}–{} the game hands out",
                    weapon.disposition,
                    DISPOSITIONS.start(),
                    DISPOSITIONS.end()
                ),
            ));
        }
        if riven.slug.is_none() {
            out.push(Finding::new(
                Layer::Coverage,
                "riven-without-market-slug",
                path,
                "takes a riven, and no market riven line names it or its weapon line".to_string(),
            ));
        }
        if let Some(pool) = pools
            .get(&riven.class.value)
            .filter(|_| ranged(riven.class.value))
        {
            out.extend(top_rolls(path, weapon.disposition, pool));
        }
        if weapon.mounts.iter().all(|m| m.attacks.is_empty()) {
            out.push(Finding::new(
                Layer::Coverage,
                "riven-weapon-without-attacks",
                path,
                format!(
                    "takes a {} riven, and the wiki lists no attack for it",
                    riven.class.value
                ),
            ));
        }
    }
    out
}

/// Whether a riven class is for guns.
fn ranged(class: RivenClass) -> bool {
    !matches!(class, RivenClass::Melee | RivenClass::Zaw)
}

/// Each stat of the pool alone at its best roll, on a riven of two positive stats: a value
/// past what any riven shows means a base or a disposition went wrong.
fn top_rolls(path: &str, disposition: f64, pool: &RivenPool) -> Vec<Finding> {
    let mut out = Vec::new();
    for stat in &pool.stats {
        let Some(base) = stat.bases.first() else {
            continue;
        };
        let top = base.abs() * RANK_EIGHT * disposition * TWO_POSITIVE * BEST_ROLL;
        let ceiling = match stat.form {
            StatForm::Percent => 5.0,
            StatForm::Multiplier => 1.0,
            StatForm::Plain => 10.0,
        };
        if top <= 0.0 || top > ceiling {
            out.push(Finding::new(
                Layer::Outlier,
                "riven-top-roll",
                path,
                format!(
                    "{} at its best comes to {top:.3}, outside (0, {ceiling}]",
                    stat.tag
                ),
            ));
        }
    }
    out
}

/// DE's disposition for the slot a wiki entry describes: a kitgun chamber carries its own
/// for the primary slot.
fn de_disposition(weapon: &Weapon, slot: &str) -> f64 {
    match (slot, weapon.disposition_primary) {
        ("Primary", Some(primary)) => primary,
        _ => weapon.disposition,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph::{NamePart, RivenStat};

    fn pool(base: f64, form: StatForm) -> RivenPool {
        let part = NamePart {
            en: String::new(),
            ru: None,
        };
        RivenPool {
            class: RivenClass::Rifle,
            stats: vec![RivenStat {
                tag: "WeaponDamageAmountMod".to_string(),
                prefix: part.clone(),
                suffix: part,
                bases: vec![base],
                reverse_symbol: false,
                form,
                shown_en: String::new(),
                shown_ru: None,
                market: None,
            }],
            challenges: Vec::new(),
            splices: Vec::new(),
        }
    }

    #[test]
    fn a_real_base_stays_under_the_ceiling() {
        assert!(top_rolls("/w", 1.55, &pool(0.018333299, StatForm::Percent)).is_empty());
    }

    #[test]
    fn a_broken_base_is_flagged() {
        assert_eq!(top_rolls("/w", 1.3, &pool(0.5, StatForm::Percent)).len(), 1);
        assert_eq!(top_rolls("/w", 1.3, &pool(0.0, StatForm::Percent)).len(), 1);
    }
}
