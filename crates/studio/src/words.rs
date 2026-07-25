use consensus::{Source, Status};
use graph::{PlaceKind, Rel};

/// Reading of a relation, for the entity card.
pub fn rel(rel: &Rel) -> &'static str {
    match rel {
        Rel::Produces => "производит",
        Rel::Requires { .. } => "требует",
        Rel::Rewards { .. } => "выдаёт",
        Rel::Member => "часть",
        Rel::Represents => "собирается в",
        Rel::Drops(_) => "роняет",
        Rel::Primed => "прайм-версия",
        Rel::Yields => "выводит",
        Rel::At => "на узле",
        Rel::Sells(_) => "продаёт",
        Rel::Refines => "улучшается в",
        Rel::Researched(_) => "исследуется",
    }
}

/// Reading of a relation from the other end.
pub fn rel_back(rel: &Rel) -> &'static str {
    match rel {
        Rel::Produces => "производится",
        Rel::Requires { .. } => "нужен для",
        Rel::Rewards { .. } => "выдаётся из",
        Rel::Member => "входит в",
        Rel::Represents => "собирается из",
        Rel::Drops(_) => "падает в",
        Rel::Primed => "обычная версия",
        Rel::Yields => "выводится из",
        Rel::At => "таблица наград",
        Rel::Sells(_) => "продаётся у",
        Rel::Refines => "улучшается из",
        Rel::Researched(_) => "исследуется в",
    }
}

/// How a value is backed by its sources.
pub fn status(status: Status) -> &'static str {
    match status {
        Status::Single => "один источник",
        Status::Confirmed => "подтверждено",
        Status::Conflict => "конфликт",
    }
}

/// Short label for a source, for the provenance badges.
pub fn source(source: Source) -> &'static str {
    match source {
        Source::Curated => "рука",
        Source::De => "DE",
        Source::Wiki => "вики",
        Source::Wfm => "WFM",
        Source::Rule => "прав",
    }
}

/// What a source is, spelled out.
pub fn source_full(source: Source) -> &'static str {
    match source {
        Source::Curated => "решено вручную",
        Source::De => "экспорт DE",
        Source::Wiki => "wiki.warframe.com",
        Source::Wfm => "warframe.market",
        Source::Rule => "правило сборки",
    }
}

/// What kind of source drops things here.
pub fn place(kind: PlaceKind) -> &'static str {
    match kind {
        PlaceKind::Node => "узел",
        PlaceKind::Key => "ключ",
        PlaceKind::Sortie => "вылазка",
        PlaceKind::Bounty => "задание",
        PlaceKind::Transient => "временное",
        PlaceKind::Enemy => "враг",
    }
}

/// The property a claim is about.
pub fn prop(prop: &str) -> &str {
    match prop {
        "name_en" => "имя (en)",
        "name_ru" => "имя (ru)",
        "category" => "категория",
        "kind" => "подкатегория",
        "slug" => "слаг рынка",
        "tradable" => "торгуется",
        "prime" => "прайм",
        other => other,
    }
}

/// What a place that hands items over actually is. Not all of them are people: a syndicate, an
/// event and a game mode all have a counter, and calling them a trader reads as a mistake.
pub fn vendor(kind: Option<&str>) -> &str {
    match kind {
        Some("Syndicate") => "синдикат",
        Some("Event") => "событие",
        Some("Store") => "магазин",
        Some("Rotating Store") => "магазин с ротацией",
        Some("Weapon Vendor") => "оружейник",
        Some("Mining Vendor") => "добыча",
        Some("Fishing Vendor") => "рыбалка",
        Some("Conservation Vendor") => "заповедник",
        Some("Fishing and Conservation Vendor") => "рыбалка и заповедник",
        Some("Decoration Vendor") => "декор",
        Some("Companion Vendor") => "компаньоны",
        Some("Nightwave") => "Nightwave",
        Some(other) => other,
        None => "торговец",
    }
}

/// Which source prints a name the catalog could not tie to an item.
pub fn origin(source: &str) -> &str {
    match source {
        "market" => "рынок",
        "drops" => "таблицы дропа",
        "vendor" => "торговцы",
        "dojo" => "исследования",
        other => other,
    }
}

/// What a hand-written Russian word names.
pub fn term(kind: &str) -> &str {
    match kind {
        "item" => "предмет",
        "place" => "место",
        "region" => "узел",
        "planet" => "планета",
        "vendor" => "торговец",
        "lab" => "лаборатория",
        "mission" => "тип миссии",
        "faction" => "фракция",
        "node_type" => "тип узла",
        "settlement" => "поселение",
        "giver" => "кто выдаёт",
        "activity" => "задание",
        "class" => "класс",
        "kind" => "подкатегория",
        other => other,
    }
}

/// A relic refinement, as the game prints it.
pub fn refinement(refinement: &str) -> &str {
    match refinement {
        "intact" => "Нетронутая",
        "exceptional" => "Испытанная",
        "flawless" => "Безупречная",
        "radiant" => "Сияющая",
        other => other,
    }
}

/// What a funnel check looks for.
pub fn rule(rule: &str) -> &str {
    match rule {
        "anchor-broken" => "курируемый факт больше не выполняется",
        "built-is-tradable" => "собираемое числится торгуемым",
        "craft-not-in-catalog" => "рецепт ссылается на предмет вне каталога",
        "dangling-edge" => "связь ведёт в никуда",
        "dead-recipe" => "рецепт, который некому начать",
        "drop-not-in-catalog" => "дроп называет предмет вне каталога",
        "drop-only-on-the-wiki" => "вики знает дроп, официальные таблицы — нет",
        "drop-unwitnessed" => "наш дроп вики не подтверждает",
        "kind-unresolved" => "предмет не попал ни в одну подкатегорию",
        "recipe-cycle" => "рецепт требует сам себя",
        "ducat-value" => "дукаты не как у соседей",
        "ingredient-count" => "необычное число ингредиентов",
        "item-path" => "путь предмета не по форме",
        "name-empty" => "пустое имя",
        "prime-unobtainable" => "прайм ниоткуда не добывается",
        "relic-chance-differs" => "шанс реликвии расходится с таблицами",
        "relic-reward-count" => "необычное число наград у реликвии",
        "relic-reward-missing" => "таблицы знают награду, DE — нет",
        "relic-reward-unwitnessed" => "DE знает награду, таблицы — нет",
        "reward-not-in-catalog" => "награда вне каталога",
        "russian-name-missing" => "нет русского имени",
        "set-composition-differs" => "состав набора расходится с рецептом",
        "set-size" => "необычный размер набора",
        "set-without-item" => "набор без собранного предмета",
        "tradable-needs-slug" => "торгуемое без слага рынка",
        other => other,
    }
}

/// What a funnel layer is for.
pub fn layer(layer: &str) -> &str {
    match layer {
        "invariant" => "нарушения структуры",
        "cross" => "сверка источников",
        "outlier" => "выбросы",
        "coverage" => "пробелы",
        "anchor" => "якоря",
        other => other,
    }
}

/// Russian plural for a count: "1 предмет", "2 предмета", "5 предметов".
pub fn plural(n: usize, one: &'static str, few: &'static str, many: &'static str) -> &'static str {
    let (tens, units) = (n % 100, n % 10);
    if (11..=14).contains(&tens) {
        return many;
    }
    match units {
        1 => one,
        2..=4 => few,
        _ => many,
    }
}

/// A foundry build time as `Ч ч М мин`.
pub fn duration(secs: i64) -> String {
    if secs <= 0 {
        return "—".to_string();
    }
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    match (h, m) {
        (0, m) => format!("{m} мин"),
        (h, 0) => format!("{h} ч"),
        (h, m) => format!("{h} ч {m} мин"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plurals_follow_the_teens_exception() {
        let word = |n| plural(n, "предмет", "предмета", "предметов");
        assert_eq!(word(1), "предмет");
        assert_eq!(word(3), "предмета");
        assert_eq!(word(5), "предметов");
        assert_eq!(word(11), "предметов");
        assert_eq!(word(21), "предмет");
        assert_eq!(word(112), "предметов");
    }

    #[test]
    fn durations_drop_empty_parts() {
        assert_eq!(duration(3600), "1 ч");
        assert_eq!(duration(5400), "1 ч 30 мин");
        assert_eq!(duration(600), "10 мин");
        assert_eq!(duration(0), "—");
    }
}
