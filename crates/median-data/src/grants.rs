use graph::{Edge, Graph, Rel};
use serde::{Deserialize, Serialize};
use sources::notation::Value;

use crate::lineage::{Lineage, resolve};
use crate::normalize;

const STORE: &str = "/Lotus/StoreItems/";

/// What the game hands over along with an item, in the game's own paths.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Grants {
    #[serde(default)]
    pub grant: Vec<Grant>,
}

/// Something the game hands over along with an item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Grant {
    pub owner: String,
    pub item: String,
    pub how: How,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum How {
    /// The weapon a companion fights with from the start.
    Weapon,
    /// Listed on the store item as coming with it.
    Bundled,
}

/// Every grant the types describe, in path order.
pub fn read(lineage: &Lineage) -> Grants {
    let mut grants: Vec<Grant> = lineage
        .paths()
        .flat_map(|path| {
            let mut found = weapons(lineage, path);
            found.extend(bundled(lineage, path));
            found
        })
        .collect();
    let armed: Vec<String> = grants
        .iter()
        .filter(|grant| grant.how == How::Weapon)
        .map(|grant| grant.owner.clone())
        .collect();
    let twinned: Vec<Grant> = grants
        .iter()
        .filter(|grant| grant.how == How::Weapon)
        // A base type shows the icon of one of the companions built on it.
        .filter(|grant| {
            !armed
                .iter()
                .any(|other| *other != grant.owner && lineage.descends(other, &grant.owner))
        })
        .flat_map(|grant| {
            twins(lineage, &grant.owner)
                .into_iter()
                .filter(|twin| !armed.contains(twin))
                .map(|twin| Grant {
                    owner: twin,
                    item: grant.item.clone(),
                    how: How::Weapon,
                })
        })
        .collect();
    grants.extend(twinned);
    grants.sort();
    grants.dedup();
    Grants { grant: grants }
}

/// An edge for every grant whose two ends the catalog holds. Says how many it drew.
pub fn link(graph: &mut Graph, grants: &Grants) -> usize {
    let mut drawn = 0;
    for grant in &grants.grant {
        let owner = normalize::path(&grant.owner).into_owned();
        let item = normalize::path(&grant.item).into_owned();
        if owner == item || !graph.has(&owner) || !graph.has(&item) {
            continue;
        }
        let how = match grant.how {
            How::Weapon => graph::Grant::Weapon,
            How::Bundled => graph::Grant::Bundled,
        };
        graph.link(Edge {
            from: owner,
            to: item,
            rel: Rel::Grants(how),
        });
        drawn += 1;
    }
    drawn
}

fn weapons(lineage: &Lineage, path: &str) -> Vec<Grant> {
    if !lineage.ownable(path) {
        return Vec::new();
    }
    let Some((owner, Value::Text(weapon))) = lineage.field(path, "DefaultWeapon") else {
        return Vec::new();
    };
    // Other types use the same key for the slot a weapon fills.
    if weapon.is_empty() || weapon.starts_with("SLOT_") {
        return Vec::new();
    }
    let item = resolve(owner, &weapon);
    if !lineage.has(&item) {
        return Vec::new();
    }
    vec![Grant {
        owner: path.to_string(),
        item,
        how: How::Weapon,
    }]
}

/// Ownable types beside `owner` that show its icon, as a hound's head shows its model.
fn twins(lineage: &Lineage, owner: &str) -> Vec<String> {
    let Some((_, Value::Text(icon))) = lineage.field(owner, "Icon") else {
        return Vec::new();
    };
    let Some((dir, _)) = owner.rsplit_once('/') else {
        return Vec::new();
    };
    let dir = format!("{dir}/");
    let mut found: Vec<String> = lineage
        .paths()
        .filter(|path| *path != owner && path.starts_with(&dir) && !path.starts_with(STORE))
        .filter(|path| lineage.ownable(path) && lineage.sets(path, "Icon"))
        .filter(|path| {
            lineage
                .own(path)
                .is_some_and(|own| own.text("Icon") == Some(icon.as_str()))
        })
        .map(str::to_string)
        .collect();
    found.sort();
    found
}

fn bundled(lineage: &Lineage, path: &str) -> Vec<Grant> {
    if !path.starts_with(STORE) {
        return Vec::new();
    }
    let Some((owner, Value::Block(listed))) = lineage.field(path, "AdditionalItems") else {
        return Vec::new();
    };
    listed
        .items
        .iter()
        .filter_map(Value::block)
        .filter_map(|extra| extra.text("TypeName"))
        .filter(|name| !name.is_empty())
        .map(|name| Grant {
            owner: path.to_string(),
            item: resolve(owner, name),
            how: How::Bundled,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lineage::tests::kind;

    #[test]
    fn a_hound_head_brings_the_weapon_it_names() {
        let lineage = Lineage::new(vec![
            kind(
                "/Lotus/Pets/ZanukaPetPowerSuit",
                None,
                Some("DefaultWeapon=ZanukaPetMeleeWeapon\n"),
            ),
            kind(
                "/Lotus/Pets/ZanukaPetCPowerSuit",
                Some("/Lotus/Pets/ZanukaPetPowerSuit"),
                Some("DefaultWeapon=ZanukaPetMeleeWeaponPS\n"),
            ),
            kind("/Lotus/Pets/ZanukaPetMeleeWeapon", None, None),
            kind("/Lotus/Pets/ZanukaPetMeleeWeaponPS", None, None),
            kind("/Lotus/StoreItems/Pets/ZanukaPetCPowerSuit", None, None),
        ]);

        assert_eq!(
            read(&lineage).grant,
            [Grant {
                owner: "/Lotus/Pets/ZanukaPetCPowerSuit".into(),
                item: "/Lotus/Pets/ZanukaPetMeleeWeaponPS".into(),
                how: How::Weapon,
            }]
        );
    }

    #[test]
    fn the_part_that_shows_a_hound_brings_its_weapon_too() {
        let lineage = Lineage::new(vec![
            kind(
                "/Lotus/Pets/ZanukaPetCPowerSuit",
                None,
                Some("Icon=/Icons/ZanukaPetHeadC.png\nDefaultWeapon=ZanukaPetMeleeWeaponPS\n"),
            ),
            kind(
                "/Lotus/Pets/Parts/ZanukaPetPartHeadC",
                None,
                Some("Icon=/Icons/ZanukaPetHeadC.png\n"),
            ),
            kind(
                "/Lotus/Pets/Parts/ZanukaPetPartHeadB",
                None,
                Some("Icon=/Icons/ZanukaPetHeadB.png\n"),
            ),
            kind("/Lotus/Pets/ZanukaPetMeleeWeaponPS", None, None),
            kind("/Lotus/StoreItems/Pets/ZanukaPetCPowerSuit", None, None),
            kind(
                "/Lotus/StoreItems/Pets/Parts/ZanukaPetPartHeadC",
                None,
                None,
            ),
            kind(
                "/Lotus/StoreItems/Pets/Parts/ZanukaPetPartHeadB",
                None,
                None,
            ),
        ]);

        let owners: Vec<String> = read(&lineage).grant.into_iter().map(|g| g.owner).collect();

        assert_eq!(
            owners,
            [
                "/Lotus/Pets/Parts/ZanukaPetPartHeadC",
                "/Lotus/Pets/ZanukaPetCPowerSuit",
            ]
        );
    }

    #[test]
    fn a_base_type_lends_its_icon_but_not_its_weapon() {
        let lineage = Lineage::new(vec![
            kind(
                "/Lotus/Pets/ZanukaPetPowerSuit",
                None,
                Some("Icon=/Icons/ZanukaPetHeadA.png\nDefaultWeapon=ZanukaPetMeleeWeapon\n"),
            ),
            kind(
                "/Lotus/Pets/ZanukaPetAPowerSuit",
                Some("/Lotus/Pets/ZanukaPetPowerSuit"),
                Some("DefaultWeapon=ZanukaPetMeleeWeaponIP\n"),
            ),
            kind(
                "/Lotus/Pets/Parts/ZanukaPetPartHeadA",
                None,
                Some("Icon=/Icons/ZanukaPetHeadA.png\n"),
            ),
            kind("/Lotus/Pets/ZanukaPetMeleeWeapon", None, None),
            kind("/Lotus/Pets/ZanukaPetMeleeWeaponIP", None, None),
            kind("/Lotus/StoreItems/Pets/ZanukaPetPowerSuit", None, None),
            kind("/Lotus/StoreItems/Pets/ZanukaPetAPowerSuit", None, None),
            kind(
                "/Lotus/StoreItems/Pets/Parts/ZanukaPetPartHeadA",
                None,
                None,
            ),
        ]);

        let head: Vec<String> = read(&lineage)
            .grant
            .into_iter()
            .filter(|g| g.owner == "/Lotus/Pets/Parts/ZanukaPetPartHeadA")
            .map(|g| g.item)
            .collect();

        assert_eq!(head, ["/Lotus/Pets/ZanukaPetMeleeWeaponIP"]);
    }

    #[test]
    fn a_weapon_slot_is_not_a_weapon() {
        let lineage = Lineage::new(vec![
            kind("/Lotus/Gear/Item", None, Some("DefaultWeapon=SLOT_2\n")),
            kind("/Lotus/StoreItems/Gear/Item", None, None),
        ]);

        assert!(read(&lineage).grant.is_empty());
    }

    #[test]
    fn a_store_item_brings_what_it_lists_beside_it() {
        let lineage = Lineage::new(vec![kind(
            "/Lotus/StoreItems/Powersuits/Khora/Khora",
            None,
            Some(
                "AdditionalItems={
{
TypeName=/Lotus/StoreItems/Upgrades/Skins/Khora/KhoraHelmet
PurchaseQuantity=1
},
{
TypeName=Kavat/KhoraKavatPowerSuit
PurchaseQuantity=1
}
}
",
            ),
        )]);

        let items: Vec<String> = read(&lineage).grant.into_iter().map(|g| g.item).collect();

        assert_eq!(
            items,
            [
                "/Lotus/StoreItems/Powersuits/Khora/Kavat/KhoraKavatPowerSuit",
                "/Lotus/StoreItems/Upgrades/Skins/Khora/KhoraHelmet",
            ]
        );
    }
}
