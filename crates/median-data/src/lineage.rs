use std::collections::HashMap;

use sources::notation::{self, Block, Value};
use sources::packages::Type;

const LOTUS: &str = "/Lotus/";
const STORE: &str = "/Lotus/StoreItems/";
/// Deepest a parent chain runs before it is taken for a loop.
const DEPTH: usize = 64;

/// The game's types by path, each with its parent and the properties it sets itself.
pub struct Lineage {
    types: HashMap<String, Type>,
}

impl Lineage {
    pub fn new(types: Vec<Type>) -> Self {
        Self {
            types: types.into_iter().map(|t| (t.path.clone(), t)).collect(),
        }
    }

    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.types.keys().map(String::as_str)
    }

    pub fn has(&self, path: &str) -> bool {
        self.types.contains_key(path)
    }

    /// Whether a player can own the type: the game sells it under a store path.
    pub fn ownable(&self, path: &str) -> bool {
        path.strip_prefix(LOTUS)
            .is_some_and(|rest| self.has(&format!("{STORE}{rest}")))
    }

    /// Whether the type sets the field itself.
    pub fn sets(&self, path: &str, key: &str) -> bool {
        self.types
            .get(path)
            .and_then(|t| t.text.as_deref())
            .is_some_and(|text| sets(text, key))
    }

    /// The properties the type sets itself.
    pub fn own(&self, path: &str) -> Option<Block> {
        let text = self.types.get(path)?.text.as_deref()?;
        notation::read(text).ok()
    }

    /// Whether `ancestor` stands somewhere above the type in its parent chain.
    pub fn descends(&self, path: &str, ancestor: &str) -> bool {
        let mut at = self.types.get(path);
        for _ in 0..DEPTH {
            let Some(parent) = at.and_then(|t| t.parent.as_deref()) else {
                return false;
            };
            if parent == ancestor {
                return true;
            }
            at = self.types.get(parent);
        }
        false
    }

    /// A top-level field as the type sets or inherits it, with the type that set it.
    pub fn field(&self, path: &str, key: &str) -> Option<(&str, Value)> {
        let mut at = self.types.get(path)?;
        for _ in 0..DEPTH {
            if at.text.as_deref().is_some_and(|text| sets(text, key)) {
                if let Some(value) = self
                    .own(&at.path)
                    .and_then(|own| own.fields.get(key).cloned())
                {
                    return Some((&at.path, value));
                }
            }
            at = self.types.get(at.parent.as_deref()?)?;
        }
        None
    }
}

/// A path the game wrote beside `owner`, made whole.
pub fn resolve(owner: &str, written: &str) -> String {
    if written.starts_with('/') {
        return written.to_string();
    }
    let dir = owner.rsplit_once('/').map_or("", |(dir, _)| dir);
    format!("{dir}/{written}")
}

fn sets(text: &str, key: &str) -> bool {
    text.split('\n').any(|line| {
        line.strip_prefix(key)
            .is_some_and(|rest| rest.starts_with('='))
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn kind(path: &str, parent: Option<&str>, text: Option<&str>) -> Type {
        Type {
            path: path.into(),
            parent: parent.map(Into::into),
            text: text.map(Into::into),
        }
    }

    fn hounds() -> Lineage {
        Lineage::new(vec![
            kind(
                "/Lotus/Pets/HoundPowerSuit",
                None,
                Some("DefaultWeapon=HoundWeapon\nArmour=5\n"),
            ),
            kind(
                "/Lotus/Pets/HoundCPowerSuit",
                Some("/Lotus/Pets/HoundPowerSuit"),
                Some("Icon=C.png\n"),
            ),
            kind("/Lotus/StoreItems/Pets/HoundCPowerSuit", None, None),
        ])
    }

    #[test]
    fn a_field_is_inherited_from_the_type_that_sets_it() {
        let lineage = hounds();

        let (owner, value) = lineage
            .field("/Lotus/Pets/HoundCPowerSuit", "DefaultWeapon")
            .unwrap();

        assert_eq!(owner, "/Lotus/Pets/HoundPowerSuit");
        assert_eq!(value.text(), Some("HoundWeapon"));
        assert!(
            lineage
                .field("/Lotus/Pets/HoundCPowerSuit", "Default")
                .is_none()
        );
    }

    #[test]
    fn a_type_sold_under_a_store_path_is_ownable() {
        let lineage = hounds();

        assert!(lineage.ownable("/Lotus/Pets/HoundCPowerSuit"));
        assert!(!lineage.ownable("/Lotus/Pets/HoundPowerSuit"));
    }

    #[test]
    fn a_name_written_beside_its_owner_is_made_whole() {
        assert_eq!(
            resolve("/Lotus/Pets/HoundPowerSuit", "HoundWeapon"),
            "/Lotus/Pets/HoundWeapon"
        );
        assert_eq!(
            resolve(
                "/Lotus/StoreItems/Powersuits/Khora/Khora",
                "Kavat/KhoraKavatPowerSuit"
            ),
            "/Lotus/StoreItems/Powersuits/Khora/Kavat/KhoraKavatPowerSuit"
        );
        assert_eq!(resolve("/Lotus/A/B", "/Lotus/C"), "/Lotus/C");
    }
}
