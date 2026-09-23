use std::collections::BTreeMap;

use graph::{Graph, Node, Rel, Taxonomy};

use crate::finding::{Finding, Layer};

/// One measurement, judged against others in its group.
struct Sample {
    group: String,
    entity: String,
    value: f64,
}

/// Groups smaller than this say nothing about what is normal.
const MIN_GROUP: usize = 8;

/// How far a relic's slot chances may stand from a whole before it stops looking normal.
const SUM_TOLERANCE: f64 = 0.005;

/// Flag values unlike their siblings, across every measured series.
pub fn check(graph: &Graph, taxonomy: &Taxonomy) -> Vec<Finding> {
    let mut out = Vec::new();
    out.extend(judge(
        &relic_rewards(graph),
        "relic-reward-count",
        "rewards",
    ));
    out.extend(judge(&set_sizes(graph, taxonomy), "set-size", "parts"));
    out.extend(judge(
        &ingredients(graph),
        "ingredient-count",
        "ingredients",
    ));
    out.extend(judge(&ducats(graph), "ducat-value", "ducats"));
    out.extend(chances_add_up(graph));
    out
}

/// Opening a relic hands over exactly one reward, so the chances of its slots add up to a
/// whole. Requiem relics answer to a table of their own and stand out here honestly, which is
/// why this reports rather than gates.
fn chances_add_up(graph: &Graph) -> Vec<Finding> {
    let mut odds: BTreeMap<&str, f64> = BTreeMap::new();
    for edge in graph.edges() {
        let Rel::Rewards { chance, .. } = &edge.rel else {
            continue;
        };
        let Some(chance) = chance else { continue };
        *odds.entry(edge.from.as_str()).or_default() += chance;
    }
    odds.into_iter()
        .filter(|(_, sum)| (sum - 1.0).abs() > SUM_TOLERANCE)
        .map(|(relic, sum)| {
            Finding::new(
                Layer::Outlier,
                "relic-chances-sum",
                relic,
                format!("slot chances add up to {:.2}%", sum * 100.0),
            )
        })
        .collect()
}

/// Compare each sample with the median of its group, using a median absolute deviation so
/// one wild value cannot widen the band that should catch it.
fn judge(samples: &[Sample], rule: &str, unit: &str) -> Vec<Finding> {
    let mut grouped: BTreeMap<&str, Vec<&Sample>> = BTreeMap::new();
    for s in samples {
        grouped.entry(&s.group).or_default().push(s);
    }

    let mut out = Vec::new();
    for (group, members) in grouped {
        if members.len() < MIN_GROUP {
            continue;
        }
        let values: Vec<f64> = members.iter().map(|s| s.value).collect();
        let mid = median(&values);
        let spread: Vec<f64> = values.iter().map(|v| (v - mid).abs()).collect();
        // 1.4826 puts the deviation on the same scale as a standard deviation.
        let band = (3.0 * 1.4826 * median(&spread)).max(1.0);

        for s in members {
            if (s.value - mid).abs() > band {
                out.push(Finding::new(
                    Layer::Outlier,
                    rule,
                    &s.entity,
                    format!(
                        "{} {unit} against {mid} typical for {group}",
                        s.value as i64
                    ),
                ));
            }
        }
    }
    out
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("counts are never NaN"));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

fn relic_rewards(graph: &Graph) -> Vec<Sample> {
    graph
        .items()
        .filter(|i| matches!(i.extra, graph::Extra::Relic(_)))
        .map(|i| Sample {
            group: "relic".to_string(),
            entity: i.unique_name.clone(),
            value: count(graph.from(&i.unique_name), |e| {
                matches!(e.rel, Rel::Rewards { .. })
            }),
        })
        .collect()
}

/// How many parts a set sells, judged against sets of the same kind of thing. A warframe is
/// four parts and a melee weapon two, so one number for every set would call half of them odd.
fn set_sizes(graph: &Graph, taxonomy: &Taxonomy) -> Vec<Sample> {
    graph
        .nodes()
        .filter_map(|n| match n {
            Node::Set(s) => Some((n.id(), s)),
            _ => None,
        })
        .map(|(id, s)| Sample {
            group: format!("set/{}", assembles(graph, taxonomy, &id).unwrap_or("mixed")),
            entity: s.slug.clone(),
            value: count(graph.from(&id), |e| matches!(e.rel, Rel::Member)),
        })
        .collect()
}

/// The class of the thing a set assembles, where the catalog holds it.
fn assembles<'a>(graph: &'a Graph, taxonomy: &'a Taxonomy, set: &str) -> Option<&'a str> {
    let built = graph
        .from(set)
        .into_iter()
        .find(|e| matches!(e.rel, Rel::Represents))?;
    match graph.get(&built.to)? {
        Node::Item(item) => Some(taxonomy.class_slug(&item.kind.value)),
        _ => None,
    }
}

fn ingredients(graph: &Graph) -> Vec<Sample> {
    graph
        .nodes()
        .filter_map(|n| match n {
            Node::Recipe(r) => Some((n.id(), r)),
            _ => None,
        })
        .filter_map(|(id, r)| {
            let built = graph
                .from(&id)
                .into_iter()
                .find(|e| matches!(e.rel, Rel::Produces))?;
            // Grouped by what the recipe builds, not by the manifest DE filed it under: one
            // manifest holds an alternate helmet swap with a single ingredient and a weapon
            // with five, and neither says anything about the other.
            let kind = match graph.get(&built.to)? {
                Node::Item(i) => i.kind.value.as_str().to_string(),
                _ => return None,
            };
            let needs: Vec<&str> = graph
                .from(&id)
                .into_iter()
                .filter(|e| matches!(e.rel, Rel::Requires { .. }))
                .map(|e| e.to.as_str())
                .collect();
            // A recipe with nothing to give is not a small recipe: a lich weapon is carried
            // over rather than built, so it has no ingredient list to compare.
            if needs.is_empty() {
                return None;
            }
            // A swap — one thing handed over for another of the same kind, as an alternate
            // helmet is — is its own kind of recipe and says nothing about a build.
            let swap = needs
                .iter()
                .all(|need| matches!(graph.get(need), Some(Node::Item(i)) if i.kind.value.as_str() == kind));
            Some(Sample {
                group: match swap {
                    true => format!("{kind}/swap"),
                    false => kind,
                },
                entity: r.blueprint.clone(),
                value: needs.len() as f64,
            })
        })
        .collect()
}

fn ducats(graph: &Graph) -> Vec<Sample> {
    graph
        .items()
        .filter_map(|i| {
            i.ducats.map(|d| Sample {
                group: i.category.value.clone(),
                entity: i.unique_name.clone(),
                value: d as f64,
            })
        })
        .collect()
}

fn count(edges: Vec<&graph::Edge>, want: impl Fn(&graph::Edge) -> bool) -> f64 {
    edges.into_iter().filter(|e| want(e)).count() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(group: &str, entity: &str, value: f64) -> Sample {
        Sample {
            group: group.to_string(),
            entity: entity.to_string(),
            value,
        }
    }

    #[test]
    fn flags_only_the_odd_one_out() {
        let mut samples: Vec<Sample> = (0..10)
            .map(|i| sample("relic", &format!("/r/{i}"), 6.0))
            .collect();
        samples.push(sample("relic", "/r/odd", 20.0));

        let found = judge(&samples, "relic-reward-count", "rewards");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity, "/r/odd");
    }

    #[test]
    fn ignores_groups_too_small_to_judge() {
        let samples = vec![sample("tiny", "/a", 1.0), sample("tiny", "/b", 99.0)];
        assert!(judge(&samples, "rule", "units").is_empty());
    }

    #[test]
    fn tolerates_one_step_in_a_tight_group() {
        let mut samples: Vec<Sample> = (0..10)
            .map(|i| sample("set", &format!("/s/{i}"), 4.0))
            .collect();
        samples.push(sample("set", "/s/five", 5.0));
        assert!(judge(&samples, "set-size", "parts").is_empty());
    }
}
