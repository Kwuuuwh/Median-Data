use std::collections::BTreeMap;

use graph::{Extra, Graph, Node, Weapon};

use crate::wiki::WeaponEntry;

/// Hand every weapon item its DE weapon facts. Returns how many items took them.
pub fn attach(graph: &mut Graph, weapons: &BTreeMap<String, Weapon>) -> usize {
    let mut armed = 0;
    for (unique_name, weapon) in weapons {
        if let Some(Node::Item(item)) = graph.get_mut(unique_name) {
            item.extra = Extra::Weapon(Box::new(weapon.clone()));
            armed += 1;
        }
    }
    armed
}

/// What tying the wiki's weapon entries to DE's weapons came to.
#[derive(Default)]
pub struct Mounted {
    pub mounts: usize,
    /// Entries naming no DE weapon, by entry name, with the path they name.
    pub unmatched: Vec<(String, Option<String>)>,
    /// What an entry holds that could not be read, by the weapon it describes.
    pub unread: Vec<(String, String)>,
}

/// Hang each wiki weapon entry on the DE weapon its `InternalName` names.
pub fn mount(graph: &mut Graph, entries: Vec<WeaponEntry>) -> Mounted {
    let mut out = Mounted::default();
    for entry in entries {
        let entity = entry
            .path
            .clone()
            .unwrap_or_else(|| entry.mount.name.clone());
        out.unread
            .extend(entry.unread.into_iter().map(|note| (entity.clone(), note)));
        let weapon = entry
            .path
            .as_deref()
            .and_then(|path| match graph.get_mut(path) {
                Some(Node::Item(item)) => match &mut item.extra {
                    Extra::Weapon(weapon) => Some(weapon),
                    _ => None,
                },
                _ => None,
            });
        match weapon {
            Some(weapon) => {
                weapon.mounts.push(entry.mount);
                out.mounts += 1;
            }
            None => out.unmatched.push((entry.mount.name, entry.path)),
        }
    }
    out
}
