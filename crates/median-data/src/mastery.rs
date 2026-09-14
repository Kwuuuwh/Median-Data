use std::path::Path;

use anyhow::{Context, Result, bail};
use consensus::{Resolved, Source};
use graph::{Kind, Mastery, Taxonomy};
use serde::Deserialize;

use crate::merge::single;

/// Which items count toward the account's mastery rank, and how far each one ranks.
pub struct Policy {
    exclude: Vec<Exclude>,
    rule: Vec<Rule>,
    /// Caps that name kinds, in the order they were written.
    caps: Vec<Cap>,
    /// Top rank of every item no kinded cap covers.
    everywhere: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    exclude: Vec<Exclude>,
    #[serde(default)]
    rule: Vec<Rule>,
    #[serde(default)]
    cap: Vec<Cap>,
}

/// Items that look like they count and do not.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Exclude {
    reason: String,
    path_contains: Vec<String>,
}

/// Items of these kinds count on one table; with paths, only those whose path names one.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    mastery: Mastery,
    reason: String,
    kind: Vec<String>,
    #[serde(default)]
    path_contains: Vec<String>,
}

/// Top rank of items of these kinds where DE prints none; no kinds covers every item.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cap {
    rank: i64,
    reason: String,
    #[serde(default)]
    kind: Vec<String>,
}

impl Policy {
    /// The table an item's ranks count on, and nothing when it gives no mastery.
    pub fn judge(&self, path: &str, kind: &Kind) -> Option<Mastery> {
        if self
            .exclude
            .iter()
            .any(|e| contains(&e.path_contains, path))
        {
            return None;
        }
        self.rule
            .iter()
            .find(|r| {
                names(&r.kind, kind)
                    && (r.path_contains.is_empty() || contains(&r.path_contains, path))
            })
            .map(|r| r.mastery)
    }

    /// How far an item ranks: DE's own figure where it prints one, else the cap for its kind.
    pub fn cap(&self, kind: &Kind, de: Option<i64>) -> Resolved<i64> {
        match de {
            Some(rank) => single(Source::De, rank),
            None => single(
                Source::Rule,
                self.caps
                    .iter()
                    .find(|c| names(&c.kind, kind))
                    .map_or(self.everywhere, |c| c.rank),
            ),
        }
    }
}

/// Read the policy. Every kind it names has to be declared in the taxonomy.
pub fn load(path: &Path, tree: &Taxonomy) -> Result<Policy> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    parse(&text, tree).with_context(|| format!("parse {}", path.display()))
}

fn parse(text: &str, tree: &Taxonomy) -> Result<Policy> {
    let file: File = toml::from_str(text)?;
    let reasons = file
        .exclude
        .iter()
        .map(|e| &e.reason)
        .chain(file.rule.iter().map(|r| &r.reason))
        .chain(file.cap.iter().map(|c| &c.reason));
    for reason in reasons {
        if reason.trim().is_empty() {
            bail!("every exclusion, rule and cap has to say why");
        }
    }
    for rule in &file.rule {
        if rule.kind.is_empty() {
            bail!("rule '{}' names no kind", rule.reason);
        }
    }
    let kinds = file
        .rule
        .iter()
        .flat_map(|r| &r.kind)
        .chain(file.cap.iter().flat_map(|c| &c.kind));
    for kind in kinds {
        if !tree.has(kind) {
            bail!("'{kind}' is a kind no class declares");
        }
    }

    let mut caps = file.cap;
    let everywhere = match caps.pop() {
        Some(last) if last.kind.is_empty() => last.rank,
        _ => bail!("the last cap has to name no kind, so every item has a top rank"),
    };
    if caps.iter().any(|c| c.kind.is_empty()) {
        bail!("only the last cap may name no kind");
    }

    Ok(Policy {
        exclude: file.exclude,
        rule: file.rule,
        caps,
        everywhere,
    })
}

fn names(kinds: &[String], kind: &Kind) -> bool {
    kinds.iter().any(|k| k == kind.as_str())
}

fn contains(parts: &[String], path: &str) -> bool {
    parts.iter().any(|p| path.contains(p.as_str()))
}

#[cfg(test)]
mod tests {
    use graph::{Class, Leaf};

    use super::*;

    fn tree() -> Taxonomy {
        let class = |slug: &str, kinds: &[&str]| Class {
            slug: slug.to_string(),
            en: slug.to_string(),
            ru: slug.to_string(),
            sourced: true,
            kind: kinds
                .iter()
                .map(|k| Leaf {
                    slug: k.to_string(),
                    en: k.to_string(),
                    ru: k.to_string(),
                })
                .collect(),
        };
        Taxonomy::new(vec![
            class("warframe", &["warframe", "necramech"]),
            class("primary", &["primary"]),
            class("modular", &["zaw-part"]),
            class("exalted", &["exalted-weapon"]),
        ])
        .unwrap()
    }

    const POLICY: &str = r#"
[[exclude]]
reason = "a Conclave copy"
path_contains = ["PvPVariant"]

[[rule]]
mastery = "carried"
reason = "frames"
kind = ["warframe", "necramech"]

[[rule]]
mastery = "wielded"
reason = "guns"
kind = ["primary"]

[[rule]]
mastery = "wielded"
reason = "a zaw is its strike"
kind = ["zaw-part"]
path_contains = ["/Tip/"]

[[cap]]
rank = 40
reason = "mechs"
kind = ["necramech"]

[[cap]]
rank = 30
reason = "everything else"
"#;

    fn policy() -> Policy {
        parse(POLICY, &tree()).unwrap()
    }

    #[test]
    fn a_warframe_counts_on_the_carried_table() {
        let verdict = policy().judge("/Lotus/Powersuits/Volt/Volt", &Kind::new("warframe"));
        assert_eq!(verdict, Some(Mastery::Carried));
    }

    #[test]
    fn a_zaw_counts_on_its_strike_alone() {
        let policy = policy();
        let zaw = Kind::new("zaw-part");
        let strike = "/Lotus/Weapons/Ostron/Melee/ModularMelee01/Tip/TipOne";
        let handle = "/Lotus/Weapons/Ostron/Melee/ModularMelee01/Handle/HandleOne";
        assert_eq!(policy.judge(strike, &zaw), Some(Mastery::Wielded));
        assert_eq!(policy.judge(handle, &zaw), None);
    }

    #[test]
    fn an_exclusion_outranks_every_rule() {
        let path = "/Lotus/Weapons/Ostron/Melee/ModularMelee01/Tip/PvPVariantTipOne";
        assert_eq!(policy().judge(path, &Kind::new("zaw-part")), None);
    }

    #[test]
    fn a_kind_no_rule_names_gives_no_mastery() {
        let path = "/Lotus/Powersuits/Excalibur/ExcaliburSwordWeapon";
        assert_eq!(policy().judge(path, &Kind::new("exalted-weapon")), None);
    }

    #[test]
    fn des_own_cap_wins_over_every_rule() {
        let cap = policy().cap(&Kind::new("primary"), Some(40));
        assert_eq!((cap.value, cap.winner), (40, Source::De));
    }

    #[test]
    fn without_des_cap_the_rule_for_the_kind_decides() {
        let policy = policy();
        let mech = policy.cap(&Kind::new("necramech"), None);
        let gun = policy.cap(&Kind::new("primary"), None);
        assert_eq!((mech.value, mech.winner), (40, Source::Rule));
        assert_eq!((gun.value, gun.winner), (30, Source::Rule));
    }

    #[test]
    fn a_rule_naming_an_undeclared_kind_is_refused() {
        let text = POLICY.replace(r#"kind = ["primary"]"#, r#"kind = ["pistol"]"#);
        assert!(parse(&text, &tree()).is_err());
    }

    #[test]
    fn a_policy_without_a_catch_all_cap_is_refused() {
        let text = POLICY.replace(
            "rank = 30\nreason = \"everything else\"",
            "rank = 30\nreason = \"everything else\"\nkind = [\"primary\"]",
        );
        assert!(parse(&text, &tree()).is_err());
    }
}
