use std::borrow::Cow;

const STORE: &str = "/Lotus/StoreItems/";

/// DE store references (`/Lotus/StoreItems/…`) name the same entity as the plain path.
pub fn path(raw: &str) -> Cow<'_, str> {
    match raw.strip_prefix(STORE) {
        Some(rest) => Cow::Owned(format!("/Lotus/{rest}")),
        None => Cow::Borrowed(raw),
    }
}

/// Clean a display name DE exported with internal markup: a leading `<TAG>` (as in
/// `<ARCHWING> Odonata` or `<SHARD_RED_SIMPLE> …`) and any trailing control characters.
pub fn name(raw: &str) -> &str {
    let mut name = raw.trim();
    if let Some(rest) = name.strip_prefix('<') {
        if let Some((_, after)) = rest.split_once('>') {
            name = after.trim_start();
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_store_prefix() {
        assert_eq!(
            path("/Lotus/StoreItems/Types/Recipes/WarframeRecipes/VoltPrimeChassisBlueprint"),
            "/Lotus/Types/Recipes/WarframeRecipes/VoltPrimeChassisBlueprint"
        );
    }

    #[test]
    fn leaves_plain_path_untouched() {
        let plain = "/Lotus/Powersuits/Volt/VoltPrime";
        assert_eq!(path(plain), plain);
    }

    #[test]
    fn strips_a_leading_marker() {
        assert_eq!(name("<ARCHWING> Odonata"), "Odonata");
        assert_eq!(name("<SHARD_RED_SIMPLE> Crimson Archon Shard"), "Crimson Archon Shard");
    }

    #[test]
    fn trims_trailing_control_characters() {
        assert_eq!(name("Solstice Square Stage Scene\r\n"), "Solstice Square Stage Scene");
    }

    #[test]
    fn leaves_a_plain_name_untouched() {
        assert_eq!(name("Volt Prime"), "Volt Prime");
        assert_eq!(name("Wisp <3"), "Wisp <3");
    }
}
