use graph::{Graph, Node, Rel};

/// Print every node whose id or English name contains `query`, with its neighbourhood.
pub fn run(graph: &Graph, query: &str) {
    let needle = query.to_lowercase();
    let hits: Vec<&Node> = graph
        .nodes()
        .filter(|n| {
            n.id().to_lowercase().contains(&needle) || n.label().to_lowercase().contains(&needle)
        })
        .collect();

    if hits.is_empty() {
        println!("nothing matches '{query}'");
        return;
    }
    for node in hits {
        print(graph, node);
    }
}

fn print(graph: &Graph, node: &Node) {
    let id = node.id();
    println!("\n=== {} ===", node.label());
    println!("  id       {id}");
    match node {
        Node::Item(it) => {
            if let Some(ru) = &it.names.ru {
                println!("  ru       {} [{}]", ru.value, ru.status.as_str());
            }
            println!("  category {}", it.category.value);
            if let Some(slug) = &it.slug {
                println!("  slug     {}", slug.value);
            }
            if let Some(t) = &it.tradable {
                println!("  tradable {} [{}]", t.value, t.status.as_str());
            }
            if let Some(d) = it.ducats {
                println!("  ducats   {d}");
            }
        }
        Node::Recipe(r) => {
            println!("  consumed {}", r.consumed);
            if let Some(p) = r.build_price {
                println!("  credits  {p}");
            }
        }
        Node::Set(s) => {
            if let Some(ru) = &s.names.ru {
                println!("  ru       {}", ru.value);
            }
            if let Some(d) = s.ducats {
                println!("  ducats   {d}");
            }
        }
        Node::Imprint(i) => {
            println!("  animal   {}", i.animal);
        }
        Node::Place(p) => println!("  kind     {}", p.kind.as_str()),
        Node::Enemy(e) => {
            if let Some(ru) = &e.name_ru {
                println!("  ru       {ru}");
            }
        }
        Node::Location(l) => {
            println!("  kind     {}", l.kind.as_deref().unwrap_or("?"));
            if let Some(ru) = &l.name_ru {
                println!("  ru       {ru}");
            }
        }
        Node::Vendor(v) => println!("  vendor   {} (rotates {})", v.name, v.rotates),
        Node::Lab(l) => println!("  lab      {} ({})", l.name, l.faction),
        Node::Region(r) => {
            println!("  location {}", r.location);
            println!(
                "  mission  {} ({})",
                r.mission_label.en.as_deref().unwrap_or("?"),
                r.mission
            );
            println!(
                "  faction  {} ({})",
                r.faction_label.en.as_deref().unwrap_or("?"),
                r.faction
            );
            println!("  levels   {}-{}", r.min_level, r.max_level);
        }
    }

    for edge in graph.from(&id) {
        println!(
            "  -{}-> {}{}",
            edge.rel.as_str(),
            label(graph, &edge.to),
            detail(&edge.rel)
        );
    }
    for edge in graph.into(&id) {
        println!(
            "  <-{}- {}{}",
            edge.rel.as_str(),
            label(graph, &edge.from),
            detail(&edge.rel)
        );
    }
}

fn detail(rel: &Rel) -> String {
    match rel {
        Rel::Rewards { rarity, .. } => format!(" ({rarity})"),
        Rel::Requires { count } if *count > 1 => format!(" (x{count})"),
        Rel::Drops(d) => {
            let where_ = [d.rotation.as_deref(), d.stage.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            let at = if where_.is_empty() {
                String::new()
            } else {
                format!(", {where_}")
            };
            format!(" ({} {:.2}%{at})", d.rarity, d.chance * 100.0)
        }
        _ => String::new(),
    }
}

fn label(graph: &Graph, id: &str) -> String {
    match graph.get(id) {
        Some(node) => format!("{} [{id}]", node.label()),
        None => id.to_string(),
    }
}
