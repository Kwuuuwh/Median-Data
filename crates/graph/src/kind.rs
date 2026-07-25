use std::collections::BTreeMap;

use serde::Deserialize;

/// A leaf of the taxonomy. Slugs are unique across classes, so a kind names its class too.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Kind(String);

impl Kind {
    /// Where an item lands when no rule classifies it.
    pub const UNKNOWN: &'static str = "unknown";

    pub fn new(slug: impl Into<String>) -> Self {
        Self(slug.into())
    }

    pub fn unknown() -> Self {
        Self::new(Self::UNKNOWN)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_unknown(&self) -> bool {
        self.0 == Self::UNKNOWN
    }
}

/// One declared kind, with its label per language.
#[derive(Debug, Clone, Deserialize)]
pub struct Leaf {
    pub slug: String,
    pub en: String,
    pub ru: String,
}

/// A top-level group of kinds.
#[derive(Debug, Clone, Deserialize)]
pub struct Class {
    pub slug: String,
    pub en: String,
    pub ru: String,
    /// Whether we care where these items come from. Appearance items are bought and gifted in
    /// ways the game never publishes, and the product does not need to explain them, so asking
    /// would only produce a permanent gap.
    #[serde(default = "yes")]
    pub sourced: bool,
    #[serde(default)]
    pub kind: Vec<Leaf>,
}

fn yes() -> bool {
    true
}

/// The declared tree: classes in the order they were written, kinds indexed by slug.
#[derive(Debug, Default, Clone)]
pub struct Taxonomy {
    classes: Vec<Class>,
    index: BTreeMap<String, (usize, usize)>,
}

impl Taxonomy {
    /// Index declared classes. A kind slug used twice is an error: a kind names its class,
    /// so the slug has to be unique.
    pub fn new(classes: Vec<Class>) -> Result<Self, String> {
        let mut index = BTreeMap::new();
        for (ci, class) in classes.iter().enumerate() {
            for (li, leaf) in class.kind.iter().enumerate() {
                if let Some((other, _)) = index.insert(leaf.slug.clone(), (ci, li)) {
                    return Err(format!(
                        "kind '{}' is declared in both '{}' and '{}'",
                        leaf.slug, classes[other].slug, class.slug
                    ));
                }
            }
        }
        Ok(Self { classes, index })
    }

    pub fn classes(&self) -> &[Class] {
        &self.classes
    }

    pub fn has(&self, kind: &str) -> bool {
        self.index.contains_key(kind)
    }

    pub fn class_of(&self, kind: &Kind) -> Option<&Class> {
        let (ci, _) = self.index.get(kind.as_str())?;
        self.classes.get(*ci)
    }

    pub fn leaf(&self, kind: &Kind) -> Option<&Leaf> {
        let (ci, li) = self.index.get(kind.as_str())?;
        self.classes.get(*ci)?.kind.get(*li)
    }

    /// The kind's label in one language, falling back to its slug.
    pub fn label<'a>(&'a self, kind: &'a Kind, lang: &str) -> &'a str {
        match self.leaf(kind) {
            Some(leaf) if lang == "ru" => &leaf.ru,
            Some(leaf) => &leaf.en,
            None => kind.as_str(),
        }
    }

    /// The class label of a kind, in one language.
    pub fn class_label(&self, kind: &Kind, lang: &str) -> &str {
        match self.class_of(kind) {
            Some(class) if lang == "ru" => &class.ru,
            Some(class) => &class.en,
            None => Kind::UNKNOWN,
        }
    }

    /// Whether the class a kind belongs to is one whose sources we track.
    pub fn sourced(&self, kind: &Kind) -> bool {
        self.class_of(kind).is_none_or(|c| c.sourced)
    }

    /// The class slug a kind belongs to.
    pub fn class_slug(&self, kind: &Kind) -> &str {
        self.class_of(kind).map_or(Kind::UNKNOWN, |c| c.slug.as_str())
    }

    /// Write the Russian label of a class or a kind by hand. Unknown slugs are ignored: the
    /// tree is declared in one place and this only translates what it declares.
    pub fn relabel(&mut self, slug: &str, ru: &str) {
        if let Some(&(ci, li)) = self.index.get(slug) {
            self.classes[ci].kind[li].ru = ru.to_string();
            return;
        }
        if let Some(class) = self.classes.iter_mut().find(|c| c.slug == slug) {
            class.ru = ru.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Taxonomy {
        Taxonomy::new(vec![
            Class {
                slug: "mod".into(),
                en: "Mods".into(),
                ru: "Моды".into(),
                sourced: true,
                kind: vec![Leaf {
                    slug: "aura".into(),
                    en: "Aura".into(),
                    ru: "Аура".into(),
                }],
            },
            Class {
                slug: "unknown".into(),
                en: "Unclassified".into(),
                ru: "Не определено".into(),
                sourced: true,
                kind: vec![Leaf {
                    slug: "unknown".into(),
                    en: "Unclassified".into(),
                    ru: "Не определено".into(),
                }],
            },
        ])
        .unwrap()
    }

    #[test]
    fn a_kind_knows_its_class() {
        let t = tree();
        let aura = Kind::new("aura");
        assert_eq!(t.class_slug(&aura), "mod");
        assert_eq!(t.label(&aura, "ru"), "Аура");
        assert_eq!(t.class_label(&aura, "ru"), "Моды");
    }

    #[test]
    fn an_undeclared_kind_falls_back_to_its_slug() {
        let t = tree();
        let made_up = Kind::new("dropship");
        assert!(!t.has(made_up.as_str()));
        assert_eq!(t.label(&made_up, "ru"), "dropship");
        assert_eq!(t.class_slug(&made_up), Kind::UNKNOWN);
    }

    #[test]
    fn a_kind_slug_declared_twice_is_rejected() {
        let leaf = || Leaf {
            slug: "aura".into(),
            en: "Aura".into(),
            ru: "Аура".into(),
        };
        let class = |slug: &str| Class {
            slug: slug.into(),
            en: slug.into(),
            ru: slug.into(),
            sourced: true,
            kind: vec![leaf()],
        };
        let err = Taxonomy::new(vec![class("mod"), class("gear")]).unwrap_err();
        assert!(err.contains("aura"), "{err}");
    }
}
